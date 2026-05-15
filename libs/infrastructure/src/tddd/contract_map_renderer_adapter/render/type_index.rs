//! Same-catalogue `TypeRef` → mermaid node-id resolution for the T006 renderer.
//!
//! `TypeIndex` maps `(crate_name, bare_type_name)` pairs to mermaid subgraph ids.
//! Resolution is scoped to the caller's crate (same-catalogue semantics). Types
//! and traits are stored in separate maps to prevent a same-name collision from
//! silently replacing the type node id with the trait node id.

use std::collections::HashMap;

use domain::tddd::ContractMapRendererError;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::CatalogueDocument;
use domain::tddd::catalogue_v2::identifiers::{CrateName, TypeRef};
use domain::tddd::catalogue_v2::roles::ItemAction;

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
    /// Entries with `action: Delete` are excluded from the index so that deleted
    /// types cannot be resolved: a live entry that references a deleted type/trait
    /// by `TypeRef` must not produce a node id pointing to a deleted (and unrendered)
    /// subgraph node, which would create a dangling Mermaid edge.
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
            for (name, entry) in &doc.types {
                // Skip deletion records — deleted types must not resolve to a node id
                // because they are not rendered in the contract map. Resolving them
                // would produce a dangling Mermaid edge pointing to an absent subgraph.
                if entry.action == ItemAction::Delete {
                    continue;
                }
                let id = type_node_id(layer, crate_name, name);
                types.insert((crate_key.clone(), name.as_str().to_owned()), id);
            }
            for (name, entry) in &doc.traits {
                if entry.action == ItemAction::Delete {
                    continue;
                }
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
    ///
    /// # Errors
    ///
    /// Returns [`ContractMapRendererError::RenderFailed`] when the `TypeRef` string has
    /// mismatched generic angle brackets (e.g. `Result<UserId` with no closing `>`). Such
    /// inputs indicate a malformed catalogue entry and are fail-closed per CN-03.
    /// Well-formed-but-unresolvable refs (e.g. a type not in any catalogue) return `Ok(None)`.
    pub(super) fn resolve(
        &self,
        type_ref: &TypeRef,
        caller_crate: &CrateName,
    ) -> Result<Option<&str>, ContractMapRendererError> {
        // Fail-closed: validate bracket balance before extraction (CN-03).
        validate_type_ref(type_ref.as_str())?;

        let Some(base) = extract_base_name(type_ref.as_str()) else {
            return Ok(None);
        };
        // Cross-crate refs (contain "::") are silently skipped (T008).
        if base.contains("::") {
            return Ok(None);
        }
        let key = (caller_crate.as_str().to_owned(), base.to_owned());
        // Type entries take precedence; fall back to trait entries.
        Ok(self.types.get(&key).or_else(|| self.traits.get(&key)).map(|s| s.as_str()))
    }
}

// ---------------------------------------------------------------------------
// TypeRef validation helper
// ---------------------------------------------------------------------------

