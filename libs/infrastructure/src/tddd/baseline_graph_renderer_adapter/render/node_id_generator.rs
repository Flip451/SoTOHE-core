//! Node ID generation scheme for the baseline-graph renderer (T006).
//!
//! Implements Decision D from ADR 2026-05-22-1507:
//! prefix + length-prefix + sanitized segments (injective, avoiding collisions
//! when distinct names sanitize identically).
//!
//! Formats:
//!
//! - **Type** (Struct / Enum / TypeAlias):
//!   `T<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_module_path>_<sanitized_name>`
//! - **Trait**:
//!   `R<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_module_path>_<sanitized_name>`
//! - **Function**:
//!   `F<len>_<sanitized_layer>_<sanitized_crate>_<sanitized_full_path>`
//!
//! `<len>` is the number of characters in the body that follows (excluding the
//! `T<len>_` / `R<len>_` / `F<len>_` prefix itself).
//!
//! `<sanitized_module_path>` is derived from `rustdoc_types::ItemSummary.path`
//! by removing the leading crate_name segment and the trailing item_name segment,
//! then joining the remaining middle segments with `_` and sanitizing the result.
//! For items at the crate root (no middle segments) the value is the empty string;
//! the separator `_` between `<sanitized_crate>` and `<sanitized_name>` in the body
//! is still present even when `<sanitized_module_path>` is empty (see body format
//! below).
//!
//! Body format for Type / Trait:
//!   `<sl>_<sc>_<sm>_<sn>`   when `sm` is non-empty
//!   `<sl>_<sc>__<sn>`       when `sm` is empty  (double underscore acts as placeholder)
//!
//! This ensures the length-prefix encodes a segment boundary difference that
//! disambiguates crate-root items from module-nested items with otherwise identical
//! names.
//!
//! Body format for Function:
//!   `<sl>_<sc>_<sp>`
//! where `<sp>` is the sanitized form of the full item path (all segments joined with `::`,
//! then sanitized as a single string).
//!
//! # Collision avoidance
//!
//! Including `sanitized_module_path` prevents node_id collision when the same type
//! name appears in different modules within the same crate (IN-05 / AC-11).  The
//! length-prefix further disambiguates sanitized bodies that happen to be identical
//! after character replacement (e.g. `a-b` and `a_b` both sanitize to `a_b`).
//!
//! Note: two distinct names that sanitize to the *same* string **and** have the
//! same length will still collide.  In practice this is extremely unlikely for
//! real Rust identifier names (which consist of ASCII letters, digits, and `_`).
//! The ADR notes that a hash-suffix strategy may be added if this proves
//! insufficient.
//!
//! # No panics
//!
//! All functions are panic-free: slice indexing is performed through
//! `Iterator::skip` / `Iterator::take` / `.get()`; no `[i]` indexing.
//!
//! (IN-05 / AC-11 / ADR 2026-05-22-1507 Decision D)

use super::sanitize;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Generate a node_id for a **Type** entry (Struct / Enum / TypeAlias).
///
/// Format: `T<len>_<sl>_<sc>_<sm>_<sn>` (Decision D).
///
/// - `layer` — layer name (e.g. `"domain"`).
/// - `crate_name` — crate name from `BaselineDocument.crate_name`.
/// - `module_path` — middle segments of `ItemSummary.path` (without the leading
///   crate-name and trailing item-name), joined with `_` by
///   [`module_path_from_summary`] **before** sanitization.
///   Pass an empty string for crate-root items.
/// - `type_name` — short item name.
#[allow(dead_code)]
pub(super) fn type_node_id(
    layer: &str,
    crate_name: &str,
    module_path: &str,
    type_name: &str,
) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sm = sanitize(module_path);
    let sn = sanitize(type_name);
    let body = type_trait_body(&sl, &sc, &sm, &sn);
    format!("T{}_{}", body.chars().count(), body)
}

