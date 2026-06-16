//! Cross-layer type-role resolution helpers for `evaluate_catalogue_lint`.
//!
//! This module is declared by `catalogue_linter_eval.rs` via `#[path]` and is
//! not a public module. All items are `pub(super)` so they are visible to
//! `evaluate_catalogue_lint` in the parent `eval` module.
//!
//! These helpers implement the cross-layer lookup logic that allows rules such
//! as `ReferencedRoleConstraint` and `NoRoleInMethodSignature` to resolve type
//! roles across multiple catalogue documents.

use std::collections::BTreeMap;

use super::super::RoleKind;
use super::super::helpers::{bare_name_in_type_ref, entry_role_kind};
use crate::tddd::catalogue_v2::CatalogueDocument;
use crate::tddd::catalogue_v2::roles::ItemAction;
use crate::tddd::layer_id::LayerId;

/// Strips leading reference and pointer sigils from a type reference string.
///
/// Rust type references in method signatures and `TypeRef` fields may be written
/// with leading `&`, `&mut `, or `*` prefixes (e.g. `"&domain::OrderPlaced"`,
/// `"&mut domain::Foo"`, `"*const T"`).  Before extracting a layer hint from the
/// path, these sigils must be removed so that `"&domain"` is not mistaken for an
/// unknown layer prefix.
///
/// Strips in a loop to handle forms like `"&&T"` or `"*mut T"`.  The returned
/// slice always starts at the first non-sigil character.
pub(super) fn strip_ref_sigils(mut s: &str) -> &str {
    loop {
        if let Some(rest) = s.strip_prefix("&mut ") {
            s = rest;
        } else if let Some(rest) = s.strip_prefix("*mut ") {
            s = rest;
        } else if let Some(rest) = s.strip_prefix("*const ") {
            s = rest;
        } else if let Some(rest) = s.strip_prefix('&') {
            s = rest;
        } else if let Some(rest) = s.strip_prefix('*') {
            s = rest;
        } else {
            break;
        }
    }
    s
}

/// Extract a top-level layer hint and the full tail from a type reference,
/// but only when the `::` separator is not nested inside generic angle brackets.
///
/// Finds the **rightmost** `::` that appears before any `<`, so the layer hint
/// is everything before that separator and the tail is the final path segment
/// (possibly including generic arguments such as `Wrapper<crate::Foo>`).
///
/// A `::` that appears inside `<...>` is ignored because it belongs to a
/// generic argument, not to the top-level path.
///
/// Examples:
/// - `"domain::OrderPlaced"` → `Some(("domain", "OrderPlaced"))`
/// - `"crate::domain::OrderPlaced"` → `Some(("crate::domain", "OrderPlaced"))`
/// - `"domain::models::OrderPlaced"` → `Some(("domain::models", "OrderPlaced"))`
/// - `"domain::Wrapper<crate::Foo>"` → `Some(("domain", "Wrapper<crate::Foo>"))`
/// - `"Option<domain::OrderPlaced>"` → `None` (all `::` are inside `<>`)
/// - `"OrderPlaced"` → `None` (no `::`)
pub(super) fn extract_top_level_layer_hint(type_ref: &str) -> Option<(&str, &str)> {
    // Determine where the top-level generic `<` begins (if any).
    // Any `::` at or after this position belongs to a generic argument.
    let top_level_lt = type_ref.find('<');

    // Find the rightmost `::` that appears strictly before `top_level_lt`
    // (or anywhere in the string if there is no `<`).
    let search_end = top_level_lt.unwrap_or(type_ref.len());
    let prefix = type_ref.get(..search_end)?;
    let sep_pos = prefix.rfind("::")?;

    let layer_hint = &type_ref[..sep_pos];
    let tail = &type_ref[sep_pos + 2..];
    Some((layer_hint, tail))
}

/// Given a path prefix (potentially multi-segment, e.g. `"domain::models"` or
/// `"crate::domain"`) and the set of known catalogues, return the catalogue
/// whose `LayerId` appears as a segment of `path_prefix`.
///
/// `path_prefix` is split on `"::"` and each segment is tested against every
/// known `LayerId` for an exact match. This approach is segment-exact: a layer
/// id `"domain"` matches segment `"domain"` but never `"subdomain"`.
///
/// Examples:
/// - `"domain"` → `domain` (single segment, exact match)
/// - `"domain::models"` → `domain` (first segment matches)
/// - `"crate::domain"` → `domain` (second segment matches)
/// - `"crate::domain::models"` → `domain` (middle segment matches)
///
/// When multiple known layers appear as segments (unusual), the first one
/// encountered in left-to-right segment order is returned.
///
/// Returns the matching layer's `LayerId` and `CatalogueDocument` when found.
pub(super) fn match_layer_prefix<'a>(
    path_prefix: &str,
    all_catalogues: &'a BTreeMap<LayerId, CatalogueDocument>,
) -> Option<(&'a LayerId, &'a CatalogueDocument)> {
    for segment in path_prefix.split("::") {
        if let Some(pair) = all_catalogues.iter().find(|(layer_id, _)| layer_id.as_ref() == segment)
        {
            return Some(pair);
        }
    }
    None
}

