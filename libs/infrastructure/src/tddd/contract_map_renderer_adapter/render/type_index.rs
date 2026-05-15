//! Same-catalogue `TypeRef` → mermaid node-id resolution for the T006 renderer.
//!
//! `TypeIndex` maps `(crate_name, bare_type_name)` pairs to mermaid subgraph ids.
//! Resolution is scoped to the caller's crate (same-catalogue semantics). Types
//! and traits are stored in separate maps to prevent a same-name collision from
//! silently replacing the type node id with the trait node id.

use std::collections::HashMap;

use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::catalogue_v2::identifiers::{CrateName, TypeRef};

use super::super::{trait_node_id, type_node_id};

// ---------------------------------------------------------------------------
// TypeIndex — same-catalogue TypeRef → node_id resolution (T006 scope)
// ---------------------------------------------------------------------------

/// Maps a `(crate_name, bare_type_name)` pair to the mermaid subgraph id
/// of the corresponding TypeEntry or TraitEntry.
///
/// Same-catalogue resolution only (T006 scope): a `TypeRef` in crate A only
/// resolves to an entry also declared in crate A. Cross-catalogue resolution is
/// deferred to T008. Unresolved refs produce no edge (silent skip).
///
/// Resolution heuristic: strip leading `&`, `&mut`, and common generic wrappers
/// (`Box<`, `Arc<`, `Rc<`, `Option<`, `Result<`, `Vec<`) to extract the
/// base name, then look it up in the index scoped to the caller's crate.
///
/// Types and traits are stored in separate maps so that a crate declaring both
/// a type `Foo` and a trait `Foo` does not produce an id collision. Resolution
/// checks the type map first, then falls back to the trait map.
pub(super) struct TypeIndex {
    /// Maps `(crate_name_str, bare_type_name_str)` → mermaid subgraph id for TypeEntry.
    types: HashMap<(String, String), String>,
    /// Maps `(crate_name_str, bare_trait_name_str)` → mermaid subgraph id for TraitEntry.
    traits: HashMap<(String, String), String>,
}

impl TypeIndex {
    /// Builds a `TypeIndex` from a slice of `CatalogueDocument` references.
    ///
    /// Accepts `&[&CatalogueDocument]` so that callers can pass a pre-filtered
    /// reference slice (e.g. catalogues from rendered layers only) without an extra
    /// allocation. `TypeEntry` names and `TraitEntry` names are stored in separate
    /// maps under the owning document's `crate_name` to preserve same-catalogue
    /// semantics (T006) and prevent same-name type/trait id collisions.
    ///
    /// `render_mermaid` restricts the input to rendered layers so that TypeRef
    /// resolution never produces node ids for entries that were not emitted, avoiding
    /// dangling Mermaid edges.
    pub(super) fn build(catalogues: &[&CatalogueDocument]) -> Self {
        let mut types = HashMap::new();
        let mut traits = HashMap::new();
        for doc in catalogues {
            let layer: &LayerId = &doc.layer;
            let crate_name: &CrateName = &doc.crate_name;
            let crate_key = crate_name.as_str().to_owned();
            for name in doc.types.keys() {
                let id = type_node_id(layer, crate_name, name);
                types.insert((crate_key.clone(), name.as_str().to_owned()), id);
            }
            for name in doc.traits.keys() {
                let id = trait_node_id(layer, crate_name, name);
                traits.insert((crate_key.clone(), name.as_str().to_owned()), id);
            }
        }
        Self { types, traits }
    }

