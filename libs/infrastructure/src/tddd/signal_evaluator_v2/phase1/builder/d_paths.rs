//! D-side path population for Phase 1.

use std::collections::{HashMap, HashSet};

use rustdoc_types::{Crate, Id};

use super::super::super::collect_refs::collect_referenced_ids;
use super::super::state::Phase1State;

/// Adds authoritative B summaries for every type reference reachable from D.
///
/// B local items are remapped into S/D's fresh id space, while external paths
/// retain their rustdoc ids. The inverse map keeps those two forms distinct.
pub(super) fn populate_d_paths(state: &mut Phase1State, b: &Crate) {
    let b_id_inverse: HashMap<Id, Id> =
        state.b_id_remap.iter().map(|(&old_id, &new_id)| (new_id, old_id)).collect();
    let mut referenced_ids = HashSet::new();
    for item in state.d_index.values() {
        referenced_ids.extend(collect_referenced_ids(item));
    }

    for ref_id in referenced_ids {
        if let std::collections::hash_map::Entry::Vacant(entry) = state.d_paths.entry(ref_id) {
            let b_id = b_id_inverse.get(&ref_id).copied().unwrap_or(ref_id);
            if let Some(summary) = b.paths.get(&b_id) {
                // A deleted impl may reference a local type that remains in S;
                // retain its path so the D-side identity remains authoritative.
                entry.insert(summary.clone());
            }
        }
    }
}