/// Returns `true` if a path rooted at `layer_id` and ending with `bare_name`
/// appears in `sig_type` as a boundary-delimited occurrence.
///
/// Matches both direct references (`"domain::OrderPlaced"`) and references with
/// intermediate sub-module segments (`"domain::models::OrderPlaced"`). After
/// finding `"{layer_id}::"` at a valid boundary, the function scans forward
/// through any number of `"{segment}::"` path components and checks that the
/// final identifier segment equals `bare_name` and is properly terminated.
///
/// Boundary rules:
/// - `layer_id` must start at an identifier boundary (start of string or
///   preceded by a non-alphanumeric, non-underscore character), preventing
///   `"subdomain::Foo"` from being detected as `"domain::Foo"`.
/// - `bare_name` must be terminated by a non-identifier character or
///   end-of-string, preventing `"domain::Order"` from matching
///   `"domain::OrderPlaced"`.
pub(super) fn layer_qualified_name_in_sig(sig_type: &str, layer_id: &str, bare_name: &str) -> bool {
    let prefix = format!("{layer_id}::");
    let mut haystack = sig_type;
    let mut offset = 0usize;
    while let Some(pos) = haystack.find(&prefix) {
        let abs_pos = offset + pos;
        // The `layer_id` must start at an identifier boundary.
        let boundary_before = if abs_pos == 0 {
            true
        } else {
            sig_type[..abs_pos]
                .chars()
                .next_back()
                .is_some_and(|c| !c.is_alphanumeric() && c != '_')
        };

        if boundary_before {
            // Scan forward from after "{layer_id}::" through optional
            // sub-module segments ("{ident}::") until we reach the final
            // identifier segment, then check it equals `bare_name`.
            let mut rest = &haystack[pos + prefix.len()..];
            loop {
                if let Some(after_name) = rest.strip_prefix(bare_name) {
                    // bare_name matched here — check termination.
                    let terminated =
                        after_name.chars().next().is_none_or(|c| !c.is_alphanumeric() && c != '_');
                    if terminated {
                        return true;
                    }
                }
                // Check if there is another ident:: segment to skip.
                // An ident segment consists of alphanumeric / underscore chars
                // followed by "::".
                let seg_end = rest
                    .char_indices()
                    .take_while(|(_, c)| c.is_alphanumeric() || *c == '_')
                    .last()
                    .map(|(i, c)| i + c.len_utf8())
                    .unwrap_or(0);
                if seg_end == 0 {
                    break; // no ident segment here — stop scanning
                }
                match rest.get(seg_end..) {
                    Some(after_seg) if after_seg.starts_with("::") => {
                        // There is a sub-module separator; advance past it.
                        rest = &after_seg[2..];
                    }
                    _ => break, // no "::" after segment — not a path continuation
                }
            }
        }

        // Advance past this prefix occurrence to avoid infinite loop.
        let advance = pos + prefix.len();
        offset += advance;
        haystack = &haystack[advance..];
    }
    false
}

/// Returns `true` if `bare_name` appears in `sig_type` at a position that is
/// NOT immediately preceded by `::` (i.e. an unqualified occurrence).
///
/// Uses the same boundary rules as [`bare_name_in_type_ref`] but excludes `:`
/// from the set of valid preceding characters, so a path-qualified reference
/// like `domain::Foo` does not count as an unqualified occurrence of `Foo`.
pub(super) fn bare_name_unqualified_in_sig(sig_type: &str, bare_name: &str) -> bool {
    if sig_type == bare_name {
        return true;
    }
    // Same as bare_name_in_type_ref but without `:` in start_chars.
    let start_chars: &[char] = &['<', ',', ' ', '(', '[', '&', '*', '+'];
    let end_chars: &[char] = &['>', ',', ' ', ')', ']', '<', ';', ':', '+'];
    let mut rest = sig_type;
    while let Some(pos) = rest.find(bare_name) {
        let before_ok =
            pos == 0 || rest[..pos].chars().next_back().is_some_and(|c| start_chars.contains(&c));
        let after_pos = pos + bare_name.len();
        let after_ok = after_pos == rest.len()
            || rest[after_pos..].chars().next().is_some_and(|c| end_chars.contains(&c));
        if before_ok && after_ok {
            return true;
        }
        if after_pos >= rest.len() {
            break;
        }
        rest = &rest[after_pos..];
    }
    false
}