/// Checks that a `TypeRef` string has properly nested and well-formed generic angle
/// brackets.
///
/// The following conditions are checked (any failure returns `RenderFailed`, CN-03):
///
/// 1. **No premature close**: a `>` must never appear when the generic depth is zero,
///    unless it is part of a `->` return-type arrow. A string like `Foo>Bar<` (equal
///    counts, but `>` comes before any `<`) is rejected. `->` arrows (common in function-
///    pointer TypeRefs such as `for<'a> fn(&'a T) -> T` or `Vec<fn(u32) -> u32>`) are
///    recognised at any depth by checking that the immediately preceding non-whitespace
///    character is `-`; such `>` is skipped without closing a generic block.
///
/// 2. **Count balance**: all opened `<` must be closed by the end of the string. A
///    string like `Result<UserId` (one `<`, zero `>`) fails here.
///
/// 3. **No mismatched or unmatched parentheses / square brackets**: a `(`/`[` must be
///    closed by the matching `)` / `]` before the surrounding `>` is reached (inside a
///    `<...>` block) or before end-of-string (at top level). Kind-mismatched closes
///    (`Foo<[T)>`) and stray closers without an opener (`Vec<T>)`) are both rejected.
///    Well-formed complex refs such as `for<'a> fn(&'a T) -> T` pass: their `()` is
///    balanced at top level, and `Vec<fn(u32) -> u32>` passes because the `->` `>` is
///    recognised as an arrow, not a generic close.
///
/// `TypeRef::new` only rejects empty strings, so all checks must live here as
/// part of the fail-closed render pipeline.
///
/// Well-formed-but-unresolvable refs (e.g. a type not present in any catalogue)
/// are not flagged by this check and continue to produce `Ok(None)` from `resolve`.
///
/// # Errors
///
/// Returns `RenderFailed` when any of the above conditions is violated.
pub(super) fn validate_type_ref(s: &str) -> Result<(), ContractMapRendererError> {
    let s = s.trim();

    // Stack of per-generic-block state. Each element corresponds to one open `<...>`.
    // `inner_brackets` is a stack of open `(`/`[` seen inside this `<...>` block;
    // each closing bracket must match the most-recently pushed opener.
    // `prev_nonws` is the last non-whitespace character seen inside this block.
    struct BlockState {
        inner_brackets: Vec<char>, // stack of unmatched `(`/`[` inside this `<...>`
        prev_char: Option<char>,   // immediately preceding character (for `->` detection)
    }
    // Immediately preceding character at top level (outside any `<...>`).
    // Used to detect `->` return-type arrows: `>` is an arrow only when preceded
    // by `-` with no intervening whitespace (i.e. `prev_char == Some('-')`).
    let mut top_prev_char: Option<char> = None;
    // Top-level bracket stack: tracks `(`/`[` opened outside any `<...>` block.
    // A stray `)` or `]` at top level (no matching opener) is a malformed TypeRef.
    // `Vec<T>)` is rejected because the `)` has no opening `(` at top level.
    // `for<'a> fn(&'a T) -> T` passes because the `(` and `)` are balanced at top level.
    let mut top_brackets: Vec<char> = Vec::new();
    let mut stack: Vec<BlockState> = Vec::new();

    for ch in s.chars() {
        match ch {
            '<' => {
                // Update the enclosing context's prev_char to '<' before pushing the new block.
                if let Some(outer) = stack.last_mut() {
                    outer.prev_char = Some('<');
                } else {
                    top_prev_char = Some('<');
                }
                stack.push(BlockState { inner_brackets: Vec::new(), prev_char: None });
            }
            '>' if !stack.is_empty() => {
                // `->` arrows (e.g. `Vec<fn(u32) -> u32>`) can appear inside a generic
                // block. Detect them by checking the block's `prev_char`: `>` is treated
                // as a `->` arrow only when the immediately preceding character is `-`
                // (no intervening whitespace). `Foo<Bar - > Baz>` is rejected because
                // the space before `>` means `prev_char` is ` `, not `-`.
                let is_arrow = stack.last().is_some_and(|b| b.prev_char == Some('-'));
                if is_arrow {
                    // Treat as part of `->`: update prev_char of the enclosing block.
                    if let Some(block) = stack.last_mut() {
                        block.prev_char = Some('>');
                    }
                } else {
                    if let Some(block) = stack.last() {
                        if !block.inner_brackets.is_empty() {
                            // Open `(`/`[` not closed before this `>`.
                            return Err(ContractMapRendererError::RenderFailed {
                                reason: format!(
                                    "malformed TypeRef \"{s}\": unmatched inner bracket before '>'"
                                ),
                            });
                        }
                    }
                    stack.pop();
                    // After closing a nested block, update the enclosing block's prev_char.
                    if let Some(outer) = stack.last_mut() {
                        outer.prev_char = Some('>');
                    } else {
                        top_prev_char = Some('>');
                    }
                }
            }
            '>' => {
                // `>` at depth 0 (stack is empty). Accept `->` arrows (immediately preceded
                // by `-`); reject all other bare `>` as unbalanced angle brackets.
                if top_prev_char == Some('-') {
                    top_prev_char = Some('>');
                } else {
                    return Err(ContractMapRendererError::RenderFailed {
                        reason: format!(
                            "malformed TypeRef \"{s}\": unbalanced angle brackets \
                             ('>' at depth < 0)"
                        ),
                    });
                }
            }
            '(' | '[' if !stack.is_empty() => {
                if let Some(block) = stack.last_mut() {
                    block.inner_brackets.push(ch);
                    block.prev_char = Some(ch);
                }
            }
            ')' | ']' if !stack.is_empty() => {
                if let Some(block) = stack.last_mut() {
                    let expected_open = if ch == ')' { '(' } else { '[' };
                    match block.inner_brackets.last() {
                        Some(&open) if open == expected_open => {
                            block.inner_brackets.pop();
                            block.prev_char = Some(ch);
                        }
                        Some(_) => {
                            // Mismatched bracket kind: e.g. `[` closed by `)`.
                            return Err(ContractMapRendererError::RenderFailed {
                                reason: format!(
                                    "malformed TypeRef \"{s}\": mismatched bracket inside generic \
                                     (expected closing for previous opener)"
                                ),
                            });
                        }
                        None => {
                            // Unmatched closing bracket with no opener inside this block.
                            return Err(ContractMapRendererError::RenderFailed {
                                reason: format!(
                                    "malformed TypeRef \"{s}\": unmatched closing bracket inside \
                                     generic"
                                ),
                            });
                        }
                    }
                }
            }
            '(' | '[' if stack.is_empty() => {
                // Top-level opener: track it so we can detect stray closers.
                top_brackets.push(ch);
                top_prev_char = Some(ch);
            }
            ')' | ']' if stack.is_empty() => {
                // Top-level closer: must match the most-recently opened top-level bracket.
                let expected_open = if ch == ')' { '(' } else { '[' };
                match top_brackets.last() {
                    Some(&open) if open == expected_open => {
                        top_brackets.pop();
                        top_prev_char = Some(ch);
                    }
                    Some(_) => {
                        return Err(ContractMapRendererError::RenderFailed {
                            reason: format!(
                                "malformed TypeRef \"{s}\": mismatched bracket outside generic \
                                 (expected closing for previous opener)"
                            ),
                        });
                    }
                    None => {
                        // Stray closing bracket at top level with no opener.
                        return Err(ContractMapRendererError::RenderFailed {
                            reason: format!(
                                "malformed TypeRef \"{s}\": unmatched closing bracket outside \
                                 generic (e.g. stray ')' or ']')"
                            ),
                        });
                    }
                }
            }
            c if !stack.is_empty() => {
                if let Some(block) = stack.last_mut() {
                    block.prev_char = Some(c);
                }
            }
            c => {
                // Top-level character (outside any generic block).
                top_prev_char = Some(c);
            }
        }
    }
    if !stack.is_empty() {
        return Err(ContractMapRendererError::RenderFailed {
            reason: format!(
                "malformed TypeRef \"{s}\": unbalanced angle brackets \
                 ({} unclosed '<')",
                stack.len()
            ),
        });
    }
    if !top_brackets.is_empty() {
        return Err(ContractMapRendererError::RenderFailed {
            reason: format!(
                "malformed TypeRef \"{s}\": unclosed bracket outside generic \
                 ({} unclosed '(' or '[')",
                top_brackets.len()
            ),
        });
    }
    Ok(())
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
