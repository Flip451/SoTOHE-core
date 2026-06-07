//! Details-column renderers and supporting helpers for the type catalogue view.
//!
//! Contains `for_type_local_bare_name`, `trait_ref_short_name`,
//! `v3_type_entry_details`, `v3_trait_entry_details`, and
//! `v3_function_entry_details`, extracted from `type_catalogue_render` to keep
//! the parent module under the 700-line production-code limit.

use domain::tddd::catalogue_v2::entries::{FunctionEntry, TraitEntry, TypeEntry};
use domain::tddd::catalogue_v2::{DataRole, TypeKindV2};

/// Returns the bare local type name from a `for_type` string, or `None` if the
/// `for_type` refers to an external type.
///
/// "Local" means the type belongs to the current crate: it has no path prefix, or
/// its path prefix is one of the Rust path keywords (`crate::`, `self::`, `super::`)
/// or the self-crate name passed as `self_crate_name`.
///
/// Per ADR `2026-05-20-0048` D2, the catalogue convention is:
/// - local type → bare short name (e.g. `"SelfType"`) or local-path-qualified form
/// - external type → crate-prefix fully-qualified path (e.g. `"std::vec::Vec<i32>"`)
///
/// A bare name (no `::`) is therefore always treated as a local type; callers must
/// use qualified paths to refer to external types.  The caller matches the returned
/// name against the actual `TypeEntry` names in the catalogue: if no `TypeEntry` is
/// named by the returned bare name, the impl simply does not appear in any row.
/// This is the intended outcome: external self-type impls (e.g.
/// `impl MyTrait for std::vec::Vec<i32>`) must be declared with a crate-prefixed
/// `for_type` and have no corresponding local `TypeEntry` row to appear in.
///
/// **Design note (bare prelude names):** The A-codec (`parse_type_ref_str` /
/// `type_ref_parser::STD_PRELUDE_TYPES`) resolves a bare `"Vec"` to
/// `std::vec::Vec` when no local type with that name exists.  This function does not
/// replicate that expansion — it always returns `Some("Vec")` for a bare `"Vec"`.
/// This means both paths agree when a local `TypeEntry` named `"Vec"` exists: the
/// A-codec resolves to the local type, and this function returns `Some("Vec")` which
/// matches the entry name.  When no local `"Vec"` TypeEntry exists, the A-codec
/// treats the bare name as an external type (std prelude), while this function still
/// returns `Some("Vec")` which then fails to match any row — the impl is not shown
/// in the rendered view.  Both outcomes are consistent with ADR D2: for the external
/// case, authors should write the qualified path (e.g. `"std::vec::Vec<i32>"`),
/// which this function correctly maps to `None` (external).
///
/// Generic arguments are stripped before comparison.
///
/// Examples (with `self_crate_name = "my_crate"`):
/// - `"crate::MyAdapter"` → `Some("MyAdapter")`
/// - `"self::MyAdapter"` → `Some("MyAdapter")`
/// - `"my_crate::MyAdapter"` → `Some("MyAdapter")`
/// - `"MyAdapter<T>"` → `Some("MyAdapter")`
/// - `"MyAdapter"` → `Some("MyAdapter")` (bare name → always local per ADR D2)
/// - `"std::vec::Vec<i32>"` → `None` (external — crate-prefixed)
/// - `"other_crate::Foo"` → `None` (external — crate-prefixed)
pub(super) fn for_type_local_bare_name<'a>(
    for_type: &'a str,
    self_crate_name: &str,
) -> Option<&'a str> {
    // Strip generic arguments: everything from the first `<` onward.
    let angle_pos = for_type.find('<').unwrap_or(for_type.len());
    let base = &for_type[..angle_pos];
    match base.find("::") {
        None => {
            // No path separator → bare name, always local.
            Some(base)
        }
        Some(colon_pos) => {
            let prefix = &base[..colon_pos];
            if matches!(prefix, "crate" | "self" | "super") || prefix == self_crate_name {
                // Local path keyword or self-crate name — strip the prefix and return the last segment.
                let rest = &base[colon_pos + 2..];
                Some(rest.rsplit("::").next().unwrap_or(rest))
            } else {
                // Unknown prefix → treat as external crate, skip.
                None
            }
        }
    }
}

