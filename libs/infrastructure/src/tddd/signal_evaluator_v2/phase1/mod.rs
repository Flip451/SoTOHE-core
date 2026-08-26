//! Phase 1 — S / D construction from A (catalogue TypeGraph) and B (baseline rustdoc).
//!
//! `phase1_build_s_and_d` is the main entry point; it walks A's item actions and
//! drives Phase 1.5 (closed-world resolution) and Phase 1.6 (dangling Id check).
//!
//! ## Sub-modules
//!
//! - `state`       — `Phase1State` accumulator (S / D index, name maps, Id allocator)
//! - `child_items` — child-item collection, remapping, insert/copy/remove helpers
//! - `builder`     — `phase1_build_s_and_d` main entry-point

mod builder;
mod child_items;
mod state;

pub(super) mod rustdoc_authority {
    use std::collections::{HashMap, HashSet};

    use domain::tddd::Phase1Error;
    use domain::tddd::catalogue_v2::CrateName;
    use rustdoc_types::{Crate, Id, ItemSummary};

    use crate::tddd::canonical_type_identity::canonicalize_rustdoc_root_path;

    pub(super) fn canonicalize_rustdoc_paths(
        krate: &Crate,
        package_name: Option<&CrateName>,
        rustdoc_root_name: Option<&CrateName>,
    ) -> Crate {
        let Some(package_name) = package_name else {
            return krate.clone();
        };
        let mut canonical = krate.clone();
        canonical.paths = krate
            .paths
            .iter()
            .map(|(&id, summary)| {
                let mut canonical_summary = summary.clone();
                if summary.crate_id == 0 {
                    canonical_summary.path = canonicalize_rustdoc_root_path(
                        &summary.path,
                        package_name,
                        rustdoc_root_name,
                    );
                }
                (id, canonical_summary)
            })
            .collect();
        canonical
    }

    /// Rewrites the local `Crate::paths` roots of an owned crate in place.
    ///
    /// Only the `paths` summaries change; `index` and every other field are
    /// left untouched, so no second copy of an externally sized rustdoc
    /// artifact is ever held.
    pub(in crate::tddd::signal_evaluator_v2) fn canonicalize_rustdoc_paths_in_place(
        krate: &mut Crate,
        package_name: Option<&CrateName>,
        rustdoc_root_name: Option<&CrateName>,
    ) {
        let Some(package_name) = package_name else {
            return;
        };
        for summary in krate.paths.values_mut() {
            if summary.crate_id == 0 {
                summary.path =
                    canonicalize_rustdoc_root_path(&summary.path, package_name, rustdoc_root_name);
            }
        }
    }

    pub(super) fn merge_definition_path_maps(
        baseline: &HashMap<Id, ItemSummary>,
        catalogue: &HashMap<Id, ItemSummary>,
    ) -> Result<HashMap<Id, ItemSummary>, Phase1Error> {
        let mut merged = baseline.clone();
        // Baseline and catalogue paths use independent Id spaces. Reserve both
        // spaces before allocating a replacement so an unvisited catalogue Id
        // cannot be overwritten by a remapped collision.
        let mut used_ids = baseline.keys().chain(catalogue.keys()).copied().collect::<HashSet<_>>();
        let mut next_id = used_ids.iter().map(|id| id.0).max().unwrap_or(0).saturating_add(1);
        let mut catalogue_entries = catalogue.iter().collect::<Vec<_>>();
        catalogue_entries.sort_unstable_by_key(|(id, _)| id.0);
        for (&id, summary) in catalogue_entries {
            let target_id = if merged.contains_key(&id) {
                next_unused_definition_path_id(&mut next_id, &mut used_ids).ok_or_else(|| {
                    Phase1Error::rustdoc_root_resolution(
                        "Phase 1 definition-path authority exhausted its item-id space",
                    )
                })?
            } else {
                id
            };
            merged.insert(target_id, summary.clone());
        }
        Ok(merged)
    }

    fn next_unused_definition_path_id(next_id: &mut u32, used_ids: &mut HashSet<Id>) -> Option<Id> {
        loop {
            let candidate = Id(*next_id);
            if used_ids.insert(candidate) {
                *next_id = (*next_id).checked_add(1).map_or(u32::MAX, |next| next);
                return Some(candidate);
            }
            *next_id = (*next_id).checked_add(1)?;
        }
    }
}

#[cfg(test)]
mod definition_path_authority_tests;

#[cfg(test)]
pub(crate) use builder::phase1_build_s_and_d;
pub(crate) use builder::phase1_build_s_and_d_with_rustdoc_root;