/// Returns `true` if the signature token `sig_type` contains an occurrence of
/// `bare_name` that is attributable to `cat_layer_id`, given `target_layer_id`
/// as the layer being linted.
///
/// Attribution rules (applied in order):
/// 1. If `"{cat_layer_id}::{bare_name}"` appears in `sig_type` as a proper
///    boundary-delimited qualified reference (e.g. inside
///    `Vec<domain::OrderPlaced>`), the entry is explicitly attributed to this
///    layer. Return `true`.
/// 2. If a different known layer's qualified form (`"{other}::{bare_name}"`)
///    appears in `sig_type` (boundary-delimited) AND `bare_name` does NOT also
///    appear unqualified (without `::` immediately before it), the reference
///    belongs entirely to that other layer. Return `false` for this
///    `cat_layer_id`. If `bare_name` appears both qualified (with another layer)
///    AND unqualified in the same expression (e.g. `(Foo, domain::Foo)`), the
///    unqualified occurrence may still be attributable to `cat_layer_id` — fall
///    through to Rule 3.
/// 3. If `bare_name` appears in `sig_type` unqualified (no known layer prefix),
///    apply target-layer-first priority: if the target layer catalogue owns
///    `bare_name`, only the target layer's entry is attributed — other layers
///    are suppressed. If no layer owns it or the target layer does not carry it,
///    all catalogue entries with a matching bare name are candidates.
pub(super) fn sig_type_contains_entry(
    sig_type: &str,
    bare_name: &str,
    cat_layer_id: &LayerId,
    target_layer_id: &LayerId,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
) -> bool {
    // Rule 1: explicit attribution to this layer (boundary-aware).
    if layer_qualified_name_in_sig(sig_type, cat_layer_id.as_ref(), bare_name) {
        return true;
    }

    // Rule 2: explicitly attributed to a different known layer (boundary-aware).
    // Suppress only when bare_name has NO unqualified occurrence alongside the
    // qualified one.  If both a qualified `other::bare_name` and an unqualified
    // `bare_name` appear in the same expression, fall through to Rule 3 so the
    // unqualified occurrence can still be attributed via target-layer-first logic.
    let has_other_qualified = all_catalogues.keys().any(|other_layer| {
        other_layer != cat_layer_id
            && layer_qualified_name_in_sig(sig_type, other_layer.as_ref(), bare_name)
    });
    if has_other_qualified && !bare_name_unqualified_in_sig(sig_type, bare_name) {
        return false;
    }

    // Rule 3: unqualified bare-name match — apply target-layer-first priority.
    if !bare_name_in_type_ref(sig_type, bare_name) {
        return false;
    }

    // If the target layer owns this bare name, only the target layer entry is
    // attributed for unqualified references.
    let target_owns = all_catalogues
        .get(target_layer_id)
        .and_then(|cat| find_in_catalogue(cat, bare_name))
        .is_some();

    if target_owns {
        // Only the target layer entry matches an unqualified reference.
        cat_layer_id == target_layer_id
    } else {
        // Target layer doesn't own it.  Treat any non-target catalogue that
        // declares this bare name as a candidate.  This is the conservative
        // choice: an unqualified reference that exists in multiple non-target
        // catalogues is inherently ambiguous, and `NoRoleInMethodSignature`
        // should flag it if ANY candidate has a forbidden role.  Authors that
        // need to reference only the permitted version must use an explicit
        // layer-qualified form (e.g. `permitted_layer::Foo`).
        all_catalogues.get(cat_layer_id).and_then(|cat| find_in_catalogue(cat, bare_name)).is_some()
    }
}

/// Strips generic arguments from a type name, returning the outer type identifier.
///
/// Examples:
/// - `"Result<OrderPlaced>"` → `"Result"`
/// - `"Paged<domain::OrderPlaced>"` → `"Paged"`
/// - `"OrderPlaced"` → `"OrderPlaced"` (unchanged)
pub(super) fn strip_generics(type_name: &str) -> &str {
    match type_name.find('<') {
        Some(pos) => &type_name[..pos],
        None => type_name,
    }
}