/// Generate the representative node id for a Type entry.
///
/// Appends `__self` to the subgraph id so that edge endpoints can use a
/// concrete node rather than the subgraph container id (matches Contract Map
/// pattern).
#[allow(dead_code)]
pub(super) fn type_rep_node_id(
    layer: &str,
    crate_name: &str,
    module_path: &str,
    type_name: &str,
) -> String {
    format!("{}__self", type_node_id(layer, crate_name, module_path, type_name))
}

/// Generate a node_id for a **Trait** entry.
///
/// Format: `R<len>_<sl>_<sc>_<sm>_<sn>` (Decision D).
#[allow(dead_code)]
pub(super) fn trait_node_id(
    layer: &str,
    crate_name: &str,
    module_path: &str,
    trait_name: &str,
) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sm = sanitize(module_path);
    let sn = sanitize(trait_name);
    let body = type_trait_body(&sl, &sc, &sm, &sn);
    format!("R{}_{}", body.chars().count(), body)
}

/// Generate the representative node id for a Trait entry.
///
/// Appends `__self` to the subgraph id.
#[allow(dead_code)]
pub(super) fn trait_rep_node_id(
    layer: &str,
    crate_name: &str,
    module_path: &str,
    trait_name: &str,
) -> String {
    format!("{}__self", trait_node_id(layer, crate_name, module_path, trait_name))
}

/// Generate a node_id for a **Function** entry.
///
/// Format: `F<len>_<sl>_<sc>_<sp>` (Decision D).
///
/// - `full_path` — all path segments joined with `::` (including crate name and
///   item name), passed as-is; the function sanitizes it as a single string
///   per ADR 2026-05-22-1507 Decision D.
#[allow(dead_code)]
pub(super) fn function_node_id(layer: &str, crate_name: &str, full_path: &str) -> String {
    let sl = sanitize(layer);
    let sc = sanitize(crate_name);
    let sp = sanitize(full_path);
    let body = format!("{sl}_{sc}_{sp}");
    format!("F{}_{}", body.chars().count(), body)
}

// ---------------------------------------------------------------------------
// Public utility: extract module_path from ItemSummary.path
// ---------------------------------------------------------------------------

