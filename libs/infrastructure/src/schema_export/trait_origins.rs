//! Trait origin resolution for the schema export infrastructure adapter.
//!
//! Maps each trait's stable fully-qualified definition path (from `rustdoc_types::Crate::paths`)
//! to the name of its defining crate. Used by `build_schema_export` to populate
//! `SchemaExport::trait_origins`, which `code_profile_builder` consults when resolving
//! per-impl trait provenance.

use rustdoc_types::ItemEnum;

/// Builds a map of trait def_path → defining crate name from rustdoc metadata.
///
/// For each impl block that implements a named trait, looks up the trait's `Id`
/// in `krate.paths` to find its `crate_id`, then resolves the crate name from
/// `krate.external_crates`. `crate_id == 0` means the trait is defined in the
/// current crate (`crate_name`).
///
/// The map key is the trait's **stable fully-qualified definition path** from
/// `krate.paths` (e.g., `"std::fmt::Display"`), not the short name.  Keying
/// by def_path avoids aliasing when two distinct traits share the same short
/// name (e.g., a local `Display` and `std::fmt::Display`): both get their own
/// entry, so `code_profile_builder` can resolve each impl independently via
/// `ImplInfo::trait_def_path`.
///
/// When a trait id is absent from `krate.paths` the use-site `Path.path`
/// string is used as a fallback key so the entry is still reachable.
pub(super) fn build_trait_origins(
    crate_name: &str,
    krate: &rustdoc_types::Crate,
) -> std::collections::HashMap<String, String> {
    // Keyed by def_path (stable); BTreeMap for deterministic iteration.
    let mut by_def_path: std::collections::BTreeMap<String, String> =
        std::collections::BTreeMap::new();

    for item in krate.index.values() {
        let ItemEnum::Impl(i) = &item.inner else { continue };
        if i.is_synthetic || i.blanket_impl.is_some() || i.is_negative {
            continue;
        }
        let trait_path = match &i.trait_ {
            Some(p) => p,
            None => continue,
        };
        // Use the definition path from krate.paths (stable, canonical) as the
        // key. Fall back to the use-site path only when the trait id is not
        // present in the paths table.
        let def_path = krate
            .paths
            .get(&trait_path.id)
            .map(|s| s.path.join("::"))
            .unwrap_or_else(|| trait_path.path.clone());
        if by_def_path.contains_key(&def_path) {
            // Already recorded (same trait, different use-site spelling).
            continue;
        }
        let origin = resolve_trait_origin(crate_name, &trait_path.id, krate);
        by_def_path.insert(def_path, origin);
    }

    // Return def_path → origin directly (no collapse to short_name).
    by_def_path.into_iter().collect()
}

/// Resolves the origin crate name for a trait `Id`.
///
/// Returns `crate_name` when the trait is defined in the current crate
/// (`crate_id == 0`), the external crate name from `external_crates` otherwise,
/// or `""` when the `Id` is not found in `paths`.
pub(super) fn resolve_trait_origin(
    crate_name: &str,
    trait_id: &rustdoc_types::Id,
    krate: &rustdoc_types::Crate,
) -> String {
    let summary = match krate.paths.get(trait_id) {
        Some(s) => s,
        None => return String::new(),
    };
    if summary.crate_id == 0 {
        return crate_name.to_string();
    }
    krate.external_crates.get(&summary.crate_id).map(|ec| ec.name.clone()).unwrap_or_default()
}
