//! Phase 2 — S / D / C 3-way evaluation.
//!
//! Builds identity sets for S, D, and C, then evaluates each item against
//! the signal table (ADR 3 D3) to produce `ThreeWaySignal`s.

use std::collections::{BTreeMap, HashSet};

use domain::tddd::catalogue_v2::ItemAction;
use domain::tddd::{Phase1Error, SignalRegion, ThreeWayEvaluationReport, ThreeWaySignal};
use rustdoc_types::{Crate, Id};

use crate::tddd::canonical_type_identity::DefinitionPathAuthority;

use super::impl_identity::try_build_impl_identity_map_with_authority;
use super::phase1::rustdoc_authority::canonicalize_rustdoc_paths_in_place;
use super::structural_eq::items_structurally_equal_with_authority;
use super::{
    RustdocTargetResolution, TypeTraitIdentityKey, TypeTraitIdentityMap,
    build_function_identity_map, build_type_trait_identity_map,
};
use domain::tddd::ExtendedCrate;

/// Runs Phase 2: evaluates S / D / C and produces a `ThreeWayEvaluationReport`.
pub(super) fn phase2_evaluate(
    s: &ExtendedCrate,
    d: &Crate,
    mut c: Crate,
    rustdoc_root: Option<&RustdocTargetResolution>,
) -> Result<ThreeWayEvaluationReport, Phase1Error> {
    let s_krate = s.krate();

    // D2: the bin-root alias is applied once, through the same canonicalization
    // the baseline crossed in Phase 1. C's type/trait identities and definition
    // paths must speak the catalogue package root before they are compared
    // against S and D; otherwise a bin target whose rustdoc root differs from
    // its package name splits every matching item into `SMinusC` plus
    // `CMinusSUnionD`. The crate is owned here and only its `paths` summaries
    // are rewritten, so an externally sized rustdoc artifact is never cloned.
    canonicalize_rustdoc_paths_in_place(
        &mut c,
        rustdoc_root.map(|resolution| resolution.package_name()),
        rustdoc_root.map(|resolution| resolution.rustdoc_root_name()),
    );
    let c = &c;

    // Derive the crate name from C's root item so that rustdoc local-trait paths
    // (`my_crate::MyTrait`) can be normalized to match codec paths (`crate::MyTrait`).
    let crate_name = c.index.get(&c.root).and_then(|item| item.name.as_deref()).unwrap_or("");
    let definition_paths =
        DefinitionPathAuthority::from_path_maps(&c.paths, &[&s_krate.paths, &d.paths]);

    // Build identity sets. Type/trait maps use complete paths; the signal label
    // is reduced to a short name only when that short name is unambiguous across
    // S, D, and C. This keeps existing reports readable without allowing the
    // display label to decide identity.
    let s_types = build_type_trait_identity_map(s_krate)?;
    let s_fns = build_function_identity_map(s_krate, rustdoc_root);

    // S inherits B-seeded impl blocks whose trait paths are in rustdoc format
    // (`my_crate::MyTrait`). Pass `crate_name` so those are normalized to short
    // names that match C-side keys.
    //
    // The former `priority_ids` (s_a_side_ids) band-aid has been removed (T015 /
    // ADR `2026-05-20-0048` D4): action-driven insertion in Phase 1 (builder.rs)
    // now places each TraitImplDeclV2 into S according to its own declared action,
    // so B-side impls cannot shadow A-side impls for the same identity key.
    let s_impls =
        try_build_impl_identity_map_with_authority(s_krate, crate_name, &definition_paths)?;
    let d_types = build_type_trait_identity_map(d)?;
    let d_fns = build_function_identity_map(d, rustdoc_root);
    let d_impls = try_build_impl_identity_map_with_authority(d, crate_name, &definition_paths)?;
    let c_types = build_type_trait_identity_map(c)?;
    let c_fns = build_function_identity_map(c, rustdoc_root);
    let c_impls = try_build_impl_identity_map_with_authority(c, crate_name, &definition_paths)?;

    // Build a secondary lookup for C impls keyed by their generic-args-stripped form.
    //
    // This handles the mismatch between S-side catalogue entries that declare a trait
    // without generic args (e.g. `From` via `TraitImplDeclV2`) and C-side rustdoc impls
    // that include concrete generic args (e.g. `From<Error>` from thiserror `#[from]`).
    // When an S key like `"Foo: From"` cannot be found in `c_impls` by exact lookup,
    // we fall back to `c_impls_stripped` to find `"Foo: From"` → the Id of `"Foo: From<Error>"`.
    //
    // When multiple C impls strip to the same key (e.g. `"Foo: From<A>"` and
    // `"Foo: From<B>"` both strip to `"Foo: From"`), only the first one (by BTreeMap
    // key sort order) is kept.  The `CMinusSUnionD` logic uses `c_matched_via_stripped`
    // (built during the S-vs-C loop) to track exactly which C key was consumed, so the
    // remaining ones correctly surface as `CMinusSUnionD`.
    let c_impls_stripped: std::collections::BTreeMap<String, rustdoc_types::Id> = c_impls
        .iter()
        .filter_map(|(key, &id)| strip_impl_key_trait_generic_args(key).map(|sk| (sk, id)))
        .collect();

    // Inverse map: C impl Id → C impl key string.  Used to recover the exact C key
    // from an Id returned by `c_impls_stripped` during the S-vs-C loop.
    let c_impl_id_to_key: std::collections::HashMap<rustdoc_types::Id, &str> =
        c_impls.iter().map(|(k, &id)| (id, k.as_str())).collect();

    // Set of C impl key strings that were matched (directly or via stripped fallback)
    // during the S-vs-C impl loop.  Only keys in this set are suppressed from
    // `CMinusSUnionD` — the single-match guarantee prevents a lone `"Foo: From"` S
    // entry from silently covering all `"Foo: From<*>"` C impls.
    let mut c_matched_via_stripped: HashSet<&str> = HashSet::new();

    let mut signals: Vec<ThreeWaySignal> = Vec::new();

    // --- Evaluate S types/traits vs C ---
    for (name, s_id) in &s_types {
        let action = s.action_for(s_id).unwrap_or(ItemAction::Reference);
        let s_item = s_krate.index.get(s_id);

        if let Some(c_id) = c_types.get(name) {
            // Item in S ∩ C.
            let c_item = c.index.get(c_id);
            let structurally_equal = match (s_item, c_item) {
                (Some(si), Some(ci)) => items_structurally_equal_with_authority(
                    si,
                    ci,
                    &s_krate.index,
                    &c.index,
                    &s_krate.paths,
                    &c.paths,
                    crate_name,
                    &definition_paths,
                ),
                _ => false,
            };
            let region = s_intersect_c_region(action, structurally_equal);
            signals.push(ThreeWaySignal::catalogue_item(
                domain::FreeText::new(type_signal_name(name, &[&s_types, &d_types, &c_types])),
                name.namespace,
                region,
            ));
        } else {
            // Item in S \ C.
            let region = s_minus_c_region(action);
            signals.push(ThreeWaySignal::catalogue_item(
                domain::FreeText::new(type_signal_name(name, &[&s_types, &d_types, &c_types])),
                name.namespace,
                region,
            ));
        }
    }

    // --- Evaluate S functions vs C ---
    for (fn_path, s_id) in &s_fns {
        let action = s.action_for(s_id).unwrap_or(ItemAction::Reference);
        let s_item = s_krate.index.get(s_id);

        if let Some(c_id) = c_fns.get(fn_path.as_str()) {
            let c_item = c.index.get(c_id);
            let structurally_equal = match (s_item, c_item) {
                (Some(si), Some(ci)) => items_structurally_equal_with_authority(
                    si,
                    ci,
                    &s_krate.index,
                    &c.index,
                    &s_krate.paths,
                    &c.paths,
                    crate_name,
                    &definition_paths,
                ),
                _ => false,
            };
            let region = s_intersect_c_region(action, structurally_equal);
            signals.push(ThreeWaySignal::label(domain::FreeText::new(fn_path.clone()), region));
        } else {
            let region = s_minus_c_region(action);
            signals.push(ThreeWaySignal::label(domain::FreeText::new(fn_path.clone()), region));
        }
    }

    // --- Evaluate S impls vs C ---
    for (key, s_id) in &s_impls {
        let action = s.action_for(s_id).unwrap_or(ItemAction::Reference);
        let s_item = s_krate.index.get(s_id);

        // Exact lookup first; fall back to stripped-key lookup for S entries that
        // declare a trait without generic args (e.g. `From`) when C has the impl
        // with concrete generic args (e.g. `From<Error>` from thiserror `#[from]`).
        //
        // When the stripped-key fallback is used (`via_stripped = true`), skip
        // structural comparison: the S item has no generic args in its trait path
        // (e.g. `From`) while the C item has concrete args (e.g. `From<Error>`).
        // `items_structurally_equal` compares the full trait path string, so it
        // would always return `false` for this case even when the impls are
        // semantically compatible. Treating the match as structurally equal is
        // correct: the catalogue's identity-only `TraitImplDeclV2` intentionally
        // omits generic args, and the concrete args come from `#[from]`/`#[derive]`
        // — both sides describe the same implementation.
        let c_id_opt = c_impls.get(key.as_str()).copied();
        let (c_id_opt, via_stripped) = if let Some(id) = c_id_opt {
            (Some(id), false)
        } else {
            (c_impls_stripped.get(key.as_str()).copied(), true)
        };

        if let Some(c_id) = c_id_opt {
            let c_item = c.index.get(&c_id);
            let structurally_equal = if via_stripped {
                // Stripped-key match: generic args differ by design; treat as equal.
                // Record the exact C key that was consumed so that other C impls
                // stripping to the same S key are NOT silently covered.
                if let Some(c_key) = c_impl_id_to_key.get(&c_id) {
                    c_matched_via_stripped.insert(c_key);
                }
                true
            } else {
                match (s_item, c_item) {
                    (Some(si), Some(ci)) => items_structurally_equal_with_authority(
                        si,
                        ci,
                        &s_krate.index,
                        &c.index,
                        &s_krate.paths,
                        &c.paths,
                        crate_name,
                        &definition_paths,
                    ),
                    _ => false,
                }
            };
            let region = s_intersect_c_region(action, structurally_equal);
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(impl_signal_name(
                    key,
                    &[&s_impls, &d_impls, &c_impls],
                    crate_name,
                )),
                region,
            ));
        } else {
            let region = s_minus_c_region(action);
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(impl_signal_name(
                    key,
                    &[&s_impls, &d_impls, &c_impls],
                    crate_name,
                )),
                region,
            ));
        }
    }

    // --- Evaluate D types/traits vs C ---
    for name in d_types.keys() {
        if c_types.contains_key(name) {
            // D ∩ C: delete in progress.
            signals.push(ThreeWaySignal::catalogue_item(
                domain::FreeText::new(type_signal_name(name, &[&s_types, &d_types, &c_types])),
                name.namespace,
                SignalRegion::DIntersectC,
            ));
        } else {
            // D \ C: delete achieved.
            signals.push(ThreeWaySignal::catalogue_item(
                domain::FreeText::new(type_signal_name(name, &[&s_types, &d_types, &c_types])),
                name.namespace,
                SignalRegion::DMinusC,
            ));
        }
    }

    // --- Evaluate D functions vs C ---
    for fn_path in d_fns.keys() {
        if c_fns.contains_key(fn_path.as_str()) {
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(fn_path.clone()),
                SignalRegion::DIntersectC,
            ));
        } else {
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(fn_path.clone()),
                SignalRegion::DMinusC,
            ));
        }
    }

    // --- Evaluate D impls vs C ---
    // D items originate from catalogue-codec Impl entries (args: None) when a type
    // was first Modify'd (replacing B's impl blocks with A's no-generic-args blocks)
    // and later Delete'd.  The D key therefore has no generic args (e.g. `"Foo: From"`)
    // while the C-side rustdoc impl may include concrete generic args (`"Foo: From<Error>"`).
    //
    // Unlike the S-side loop (where the S key is no-args and we strip the C key for
    // lookup), here we look up the D key DIRECTLY in `c_impls_stripped`: the D key
    // IS the stripped form, and `c_impls_stripped` is keyed by stripped C keys.
    //
    // When a stripped-key match is found, record the matched C key in
    // `c_matched_via_stripped` so the `CMinusSUnionD` loop only suppresses that
    // exact C impl (single-match guarantee, same as S-side).
    for key in d_impls.keys() {
        if c_impls.contains_key(key.as_str()) {
            // Exact match: D key found verbatim in C.
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(impl_signal_name(
                    key,
                    &[&s_impls, &d_impls, &c_impls],
                    crate_name,
                )),
                SignalRegion::DIntersectC,
            ));
        } else if let Some(&stripped_c_id) = c_impls_stripped.get(key.as_str()) {
            // Stripped match: C has a generic-args version of this D key.
            // Record the exact C key consumed so CMinusSUnionD can skip it.
            if let Some(c_key) = c_impl_id_to_key.get(&stripped_c_id) {
                c_matched_via_stripped.insert(c_key);
            }
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(impl_signal_name(
                    key,
                    &[&s_impls, &d_impls, &c_impls],
                    crate_name,
                )),
                SignalRegion::DIntersectC,
            ));
        } else {
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(impl_signal_name(
                    key,
                    &[&s_impls, &d_impls, &c_impls],
                    crate_name,
                )),
                SignalRegion::DMinusC,
            ));
        }
    }

    // --- Evaluate C \ (S ∪ D): undeclared implementations ---
    // For each C type/trait not in S or D.
    let s_union_d_types: HashSet<&TypeTraitIdentityKey> =
        s_types.keys().chain(d_types.keys()).collect();
    for name in c_types.keys() {
        if !s_union_d_types.contains(name) {
            signals.push(ThreeWaySignal::catalogue_item(
                domain::FreeText::new(type_signal_name(name, &[&s_types, &d_types, &c_types])),
                name.namespace,
                SignalRegion::CMinusSUnionD,
            ));
        }
    }

    // For each C function not in S or D.
    let s_union_d_fns: HashSet<&str> =
        s_fns.keys().chain(d_fns.keys()).map(String::as_str).collect();
    for fn_path in c_fns.keys() {
        if !s_union_d_fns.contains(fn_path.as_str()) {
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(fn_path.clone()),
                SignalRegion::CMinusSUnionD,
            ));
        }
    }

    // For each C impl not in S or D.
    let s_union_d_impls: HashSet<&str> =
        s_impls.keys().chain(d_impls.keys()).map(String::as_str).collect();
    for key in c_impls.keys() {
        if !s_union_d_impls.contains(key.as_str()) {
            // Generic-args fallback: a catalogue entry may declare a trait impl
            // without generic args (e.g. `From` in `TraitImplDeclV2`) while the
            // C-side rustdoc includes the concrete type parameter (e.g. `From<Error>`
            // generated by thiserror's `#[from]` attribute).
            //
            // Suppression condition: this exact C key was consumed by either the
            // S-vs-C or D-vs-C loop via the stripped-key fallback.  Only the
            // single C impl that was matched is suppressed — all other C impls
            // stripping to the same S/D key (e.g. a second `impl From<OtherError>`)
            // are NOT in `c_matched_via_stripped` and will correctly emit
            // `CMinusSUnionD`.
            //
            // The older pattern of checking `s_union_d_impls.contains(stripped)`
            // would suppress ALL C impls sharing the same stripped form, which
            // allowed newly-added `From<T>` impls to escape the gate undetected.
            if c_matched_via_stripped.contains(key.as_str()) {
                // C impl was already covered by an S/D-vs-C stripped-key match.
                continue;
            }
            signals.push(ThreeWaySignal::label(
                domain::FreeText::new(impl_signal_name(
                    key,
                    &[&s_impls, &d_impls, &c_impls],
                    crate_name,
                )),
                SignalRegion::CMinusSUnionD,
            ));
        }
    }

    Ok(ThreeWayEvaluationReport::new(signals))
}

