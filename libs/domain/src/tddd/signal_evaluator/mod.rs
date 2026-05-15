//! Signal evaluator domain types and port declaration for TDDD v2.
//!
//! ## Modules
//!
//! - [`region`]: `SignalRegion` (12 variants) / `ThreeWaySignalKind` / `ThreeWaySignal` /
//!   `ThreeWayEvaluationReport` / `signal_for_region` helper.
//! - [`phase1_error`]: `Phase1Error` (`ActionContradiction` / `UnresolvedTypeRef` / `DanglingId`).
//! - [`port`]: `SignalEvaluatorPort` trait (secondary port declared in domain).
//!
//! ## Design overview (ADR 3)
//!
//! The signal evaluator runs in two phases:
//!
//! **Phase 1 — S / D construction** (`SignalEvaluatorPort` implementor, T007):
//!
//! 1. Take TypeGraph A (`ExtendedCrate`) + B (`rustdoc_types::Crate`) as inputs.
//! 2. Build S (`ExtendedCrate`): start from B (all items `Reference`), then apply
//!    A's declare actions (Add / Modify / Reference / Delete) to produce the
//!    "expected C" state.  Delete-declared items are moved to D.
//! 3. Build D (`rustdoc_types::Crate`): items deleted from S (implicit Delete context).
//! 4. Phase 1.5 — closed-world check: resolve A's unresolved TypeRef markers using
//!    Delete-processed S as the universe.  Items not found → `Phase1Error::UnresolvedTypeRef`.
//! 5. Phase 1.6 — dangling Id check: verify no Id in S points to a deleted item.
//!    Dangling Ids → `Phase1Error::DanglingId`.
//!
//! Any declare inconsistency detected in Phase 1 is returned as `Phase1Error`.
//!
//! **Phase 2 — 3-way evaluation** (`SignalEvaluatorPort` implementor, T007):
//!
//! Evaluates S / D / C in 11 logical rows (12 `SignalRegion` variants) to produce
//! a `ThreeWayEvaluationReport`.  Phase 2 is a total function; no errors can arise.
//!
//! ## Domain layer boundary
//!
//! This module declares **only types and the port trait**.  The Phase 1 + Phase 2
//! algorithm lives in the infrastructure layer (`SignalEvaluatorV2`, T007).
//!
//! No serde derives — per ADR `knowledge/adr/2026-04-14-1531-domain-serde-ripout.md`,
//! the domain layer is serialization-free.

pub mod phase1_error;
pub mod port;
pub mod region;

// Re-exports for ergonomic access from parent module callers.

pub use phase1_error::Phase1Error;
pub use port::SignalEvaluatorPort;
pub use region::{SignalRegion, ThreeWayEvaluationReport, ThreeWaySignal, ThreeWaySignalKind};