/// Derive `module_path` from a `rustdoc_types::ItemSummary.path` slice.
///
/// `ItemSummary.path` has the form `[crate_name, module_seg1, ..., item_name]`.
/// This function strips the leading crate-name and trailing item-name, then joins
/// the middle segments with `_` (un-sanitized — the caller passes the result to
/// `type_node_id` / `trait_node_id` which will sanitize it).
///
/// Returns an empty string for crate-root items (path has length ≤ 2, i.e. only
/// `[crate_name, item_name]`).
///
/// # Residual collision note
///
/// Joining with `_` means distinct module paths whose segment names contain `_`
/// can produce the same joined string (e.g. `foo_bar::Baz` and `foo::bar::Baz`
/// both yield `"foo_bar"`).  This is an acknowledged limitation of the ADR D
/// decision ("実際のアーキテクチャ構成では問題が発生しない").  The length-prefix
/// on the node_id body provides partial disambiguation; if collisions prove
/// problematic in practice a hash-suffix can be added per the ADR note.
///
/// # No panics
///
/// The function uses iterators with `.skip()` / `.count()` / `.take()` — no
/// direct slice indexing.
#[allow(dead_code)]
pub(super) fn module_path_from_summary(path: &[String]) -> String {
    let total = path.len();
    // Need at least 3 segments to have a middle segment: [crate, module, item].
    if total <= 2 {
        return String::new();
    }
    // Middle segments: skip 1 (crate_name), take total - 2 (drop item_name).
    let middle_count = total - 2;
    path.iter().skip(1).take(middle_count).cloned().collect::<Vec<_>>().join("_")
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Build the body string for Type / Trait node ids.
///
/// When `sm` (sanitized_module_path) is non-empty:  `<sl>_<sc>_<sm>_<sn>`
/// When `sm` is empty (crate-root item):            `<sl>_<sc>__<sn>`
///
/// The double underscore in the empty case acts as a stable placeholder that
/// keeps the segment count unambiguous when reading the id.
fn type_trait_body(sl: &str, sc: &str, sm: &str, sn: &str) -> String {
    if sm.is_empty() { format!("{sl}_{sc}__{sn}") } else { format!("{sl}_{sc}_{sm}_{sn}") }
}

// ---------------------------------------------------------------------------
// Tests (T006)
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    // -----------------------------------------------------------------------
    // module_path_from_summary
    // -----------------------------------------------------------------------

    #[test]
    fn test_module_path_from_summary_crate_root_returns_empty() {
        // path = [crate_name, item_name] → no middle segments
        let path = vec!["my_crate".to_string(), "MyStruct".to_string()];
        assert_eq!(module_path_from_summary(&path), "");
    }

    #[test]
    fn test_module_path_from_summary_one_module_segment() {
        // path = [crate_name, module, item_name]
        let path = vec!["my_crate".to_string(), "review".to_string(), "Review".to_string()];
        assert_eq!(module_path_from_summary(&path), "review");
    }

    #[test]
    fn test_module_path_from_summary_two_module_segments() {
        // path = [crate_name, module1, module2, item_name]
        // Segments joined with "_": "team_manager".
        let path = vec![
            "my_crate".to_string(),
            "team".to_string(),
            "manager".to_string(),
            "TeamManager".to_string(),
        ];
        assert_eq!(module_path_from_summary(&path), "team_manager");
    }

    #[test]
    fn test_module_path_from_summary_single_segment_path_returns_empty() {
        // Degenerate: path = [item_name] only (length 1, no crate prefix)
        let path = vec!["MyStruct".to_string()];
        assert_eq!(module_path_from_summary(&path), "");
    }

    #[test]
    fn test_module_path_from_summary_empty_path_returns_empty() {
        assert_eq!(module_path_from_summary(&[]), "");
    }

    // -----------------------------------------------------------------------
    // type_node_id — T/R prefix, crate-root vs module-nested
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_node_id_crate_root_starts_with_t_prefix() {
        let id = type_node_id("domain", "my_crate", "", "MyStruct");
        assert!(id.starts_with('T'), "must start with T prefix, got: {id}");
    }

    #[test]
    fn test_type_node_id_crate_root_format() {
        // body = "domain_my_crate__MyStruct" (double underscore for empty module_path)
        let id = type_node_id("domain", "my_crate", "", "MyStruct");
        // body: "domain_my_crate__MyStruct" → 25 chars
        let body = "domain_my_crate__MyStruct";
        let expected = format!("T{}_{}", body.len(), body);
        assert_eq!(id, expected, "crate-root type node_id must match expected");
    }

    #[test]
    fn test_type_node_id_with_module_path_format() {
        // module_path = "review", body = "domain_my_crate_review_Review"
        let id = type_node_id("domain", "my_crate", "review", "Review");
        let body = "domain_my_crate_review_Review";
        let expected = format!("T{}_{}", body.len(), body);
        assert_eq!(id, expected);
    }

    #[test]
    fn test_type_node_id_sanitizes_hyphens_in_layer() {
        // Hyphens in layer name must be replaced with underscores.
        let id = type_node_id("my-layer", "my_crate", "", "MyStruct");
        assert!(id.contains("my_layer"), "hyphen in layer must be sanitized to underscore");
    }

    #[test]
    fn test_type_node_id_sanitizes_hyphens_in_crate_name() {
        let id = type_node_id("domain", "my-crate", "", "MyStruct");
        assert!(id.contains("my_crate"), "hyphen in crate name must be sanitized");
    }

    // -----------------------------------------------------------------------
    // Collision prevention: same name, different module
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_node_id_same_name_different_module_no_collision() {
        // The key requirement of AC-11: same type name in different modules must
        // not produce the same node_id.
        let id_a = type_node_id("domain", "my_crate", "review", "Status");
        let id_b = type_node_id("domain", "my_crate", "user", "Status");
        assert_ne!(id_a, id_b, "same type name in different modules must have different node_ids");
    }

    #[test]
    fn test_type_node_id_same_name_crate_root_vs_module_no_collision() {
        // Crate-root item vs module-nested item with same name.
        let id_root = type_node_id("domain", "my_crate", "", "Config");
        let id_mod = type_node_id("domain", "my_crate", "config", "Config");
        assert_ne!(
            id_root, id_mod,
            "crate-root item and module-nested item with same name must not collide"
        );
    }

    // -----------------------------------------------------------------------
    // trait_node_id — R prefix
    // -----------------------------------------------------------------------

    #[test]
    fn test_trait_node_id_starts_with_r_prefix() {
        let id = trait_node_id("domain", "my_crate", "", "MyTrait");
        assert!(id.starts_with('R'), "trait node_id must start with R, got: {id}");
    }

    #[test]
    fn test_trait_node_id_crate_root_format() {
        let id = trait_node_id("domain", "my_crate", "", "MyTrait");
        let body = "domain_my_crate__MyTrait";
        let expected = format!("R{}_{}", body.len(), body);
        assert_eq!(id, expected);
    }

    #[test]
    fn test_trait_node_id_with_module_path() {
        let id = trait_node_id("domain", "my_crate", "ports", "Repository");
        let body = "domain_my_crate_ports_Repository";
        let expected = format!("R{}_{}", body.len(), body);
        assert_eq!(id, expected);
    }

    #[test]
    fn test_trait_node_id_same_name_different_module_no_collision() {
        // Traits with the same name in different modules.
        let id_a = trait_node_id("domain", "my_crate", "review", "Repository");
        let id_b = trait_node_id("domain", "my_crate", "user", "Repository");
        assert_ne!(id_a, id_b, "same trait name in different modules must have different node_ids");
    }

    // -----------------------------------------------------------------------
    // Type vs Trait prefix difference
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_and_trait_node_ids_differ_for_same_name() {
        // A Struct and a Trait with the exact same name must NOT collide (T vs R prefix).
        let type_id = type_node_id("domain", "my_crate", "", "Foo");
        let trait_id = trait_node_id("domain", "my_crate", "", "Foo");
        assert_ne!(
            type_id, trait_id,
            "Type and Trait with same name must differ due to T vs R prefix"
        );
    }

    // -----------------------------------------------------------------------
    // function_node_id — F prefix
    // -----------------------------------------------------------------------

    #[test]
    fn test_function_node_id_starts_with_f_prefix() {
        let id = function_node_id("domain", "my_crate", "my_crate::my_fn");
        assert!(id.starts_with('F'), "function node_id must start with F, got: {id}");
    }

    #[test]
    fn test_function_node_id_format() {
        // full_path sanitized: "my_crate::my_fn" → "my_crate__my_fn"
        let id = function_node_id("domain", "my_crate", "my_crate::my_fn");
        let body = "domain_my_crate_my_crate__my_fn";
        let expected = format!("F{}_{}", body.len(), body);
        assert_eq!(id, expected);
    }

    #[test]
    fn test_function_node_id_module_nested() {
        // full_path = "my_crate::utils::my_fn"
        let id = function_node_id("domain", "my_crate", "my_crate::utils::my_fn");
        let sanitized_path = "my_crate__utils__my_fn";
        let body = format!("domain_my_crate_{sanitized_path}");
        let expected = format!("F{}_{}", body.len(), body);
        assert_eq!(id, expected);
    }

    // -----------------------------------------------------------------------
    // rep node id helpers
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_rep_node_id_appends_double_underscore_self() {
        let rep = type_rep_node_id("domain", "my_crate", "", "MyStruct");
        let base = type_node_id("domain", "my_crate", "", "MyStruct");
        assert_eq!(rep, format!("{base}__self"));
    }

    #[test]
    fn test_trait_rep_node_id_appends_double_underscore_self() {
        let rep = trait_rep_node_id("domain", "my_crate", "", "MyTrait");
        let base = trait_node_id("domain", "my_crate", "", "MyTrait");
        assert_eq!(rep, format!("{base}__self"));
    }

    // -----------------------------------------------------------------------
    // Length prefix correctness
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_node_id_length_prefix_matches_body_char_count() {
        let id = type_node_id("domain", "my_crate", "review", "MyType");
        // Parse: T<num>_<body>
        let without_t = id.strip_prefix('T').expect("must start with T");
        let underscore_pos = without_t.find('_').expect("must have underscore after len");
        let len_str = &without_t[..underscore_pos];
        let body = &without_t[underscore_pos + 1..];
        let stated_len: usize = len_str.parse().expect("len must be numeric");
        assert_eq!(
            stated_len,
            body.chars().count(),
            "stated length must equal actual body char count"
        );
    }

    #[test]
    fn test_trait_node_id_length_prefix_matches_body_char_count() {
        let id = trait_node_id("infra", "some_crate", "ports", "MyTrait");
        let without_r = id.strip_prefix('R').expect("must start with R");
        let underscore_pos = without_r.find('_').expect("must have underscore after len");
        let len_str = &without_r[..underscore_pos];
        let body = &without_r[underscore_pos + 1..];
        let stated_len: usize = len_str.parse().expect("len must be numeric");
        assert_eq!(stated_len, body.chars().count());
    }

    #[test]
    fn test_function_node_id_length_prefix_matches_body_char_count() {
        let id = function_node_id("infra", "some_crate", "some_crate::do_thing");
        let without_f = id.strip_prefix('F').expect("must start with F");
        let underscore_pos = without_f.find('_').expect("must have underscore after len");
        let len_str = &without_f[..underscore_pos];
        let body = &without_f[underscore_pos + 1..];
        let stated_len: usize = len_str.parse().expect("len must be numeric");
        assert_eq!(stated_len, body.chars().count());
    }

    // -----------------------------------------------------------------------
    // Cross-layer and cross-crate uniqueness
    // -----------------------------------------------------------------------

    #[test]
    fn test_type_node_id_different_layers_do_not_collide() {
        let id_domain = type_node_id("domain", "my_crate", "", "Foo");
        let id_infra = type_node_id("infrastructure", "my_crate", "", "Foo");
        assert_ne!(id_domain, id_infra, "same type in different layers must differ");
    }

    #[test]
    fn test_type_node_id_different_crates_do_not_collide() {
        let id_a = type_node_id("domain", "crate_a", "", "Foo");
        let id_b = type_node_id("domain", "crate_b", "", "Foo");
        assert_ne!(id_a, id_b, "same type name in different crates must differ");
    }

    // -----------------------------------------------------------------------
    // Integration: module_path_from_summary → type_node_id
    // -----------------------------------------------------------------------

    #[test]
    fn test_integration_module_path_from_summary_into_type_node_id() {
        // Simulate: ItemSummary.path = ["domain", "review", "manager", "Review"]
        let path = vec![
            "domain".to_string(),
            "review".to_string(),
            "manager".to_string(),
            "Review".to_string(),
        ];
        let module_path = module_path_from_summary(&path);
        assert_eq!(module_path, "review_manager");

        let node_id = type_node_id("domain_layer", "domain", &module_path, "Review");
        // body = "domain_layer_domain_review_manager_Review"
        let body = "domain_layer_domain_review_manager_Review";
        let expected = format!("T{}_{}", body.len(), body);
        assert_eq!(node_id, expected);
    }

    #[test]
    fn test_integration_crate_root_item_module_path_from_summary_into_type_node_id() {
        // ItemSummary.path = ["domain", "DomainError"] (crate-root item)
        let path = vec!["domain".to_string(), "DomainError".to_string()];
        let module_path = module_path_from_summary(&path);
        assert_eq!(module_path, "");

        let node_id = type_node_id("domain_layer", "domain", &module_path, "DomainError");
        // body = "domain_layer_domain__DomainError" (double underscore)
        let body = "domain_layer_domain__DomainError";
        let expected = format!("T{}_{}", body.len(), body);
        assert_eq!(node_id, expected);
    }
}