/// Returns the report label for a type/trait identity.
///
/// A short label is retained for the normal, unambiguous case. If two distinct
/// fully-qualified identities share that short name, the full identity remains
/// in the report so the output cannot merge the two targets after the evaluator
/// has kept them separate internally.
fn type_signal_name(
    identity_key: &TypeTraitIdentityKey,
    identity_maps: &[&TypeTraitIdentityMap],
) -> String {
    let short_name = identity_key.short_name();
    let matching_identities: HashSet<&str> = identity_maps
        .iter()
        .flat_map(|map| map.keys().map(|candidate| candidate.path.as_str()))
        .filter(|candidate| candidate.rsplit("::").next().unwrap_or(candidate) == short_name)
        .collect();

    if matching_identities.len() > 1 { identity_key.path.clone() } else { short_name.to_owned() }
}

/// Keeps the established short display for an unambiguous impl while retaining
/// the full owner/trait identity when same-named impls coexist.
fn impl_signal_name(
    identity_key: &str,
    identity_maps: &[&BTreeMap<String, Id>],
    crate_name: &str,
) -> String {
    let Some((owner, trait_path)) = identity_key.split_once(": ") else {
        return identity_key.to_owned();
    };
    let owner_base = owner.split('<').next().unwrap_or(owner);
    let owner_short = owner_base.rsplit("::").next().unwrap_or(owner_base);
    let trait_short = trait_path
        .split('<')
        .next()
        .unwrap_or(trait_path)
        .rsplit("::")
        .next()
        .unwrap_or(trait_path);
    let trait_base = trait_path.split('<').next().unwrap_or(trait_path);
    let trait_suffix = trait_path.strip_prefix(trait_base).unwrap_or("");
    let trait_display = if !crate_name.is_empty()
        && (trait_base == crate_name || trait_base.starts_with(&format!("{crate_name}::")))
    {
        format!("{trait_short}{trait_suffix}")
    } else if trait_base.contains("::") {
        format!("{trait_base}{trait_suffix}")
    } else {
        format!("{trait_short}{trait_suffix}")
    };
    let short = format!("{owner_short}: {trait_display}");
    // Owner identity must remain qualified whenever two different fully
    // qualified owners share the same short name.  Comparing the combined
    // owner/trait pair is insufficient: `alpha::Input: alpha::Port` and
    // `beta::Input: beta::OtherPort` have distinct short pairs, but shortening
    // both owners would still make the resulting signal index cross-join them.
    let matching_owners: HashSet<&str> = identity_maps
        .iter()
        .flat_map(|map| map.keys().map(String::as_str))
        .filter_map(|candidate| {
            candidate.split_once(": ").map(|(candidate_owner, _)| candidate_owner)
        })
        .filter(|candidate_owner| impl_owner_short(candidate_owner) == owner_short)
        .collect();
    if matching_owners.len() > 1 {
        return identity_key.to_owned();
    }

    let matching: HashSet<&str> = identity_maps
        .iter()
        .flat_map(|map| map.keys().map(String::as_str))
        .filter(|candidate| {
            let Some((candidate_owner, candidate_trait)) = candidate.split_once(": ") else {
                return false;
            };
            let candidate_trait_base = candidate_trait.split('<').next().unwrap_or(candidate_trait);
            impl_owner_short(candidate_owner) == owner_short
                && candidate_trait_base.rsplit("::").next().unwrap_or(candidate_trait_base)
                    == trait_short
        })
        .collect();
    if matching.len() > 1 { identity_key.to_owned() } else { short }
}

