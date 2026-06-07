//! Step 5.5 — A-side unified trait-impl insertion loop.
//!
//! Per ADR `2026-05-20-0048` D4, `CatalogueDocument::trait_impls` entries are STANDALONE
//! top-level items in `a_krate.index` with NO parent-type attachment.  The A-side
//! type-processing loop in Step 4 does NOT reach them, so this step handles them explicitly.

use std::collections::{BTreeMap, HashMap, HashSet};

use domain::tddd::Phase1Error;
use domain::tddd::catalogue_v2::ItemAction;
use rustdoc_types::{Crate, Id, ItemEnum};

use super::super::super::build_impl_identity_map;
use super::super::child_items::{
    insert_a_item_tree_into_s, move_standalone_impl_children_to_d, remap_child_ids_in_item,
    remove_b_children_from_s,
};
use super::super::state::Phase1State;
use super::rewrite::rewrite_type_ref_ids_in_item;

/// Processes all standalone A-side impl items (Step 5.5).
///
/// "Standalone" means the impl is in `a_krate.index` with `crate_id == 0` and is NOT
/// reachable via any type/trait child-subtree traversal performed in Step 4.
///
/// # Errors
/// Returns `Phase1Error::ActionContradiction` when an action/baseline combination is
/// inconsistent (e.g. `Add` for an impl that already exists in B).
pub(crate) fn process_standalone_impls(
    state: &mut Phase1State,
    a_krate: &rustdoc_types::Crate,
    a_item_actions: &BTreeMap<Id, ItemAction>,
    b: &Crate,
    crate_name: &str,
) -> Result<(), Phase1Error> {
    // Build B-side impl identity map to find matching B impls for Modify/Reference/Delete.
    // Pass the real crate name so that when rustdoc omits a local trait from `krate.paths`
    // and the fallback `normalize_impl_trait_path` runs, `my_crate::MyTrait` is stripped
    // to `MyTrait` — consistent with Phase 2's S/D/C maps (which also pass `crate_name`).
    let b_impl_map = build_impl_identity_map(b, crate_name);
    // Invert: B identity key → corresponding S-side Id (after b_id_remap renumbering).
    let b_key_to_s_id: std::collections::BTreeMap<&str, Id> = b_impl_map
        .iter()
        .filter_map(|(key, &b_id)| {
            let s_id = state.b_id_remap.get(&b_id).copied().or(Some(b_id))?;
            Some((key.as_str(), s_id))
        })
        .collect();

    // Build A-side impl identity map to get each standalone A impl's identity key.
    // Pass the real crate name for symmetric crate-name normalization.
    let a_impl_map = build_impl_identity_map(a_krate, crate_name);
    // Invert A map: A-side Id → identity key string.
    let a_id_to_key: HashMap<Id, String> =
        a_impl_map.into_iter().map(|(key, id)| (id, key)).collect();

    // Collect impl Ids that are reachable via A-side type/trait subtree traversal in Step 4.
    // An impl is "reachable" iff it is listed in the `impls` field of a Struct or Enum
    // (which `collect_child_ids` always follows), OR it is listed in
    // `Trait.implementations` for a Trait whose own action is `Add` or `Modify`.
    let a_referenced_impl_ids: HashSet<Id> = a_krate
        .index
        .iter()
        .flat_map(|(id, item)| match &item.inner {
            ItemEnum::Struct(s) => s.impls.clone(),
            ItemEnum::Enum(e) => e.impls.clone(),
            ItemEnum::Trait(t) => {
                let action = a_item_actions.get(id).copied();
                if matches!(action, Some(ItemAction::Add) | Some(ItemAction::Modify)) {
                    t.implementations.clone()
                } else {
                    vec![]
                }
            }
            _ => vec![],
        })
        .collect();

    // Collect all standalone A-side Impl items.
    let mut a_standalone_impls: Vec<Id> = a_krate
        .index
        .iter()
        .filter(|(id, item)| {
            item.crate_id == 0
                && matches!(item.inner, ItemEnum::Impl(_))
                && !a_referenced_impl_ids.contains(id)
        })
        .map(|(id, _)| *id)
        .collect();
    // Sort for deterministic processing order.
    a_standalone_impls.sort_by_key(|id| id.0);

    for standalone_id in a_standalone_impls {
        let impl_item = match a_krate.index.get(&standalone_id) {
            Some(it) => it.clone(),
            None => continue,
        };

        // Use the impl's OWN action from `a_item_actions`.
        let own_action = a_item_actions.get(&standalone_id).copied().unwrap_or(ItemAction::Add);

        // Compute the identity key for this A-side impl.
        let impl_key = a_id_to_key.get(&standalone_id).map(String::as_str).unwrap_or("");
        let in_b = !impl_key.is_empty() && b_key_to_s_id.contains_key(impl_key);

        match own_action {
            ItemAction::Add => {
                if in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Add declared for trait impl '{impl_key}' but it already exists in baseline"
                    )));
                }
                let path = a_krate.paths.get(&standalone_id).map(|ps| ps.path.clone());
                insert_a_item_tree_into_s(state, impl_item, ItemAction::Add, path, &a_krate.index);
            }
            ItemAction::Modify => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Modify declared for trait impl '{impl_key}' but it does not exist in baseline"
                    )));
                }
                let s_id = match b_key_to_s_id.get(impl_key).copied() {
                    Some(id) => id,
                    None => {
                        return Err(Phase1Error::ActionContradiction(format!(
                            "action=Modify: trait impl '{impl_key}' expected in S but not found (internal error)"
                        )));
                    }
                };
                // Remove B's impl and children from S before inserting A's version.
                if let Some(b_seeded) = state.s_index.get(&s_id).cloned() {
                    remove_b_children_from_s(&mut state.s_index, &mut state.s_actions, &b_seeded);
                }
                state.s_index.remove(&s_id);
                state.s_actions.remove(&s_id);
                let path = a_krate.paths.get(&standalone_id).map(|ps| ps.path.clone());
                insert_a_item_tree_into_s(
                    state,
                    impl_item,
                    ItemAction::Modify,
                    path,
                    &a_krate.index,
                );
            }
            ItemAction::Reference => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Reference declared for trait impl '{impl_key}' but it does not exist in baseline"
                    )));
                }
                // S should have B's impl as Reference.  However, if the parent type had
                // `action=Modify`, `remove_b_children_from_s` may have evicted this impl.
                // Detect and recover from the eviction.
                if let Some(&s_id) = b_key_to_s_id.get(impl_key) {
                    if !state.s_index.contains_key(&s_id) {
                        // Re-insert B's impl at its pre-allocated S-id.
                        if let Some(&b_id) = b_impl_map.get(impl_key) {
                            if let Some(b_impl_item) = b.index.get(&b_id).cloned() {
                                let rewritten = rewrite_type_ref_ids_in_item(
                                    b_impl_item.clone(),
                                    &state.b_id_remap,
                                );
                                let remapped =
                                    remap_child_ids_in_item(rewritten, &state.b_id_remap);
                                let mut stored = remapped;
                                stored.id = s_id;
                                state.s_index.insert(s_id, stored);
                                state.s_actions.insert(s_id, ItemAction::Reference);
                                // Re-insert children.
                                if let ItemEnum::Impl(ref impl_inner) = b_impl_item.inner {
                                    for &child_id in &impl_inner.items {
                                        if let Some(child) = b.index.get(&child_id) {
                                            let new_child_s_id = state
                                                .b_id_remap
                                                .get(&child_id)
                                                .copied()
                                                .unwrap_or_else(|| state.alloc_id());
                                            let rewritten_child = rewrite_type_ref_ids_in_item(
                                                child.clone(),
                                                &state.b_id_remap,
                                            );
                                            let remapped_child = remap_child_ids_in_item(
                                                rewritten_child,
                                                &state.b_id_remap,
                                            );
                                            let mut stored_child = remapped_child;
                                            stored_child.id = new_child_s_id;
                                            state
                                                .s_index
                                                .entry(new_child_s_id)
                                                .or_insert(stored_child);
                                            state
                                                .s_actions
                                                .insert(new_child_s_id, ItemAction::Reference);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                // Otherwise S already has B's impl as Reference — no change needed.
            }
            ItemAction::Delete => {
                if !in_b {
                    return Err(Phase1Error::ActionContradiction(format!(
                        "action=Delete declared for trait impl '{impl_key}' but it does not exist in baseline"
                    )));
                }
                let s_id = match b_key_to_s_id.get(impl_key).copied() {
                    Some(id) => id,
                    None => {
                        return Err(Phase1Error::ActionContradiction(format!(
                            "action=Delete: trait impl '{impl_key}' expected in S but not found (internal error)"
                        )));
                    }
                };
                // Three cases — see detailed comments in the original builder.rs.
                if state.s_index.contains_key(&s_id) {
                    // Case 1: impl is in S — move children to D, then root.
                    if let Some(s_impl) = state.s_index.get(&s_id).cloned() {
                        move_standalone_impl_children_to_d(
                            &mut state.s_index,
                            &mut state.s_actions,
                            &mut state.d_index,
                            &s_impl,
                        );
                    }
                    if let Some(mut root) = state.s_index.remove(&s_id) {
                        let d_id = state.alloc_id();
                        root.id = d_id;
                        state.d_index.insert(d_id, root);
                        if let Some(ps) = state.s_paths.remove(&s_id) {
                            state.d_paths.insert(d_id, ps);
                        }
                    }
                } else if state.d_index.contains_key(&s_id) {
                    // Case 3: already moved into D by a parent-type Delete pass — no action.
                } else if let Some(&b_id) = b_impl_map.get(impl_key) {
                    // Case 2: evicted from S by parent-type Modify — reconstruct from B.
                    if let Some(b_impl_item) = b.index.get(&b_id).cloned() {
                        let rewritten =
                            rewrite_type_ref_ids_in_item(b_impl_item.clone(), &state.b_id_remap);
                        let remapped = remap_child_ids_in_item(rewritten, &state.b_id_remap);
                        let d_id = state.alloc_id();
                        let mut root = remapped;
                        root.id = d_id;
                        state.d_index.insert(d_id, root);
                        if let ItemEnum::Impl(ref impl_inner) = b_impl_item.inner {
                            for &child_id in &impl_inner.items {
                                if let Some(child) = b.index.get(&child_id) {
                                    let child_d_id = state
                                        .b_id_remap
                                        .get(&child_id)
                                        .copied()
                                        .unwrap_or_else(|| state.alloc_id());
                                    let rewritten_child = rewrite_type_ref_ids_in_item(
                                        child.clone(),
                                        &state.b_id_remap,
                                    );
                                    let remapped_child =
                                        remap_child_ids_in_item(rewritten_child, &state.b_id_remap);
                                    let mut stored_child = remapped_child;
                                    stored_child.id = child_d_id;
                                    state.d_index.insert(child_d_id, stored_child);
                                }
                            }
                        }
                    }
                }
                // Ensure the s_actions entry is removed.
                state.s_actions.remove(&s_id);
            }
        }
    }

    Ok(())
}