/// Extracts the first generic argument from a generic type expression.
///
/// Scans past the outer `<` and returns the text up to the first top-level
/// `,` or `>`, trimmed of whitespace.
///
/// "Top-level" here means not nested inside any `<...>`, `(...)`, or `[...]`
/// delimiters, so commas inside tuple types or array lengths do not split the
/// first argument prematurely.
///
/// Examples:
/// - `"Result<OrderPlaced>"` → `Some("OrderPlaced")`
/// - `"Paged<domain::OrderPlaced>"` → `Some("domain::OrderPlaced")`
/// - `"Map<K, V>"` → `Some("K")` (first argument only)
/// - `"Result<(OrderPlaced, E)>"` → `Some("(OrderPlaced, E)")` (tuple not split)
/// - `"OrderPlaced"` → `None`
pub(super) fn extract_first_generic_arg(type_str: &str) -> Option<&str> {
    let open = type_str.find('<')? + 1;
    let inner = type_str.get(open..)?;
    // Walk forward tracking nesting depth for <>, (), and [] to find the first
    // top-level , or >.  Commas inside parenthesized tuples or array lengths
    // are not at depth 0 and therefore do not terminate the first argument.
    let mut angle: usize = 0;
    let mut paren: usize = 0;
    let mut bracket: usize = 0;
    for (i, c) in inner.char_indices() {
        match c {
            '<' => angle += 1,
            '>' if angle == 0 && paren == 0 && bracket == 0 => return Some(inner[..i].trim()),
            '>' if angle > 0 => angle -= 1,
            '(' => paren += 1,
            ')' if paren > 0 => paren -= 1,
            '[' => bracket += 1,
            ']' if bracket > 0 => bracket -= 1,
            ',' if angle == 0 && paren == 0 && bracket == 0 => return Some(inner[..i].trim()),
            _ => {}
        }
    }
    // Malformed (no closing >): return the whole inner text trimmed
    Some(inner.trim())
}

/// Look up the role of a type (or trait) reference across all available
/// catalogues.
///
/// `type_ref` may be a bare name (`"OrderPlaced"`), a path-qualified name
/// (`"domain::OrderPlaced"`, `"domain::models::OrderPlaced"`), or a generic
/// expression (`"domain::Paged<X>"`, `"Vec<OrderPlaced>"`).
///
/// Resolution strategy for generic `TypeRef`s:
/// - The outer type name (before `<`) is checked first.
/// - If the outer type is not found in any catalogue, the first generic
///   argument is resolved recursively as a fallback. This handles common
///   transparent wrapper types such as `Result<T>`, `Option<T>`, `Vec<T>`,
///   and `Paged<T>` where the role-bearing type is the inner argument.
///
/// Resolution order:
/// 1. If `type_ref` has a top-level layer qualifier (no `<` before `::`) AND
///    that qualifier, or any prefix of it, matches a known `LayerId` via
///    `match_layer_prefix`, look up the **last path segment** of the tail in
///    that catalogue authoritatively. Multi-segment sub-paths like
///    `"domain::models::OrderPlaced"` are handled: `layer_hint = "domain::models"`
///    matches the `"domain"` catalogue and the tail `"OrderPlaced"` is looked
///    up therein. If the outer type is not found but the tail is generic, the
///    first generic argument is resolved recursively.
/// 2. If `type_ref` has a top-level layer qualifier but the qualifier does NOT
///    match any known `LayerId` (e.g. `crate`, `std::vec`), resolve the outer
///    type name of the tail via bare-name search; if that fails and the tail
///    is generic, fall through to recursive resolution of the first argument.
/// 3. Otherwise (`type_ref` is a bare name or unqualified generic), resolve
///    the outer type name via target-layer-first bare-name search; if that
///    fails and the expression is generic, resolve the first argument
///    recursively.
///
/// Returns `None` when the referenced type cannot be found in any catalogue.
pub(super) fn resolve_type_role(
    type_ref: &str,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
) -> Option<RoleKind> {
    // Strip leading reference / pointer sigils (`&`, `&mut `, `*`, `*mut `, `*const `)
    // before parsing: `&domain::OrderPlaced` must be treated the same as
    // `domain::OrderPlaced`, not as a reference to an unknown layer `&domain`.
    let type_ref = strip_ref_sigils(type_ref);

    // Step 1 / 2: handle top-level path-qualified identifiers.
    if let Some((layer_hint, tail)) = extract_top_level_layer_hint(type_ref) {
        // Check whether any known LayerId is a prefix of (or equal to) layer_hint.
        // This handles multi-segment sub-paths like "domain::models" → "domain".
        if let Some((_, cat)) = match_layer_prefix(layer_hint, all_catalogues) {
            // Step 1: known-layer qualifier — look up the outer type name in
            // that catalogue authoritatively (no cross-layer fallback).
            // `tail` is the final segment after the last `::`, so it is already
            // the bare type name (possibly with generic arguments).
            let outer = strip_generics(tail);
            if let Some(role) = find_in_catalogue(cat, outer) {
                return Some(role);
            }
            // Outer type not in the known-layer catalogue: if the tail is a
            // generic expression (e.g. `Result<OrderPlaced>`), the wrapper is
            // likely a transparent stdlib type. Recursively resolve the first
            // generic argument across all catalogues.
            return extract_first_generic_arg(tail)
                .and_then(|arg| resolve_type_role(arg, all_catalogues, target_layer_id));
        }

        // Step 2: unknown-layer qualifier — resolve outer type via bare-name
        // search; on failure fall through to recursive inner-arg resolution.
        let outer = strip_generics(tail);
        if let Some(role) = resolve_bare_name(outer, all_catalogues, target_layer_id) {
            return Some(role);
        }
        return extract_first_generic_arg(tail)
            .and_then(|arg| resolve_type_role(arg, all_catalogues, target_layer_id));
    }

    // Step 3: bare name or unqualified generic — resolve outer type; on
    // failure fall through to recursive inner-arg resolution.
    let outer = strip_generics(type_ref);
    if let Some(role) = resolve_bare_name(outer, all_catalogues, target_layer_id) {
        return Some(role);
    }
    extract_first_generic_arg(type_ref)
        .and_then(|arg| resolve_type_role(arg, all_catalogues, target_layer_id))
}