fn impl_owner_short(owner: &str) -> &str {
    let owner_base = owner.split('<').next().unwrap_or(owner);
    owner_base.rsplit("::").next().unwrap_or(owner_base)
}

/// Strips generic args from the trait part of an impl identity key.
///
/// Identity key format: `"ForTypeName: TraitPath<GenericArgs>"`.
///
/// Returns `Some("ForTypeName: TraitPath")` when the trait part contains `<`,
/// or `None` when the key has no generic args in the trait part (no stripping needed).
///
/// Used in the `CMinusSUnionD` impl loop: a C-side key `"Foo: From<Error>"` may
/// be covered by an S-side entry `"Foo: From"` when the catalogue declares the
/// `From` trait impl without generic args (e.g. via `TraitImplDeclV2` which has
/// no `generic_args` field).  This avoids spurious Red signals for
/// thiserror `#[from]`-generated impls whose concrete type parameter cannot be
/// expressed in the current catalogue schema.
fn strip_impl_key_trait_generic_args(key: &str) -> Option<String> {
    // Split on ": " to get the `for_` part and the trait part.
    let sep = ": ";
    let sep_pos = key.find(sep)?;
    let for_part = &key[..sep_pos];
    let trait_part = &key[sep_pos + sep.len()..];
    // Strip from the first `<` in the trait part (if any).
    let angle_pos = trait_part.find('<')?;
    let stripped_trait = &trait_part[..angle_pos];
    Some(format!("{for_part}: {stripped_trait}"))
}

