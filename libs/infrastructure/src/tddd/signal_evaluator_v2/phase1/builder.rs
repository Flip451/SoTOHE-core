//! Phase 1 builder — split into sub-modules for manageability.
//!
//! ## Sub-modules
//!
//! - `main_fn`       — `phase1_build_s_and_d` main entry-point (Steps 1-6, Phase 1.45/1.5)
//! - `step55_impls`  — Step 5.5: standalone A-side trait-impl insertion loop
//! - `phase16_check` — Phase 1.6: dangling-Id validation
//! - `rewrite`       — Type-ref Id rewriting helpers + `make_root_module_item`

mod main_fn;
mod phase16_check;
mod rewrite;
mod step55_impls;

pub(crate) use main_fn::phase1_build_s_and_d;
pub(crate) use rewrite::rewrite_type_ref_ids_in_item;
