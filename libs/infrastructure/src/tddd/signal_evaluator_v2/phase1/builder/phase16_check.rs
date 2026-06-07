//! Phase 1.6 — Dangling Id check.
//!
//! After Phase 1.45 (A-side type-ref remapping) and Phase 1.5 (closed-world resolution),
//! verify that no local-crate Id referenced in S's items is missing from S.

use std::collections::HashSet;

use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::ItemAction;
use rustdoc_types::{Crate, Id};

use super::super::super::collect_refs::collect_referenced_ids;
use super::super::state::Phase1State;

/// Performs the Phase 1.6 dangling-Id check on all items currently in S.
///
/// After T037, both A- and B-sourced items are renumbered into the same fresh S Id space.
/// The item-source discriminator uses `s_actions` exclusively (no id-based heuristic).
///
/// ## Validation rules
///
/// **A-SOURCED items** (Add/Modify in s_actions):
///   - Rule 0: `Id(0)` — sentinel — skip.
///   - Rule 1: Id is in S — valid.
///   - Rule 2a: In `a_krate.paths` with `crate_id != 0` — A-side external — valid.
///   - Rule 2b: In `a_krate.paths` with `crate_id == 0` — A-side local not remapped — `DanglingId`.
///   - Rule 3: In `b.paths` with `crate_id != 0` — B-side external (possible for Modify) — valid.
///   - Rule 4: In `b.paths` with `crate_id == 0` — deleted local — `DanglingId`.
///   - Rule 5: Not found — stale — `DanglingId`.
///
/// **B-SOURCED items** (Reference in s_actions):
///   - Rule 0: `Id(0)` — sentinel — skip.
///   - Rule 1: Id is in S — valid.
///   - Rule 2: In `b.paths` with `crate_id != 0` — B-side external — valid.
///   - Rule 3: In `b.paths` with `crate_id == 0` — deleted local — `DanglingId`.
///   - Rule 4: Not found — stale — `DanglingId`.
///
/// # Errors
/// Returns `Phase1Error::DanglingId` when a referenced Id is not present in S and cannot
/// be classified as a valid external reference.
pub(crate) fn check_dangling_ids(
    state: &Phase1State,
    a_krate: &rustdoc_types::Crate,
    b: &Crate,
) -> Result<(), Phase1Error> {
    let s_ids: HashSet<Id> = state.s_index.keys().copied().collect();

    for item in state.s_index.values() {
        let item_name = item.name.clone().unwrap_or_else(|| format!("<id:{}>", item.id.0));

        // Determine whether this item is A-sourced.
        //
        // After T037, ALL items in S (top-level + children) have an entry in
        // `s_actions`.  The `s_actions` lookup is the sole authoritative discriminator.
        let item_is_a_sourced = matches!(
            state.s_actions.get(&item.id),
            Some(&ItemAction::Add) | Some(&ItemAction::Modify),
        );

        for referenced_id in collect_referenced_ids(item) {
            // Rule 0: Id(0) is the crate root module / `Self` sentinel — skip unconditionally.
            if referenced_id.0 == 0 {
                continue;
            }
            // Rule 1: id is in S — valid regardless of source.
            if s_ids.contains(&referenced_id) || state.s_paths.contains_key(&referenced_id) {
                continue;
            }

            if item_is_a_sourced {
                // A-sourced item: check A-side paths before B-side.
                if let Some(a_ps) = a_krate.paths.get(&referenced_id) {
                    if a_ps.crate_id != 0 {
                        // Rule 2a: A-side external-crate item — valid cross-crate ref.
                        continue;
                    }
                    // Rule 2b: A-side local id not remapped by Phase 1.45.
                    let a_name = a_ps.path.last().map(|s| s.as_str()).unwrap_or("<unknown>");
                    return Err(Phase1Error::DanglingId(format!(
                        "{item_name} -> Id({}) is an A-side local ref to '{}' which is not present in S \
                         (the type may have been deleted or was never declared in the catalogue)",
                        referenced_id.0, a_name
                    )));
                }
                // Fallthrough: not in A-side paths; check B-side.
                if let Some(ps) = b.paths.get(&referenced_id) {
                    if ps.crate_id != 0 {
                        // Rule 3: B-side external-crate item — valid.
                        continue;
                    }
                    // Rule 4: B-side local deleted.
                    return Err(Phase1Error::DanglingId(format!(
                        "{item_name} -> Id({}) not found in S (may have been deleted in Phase 1)",
                        referenced_id.0
                    )));
                }
                // Rule 5: stale or unknown.
                return Err(Phase1Error::DanglingId(format!(
                    "{item_name} -> Id({}) is not in S and not in A or B paths (stale reference or unknown id)",
                    referenced_id.0
                )));
            } else {
                // B-sourced item: consult only B-side paths.
                if let Some(ps) = b.paths.get(&referenced_id) {
                    if ps.crate_id != 0 {
                        // Rule 2: B-side external-crate item — valid.
                        continue;
                    }
                    // Rule 3: B-side local deleted.
                    return Err(Phase1Error::DanglingId(format!(
                        "{item_name} -> Id({}) not found in S (may have been deleted in Phase 1)",
                        referenced_id.0
                    )));
                }
                // Rule 4: stale or unknown.
                return Err(Phase1Error::DanglingId(format!(
                    "{item_name} -> Id({}) is not in S and not in B paths (stale reference or unknown id)",
                    referenced_id.0
                )));
            }
        }
    }

    Ok(())
}