/// Determines the `SignalRegion` for an item in S ∩ C.
pub(super) fn s_intersect_c_region(action: ItemAction, structurally_equal: bool) -> SignalRegion {
    match (action, structurally_equal) {
        (ItemAction::Add, true) => SignalRegion::SIntersectC_Match_Add,
        (ItemAction::Modify, true) => SignalRegion::SIntersectC_Match_Modify,
        (ItemAction::Reference, true) => SignalRegion::SIntersectC_Match_Reference,
        (ItemAction::Reference, false) => SignalRegion::SIntersectC_Mismatch_Reference,
        (ItemAction::Add, false) => SignalRegion::SIntersectC_Mismatch_Add,
        (ItemAction::Modify, false) => SignalRegion::SIntersectC_Mismatch_Modify,
        // Delete should never appear in S (already moved to D in Phase 1).
        // Treat as Reference mismatch as a safe fallback.
        (ItemAction::Delete, true) => SignalRegion::SIntersectC_Match_Reference,
        (ItemAction::Delete, false) => SignalRegion::SIntersectC_Mismatch_Reference,
    }
}

/// Determines the `SignalRegion` for an item in S \ C.
pub(super) fn s_minus_c_region(action: ItemAction) -> SignalRegion {
    match action {
        ItemAction::Add => SignalRegion::SMinusC_Add,
        ItemAction::Modify => SignalRegion::SMinusC_Modify,
        ItemAction::Reference => SignalRegion::SMinusC_Reference,
        // Delete should never appear in S, treat as Reference.
        ItemAction::Delete => SignalRegion::SMinusC_Reference,
    }
}

