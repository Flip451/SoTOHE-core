//! Structured references from plan artefacts (spec.json / type catalogues /
//! impl-plan.json / task-coverage.json) to their upstream sources along the
//! SoT Chain.
//!
//! Each ref struct corresponds to one direction along the chain and is an
//! independent struct (no shared trait / discriminated union); the containing
//! field name is what tells downstream code which direction the reference
//! points.
//!
//! See `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md`
//! §D2.1 for the rationale.

mod adr_ref;
mod convention_ref;
mod informal_ground_ref;
mod spec_ref;

pub use adr_ref::{AdrAnchor, AdrRef};
pub use convention_ref::{ConventionAnchor, ConventionRef};
pub use informal_ground_ref::{InformalGroundKind, InformalGroundRef, InformalGroundSummary};
pub use spec_ref::{ContentHash, SpecElementId, SpecRef};