    /// Resolves a `TypeRef` string to a mermaid subgraph id, or `None` if not found.
    ///
    /// Resolution is scoped to `caller_crate`: only entries declared in the same
    /// catalogue document (same crate) are returned. Cross-crate and cross-catalogue
    /// refs are silently skipped (T008 scope).
    ///
    /// Strips leading reference markers (`&`, `&mut`) and common generic wrappers
    /// (`Box<`, `Arc<`, `Rc<`, `Option<`, `Result<`, `Vec<`) to extract the inner
    /// base name before lookup. Cross-crate refs (containing `::`) are also skipped.
    ///
    /// Type entries take precedence over trait entries when both share the same name.
    pub(super) fn resolve(&self, type_ref: &TypeRef, caller_crate: &CrateName) -> Option<&str> {
        let base = extract_base_name(type_ref.as_str())?;
        // Cross-crate refs (contain "::") are silently skipped (T008).
        if base.contains("::") {
            return None;
        }
        let key = (caller_crate.as_str().to_owned(), base.to_owned());
        // Type entries take precedence; fall back to trait entries.
        self.types.get(&key).or_else(|| self.traits.get(&key)).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// TypeRef name extraction helpers
// ---------------------------------------------------------------------------

/// Extracts the innermost base type name from a generics-inclusive type ref string.
///
/// Strips `&` / `&mut` prefixes, then repeatedly unwraps common generic wrappers
/// (`Box<`, `Arc<`, `Rc<`, `Option<`, `Result<`, `Vec<`) to get to the contained
/// base type. For example `Vec<UserId>` → `UserId`, `Option<Arc<Foo>>` → `Foo`,
/// `Option<&Foo>` → `Foo`, `Result<UserId, DomainError>` → `UserId`.
/// Finally strips any remaining trailing `<...>` generics.
///
/// Returns `None` for unit type `"()"`, empty strings, or types that consist only
/// of wrapper prefixes with no resolvable inner name.
pub(super) fn extract_base_name(s: &str) -> Option<&str> {
    let s = s.trim();
    if s.is_empty() || s == "()" {
        return None;
    }
    // Strip leading reference markers (first pass — handles `&Foo` and `&mut Foo`).
    let s = strip_leading_refs(s);
    if s.is_empty() || s == "()" {
        return None;
    }
    // Unwrap common single-type generic wrappers to reach the inner type.
    // Order matters: strip from outermost to innermost. The inner value may itself
    // begin with a reference marker (e.g. `Option<&Foo>` → `&Foo`), so strip refs
    // again after wrapper unwrapping.
    let s = unwrap_generic_wrappers(s);
    let s = strip_leading_refs(s);
    if s.is_empty() || s == "()" {
        return None;
    }
    // Strip any remaining trailing `<...>` generics to get the bare name.
    let base = s.split('<').next().unwrap_or(s).trim();
    if base.is_empty() {
        return None;
    }
    Some(base)
}

/// Strips a leading `&mut` or `&` reference marker (with optional trailing space).
fn strip_leading_refs(s: &str) -> &str {
    let s = s.strip_prefix("&mut ").unwrap_or(s);
    let s = s.strip_prefix("&mut").unwrap_or(s);
    let s = s.strip_prefix("& ").unwrap_or(s);
    let s = s.strip_prefix('&').unwrap_or(s);
    s.trim_start()
}

/// Common single-type generic wrapper prefixes that can be stripped to reach
/// the inner type. For `Vec<UserId>` the wrapper is `Vec<` and inner is `UserId`.
const GENERIC_WRAPPERS: &[&str] = &["Box<", "Arc<", "Rc<", "Option<", "Result<", "Vec<", "Pin<"];

/// Strips leading generic wrapper prefixes and their matching `>` suffixes.
///
/// Repeatedly peels one wrapper at a time until no more wrappers match.
/// For `Option<Arc<Foo>>` → `Arc<Foo>` → `Foo`.
/// For `Result<UserId, DomainError>` → `UserId` (takes only the first type arg).
/// For `Vec<UserId>` → `UserId`.
/// For `Option<Result<UserId, Error>>` → `Result<UserId, Error>` → `UserId`.
///
/// After stripping a wrapper prefix, exactly one trailing `>` is removed (the
/// matching close for the stripped wrapper). The first type argument is then
/// found by scanning for the first `,` at bracket depth 0, so that nested
/// generic arguments like `Foo<A, B>` are treated as a single argument unit.
fn unwrap_generic_wrappers(s: &str) -> &str {
    let mut s = s;
    loop {
        let mut matched = false;
        for wrapper in GENERIC_WRAPPERS {
            if let Some(inner) = s.strip_prefix(wrapper) {
                // Remove exactly one trailing `>` — the matching close for the wrapper
                // we just stripped. `trim_end_matches` would remove ALL trailing `>`s,
                // collapsing nested closing brackets and losing type information.
                let inner = inner.strip_suffix('>').unwrap_or(inner).trim();
                // Find the first type argument at bracket depth 0. A plain `split(',')`
                // would split inside nested generics like `Foo<A, B>`, so we use a
                // depth-aware scan instead.
                let first_arg = first_top_level_arg(inner).trim();
                if !first_arg.is_empty() {
                    s = first_arg;
                    matched = true;
                    break;
                }
            }
        }
        if !matched {
            break;
        }
    }
    s
}

/// Returns the first comma-separated type argument at bracket depth 0.
///
/// Scans `s` character by character, tracking `<`/`>` nesting depth. The first
/// `,` found at depth 0 ends the first argument. If no such `,` exists, the
/// entire string is the first (and only) argument.
///
/// Example: `"Foo<A, B>, C"` → `"Foo<A, B>"` (the `,` inside `<>` is at
/// depth 1 and is skipped; the `,` after `>` is at depth 0).
fn first_top_level_arg(s: &str) -> &str {
    let mut depth: usize = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth = depth.saturating_sub(1);
            }
            ',' if depth == 0 => return &s[..i],
            _ => {}
        }
    }
    s
}