#[cfg(test)]
mod tests {
    use super::impl_signal_name;
    use rustdoc_types::Id;
    use std::collections::BTreeMap;

    #[test]
    fn test_impl_signal_name_qualifies_shared_owner_with_distinct_traits() {
        let mut alpha = BTreeMap::new();
        alpha.insert("domain::alpha::Input: domain::alpha::Port".to_owned(), Id(1));
        let mut beta = BTreeMap::new();
        beta.insert("domain::beta::Input: domain::beta::OtherPort".to_owned(), Id(2));
        let maps = [&alpha, &beta];

        assert_eq!(
            impl_signal_name("domain::alpha::Input: domain::alpha::Port", &maps, "domain",),
            "domain::alpha::Input: domain::alpha::Port"
        );
        assert_eq!(
            impl_signal_name("domain::beta::Input: domain::beta::OtherPort", &maps, "domain",),
            "domain::beta::Input: domain::beta::OtherPort"
        );
    }

    #[test]
    fn test_impl_signal_name_strips_generic_owner_before_shortening() {
        let mut identities = BTreeMap::new();
        identities.insert("domain::Wrapper<domain::alpha::Input>: domain::Port".to_owned(), Id(1));
        let maps = [&identities];

        assert_eq!(
            impl_signal_name(
                "domain::Wrapper<domain::alpha::Input>: domain::Port",
                &maps,
                "domain",
            ),
            "Wrapper: Port"
        );
    }
}
