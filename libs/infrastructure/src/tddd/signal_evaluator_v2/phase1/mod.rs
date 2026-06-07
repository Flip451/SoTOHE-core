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

pub(crate) use builder::phase1_build_s_and_d;