/// Search for `bare_name` across all catalogues.
///
/// Resolution rules:
/// 1. **Target layer first**: the catalogue for `target_layer_id` is checked
///    first; if it owns `bare_name`, that role is returned immediately.
///    This unambiguously resolves references when the target layer is the sole
///    owner.
/// 2. **Non-target layer consensus**: when `bare_name` is not owned by the
///    target layer, all other catalogues are scanned. If exactly one non-target
///    catalogue declares `bare_name`, its role is returned. If multiple
///    non-target catalogues declare the same `bare_name` **with the same
///    role**, that role is returned (unambiguous consensus). If they declare it
///    with **different roles** (genuine ambiguity), `None` is returned so that
///    `ReferencedRoleConstraint` callers report a violation rather than
///    silently using an arbitrarily ordered result.  Authors should use
///    explicit layer-qualified references to resolve ambiguity.
pub(super) fn resolve_bare_name(
    bare_name: &str,
    all_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
    target_layer_id: &LayerId,
) -> Option<RoleKind> {
    // Rule 1: target layer.
    if let Some(cat) = all_catalogues.get(target_layer_id) {
        if let Some(role) = find_in_catalogue(cat, bare_name) {
            return Some(role);
        }
    }

    // Rule 2: non-target consensus.  Collect all roles from non-target catalogues.
    let mut consensus: Option<RoleKind> = None;
    let mut ambiguous = false;
    for (layer_id, cat) in all_catalogues {
        if layer_id == target_layer_id {
            continue;
        }
        if let Some(role) = find_in_catalogue(cat, bare_name) {
            match consensus {
                None => consensus = Some(role),
                Some(prev) if prev == role => {} // same role, still consensus
                Some(_) => {
                    // Conflicting roles — genuine ambiguity.
                    ambiguous = true;
                    break;
                }
            }
        }
    }
    if ambiguous { None } else { consensus }
}

/// Search a single `CatalogueDocument` for a type or trait entry with the
/// given `bare_name`. Returns the [`RoleKind`] if found.
///
/// Entries with `action: Delete` are excluded from lookup so that
/// fail-closed semantics are preserved: a delete-marked entry does not
/// satisfy role checks that require the type to be present.
pub(super) fn find_in_catalogue(
    catalogue: &CatalogueDocument,
    bare_name: &str,
) -> Option<RoleKind> {
    // Check type entries first, excluding delete-action entries.
    if let Some((_, entry)) = catalogue
        .types
        .iter()
        .find(|(tn, entry)| tn.as_str() == bare_name && entry.action != ItemAction::Delete)
    {
        return Some(entry_role_kind(entry));
    }
    // Check trait entries, excluding delete-action entries.
    catalogue
        .traits
        .iter()
        .find(|(tn, entry)| tn.as_str() == bare_name && entry.action != ItemAction::Delete)
        .map(|(_, e)| RoleKind::from_contract_role(&e.role))
}