/// Returns the short display name for a `trait_ref` string (ADR `2026-05-20-0048` D2).
///
/// Strips any leading crate-path prefix (`core::convert::`) and keeps the
/// last path segment together with any generic arguments.
///
/// Examples:
/// - `"core::convert::From<MyError>"` → `"From<MyError>"`
/// - `"std::fmt::Display"` → `"Display"`
/// - `"MyTrait"` → `"MyTrait"`
pub(super) fn trait_ref_short_name(trait_ref: &str) -> &str {
    // Split off generic args at the first `<`.
    let angle_pos = trait_ref.find('<').unwrap_or(trait_ref.len());
    let base = &trait_ref[..angle_pos];
    // The last `::` before `<` separates the crate path from the trait name.
    let start = base.rfind("::").map(|p| p + 2).unwrap_or(0);
    &trait_ref[start..]
}

/// Renders the Details column for a v3 `TypeEntry`.
///
/// - `Typestate` (PlainStruct with `typestate: Some(_)`): transition methods `→ m1, → m2`.
///   An empty transition list renders as `∅ (terminal)`.
/// - `Enum`: variant names joined by `, ` — or `—` when no variants declared.
/// - `SecondaryAdapter` (DataRole): declared trait impls `impl Trait1, impl Trait2` — or `—`.
///   Trait impls are sourced from the document-level `trait_impls` list (ADR `2026-05-20-0048`
///   D1) and filtered by `for_type == type_name`.
/// - All other kinds: `—` (existence-check only).
pub(super) fn v3_type_entry_details(
    entry: &TypeEntry,
    type_name: &str,
    self_crate_name: &str,
    doc_trait_impls: &[domain::tddd::catalogue_v2::TraitImplDeclV2],
) -> String {
    match &entry.kind {
        TypeKindV2::Struct(sk) if sk.typestate.is_some() => {
            let Some(ts) = sk.typestate.as_ref() else {
                return "\u{2014}".to_owned(); // unreachable: guard is_some above
            };
            let methods = ts.transitions().transition_methods();
            if methods.is_empty() {
                "\u{2205} (terminal)".to_owned() // ∅ (terminal)
            } else {
                methods
                    .iter()
                    .map(|m| format!("\u{2192} {}", m.as_str())) // → method
                    .collect::<Vec<_>>()
                    .join(", ")
            }
        }
        TypeKindV2::Enum { variants } => {
            if variants.is_empty() {
                "\u{2014}".to_owned() // —
            } else {
                variants.iter().map(|v| v.name.as_str()).collect::<Vec<_>>().join(", ")
            }
        }
        _ if matches!(entry.role, DataRole::SecondaryAdapter) => {
            // SecondaryAdapter: render declared trait impls from the document-level
            // `trait_impls` list (ADR `2026-05-20-0048` D1), filtering by `for_type`.
            // Only local types (crate::, self::, super::, self-crate-qualified, or bare name)
            // are matched; external self types (e.g. `std::vec::Vec`) are skipped.
            let impls: Vec<String> = doc_trait_impls
                .iter()
                .filter(|ti| {
                    for_type_local_bare_name(ti.for_type.as_str(), self_crate_name)
                        == Some(type_name)
                })
                .map(|ti| format!("impl {}", trait_ref_short_name(ti.trait_ref.as_str())))
                .collect();
            if impls.is_empty() {
                "\u{2014}".to_owned() // —
            } else {
                impls.join(", ")
            }
        }
        _ => "\u{2014}".to_owned(), // — (existence-check only)
    }
}

/// Renders the Details column for a v3 `TraitEntry`.
///
/// - `SecondaryPort` / `ApplicationService` / `SpecificationPort`: method signatures
///   joined by `, ` — or `—` when no methods declared.
pub(super) fn v3_trait_entry_details(entry: &TraitEntry) -> String {
    if entry.methods.is_empty() {
        "\u{2014}".to_owned() // —
    } else {
        entry.methods.iter().map(|m| m.signature_string()).collect::<Vec<_>>().join(", ")
    }
}

/// Renders the Details column for a v3 `FunctionEntry`.
///
/// Emits the function signature: `[async ]fn(params) -> returns`.
pub(super) fn v3_function_entry_details(entry: &FunctionEntry) -> String {
    let async_prefix = if entry.is_async { "async " } else { "" };
    let params: Vec<String> =
        entry.params.iter().map(|p| format!("{}: {}", p.name, p.ty)).collect();
    let params_str = params.join(", ");
    format!("{}fn({}) -> {}", async_prefix, params_str, entry.returns)
}
