//! ADR decision lifecycle typestate cluster, enum wrapper, and grounds types.
//!
//! This module provides:
//!
//! - Five independent typestate structs for ADR decision lifecycle:
//!   [`ProposedDecision`], [`AcceptedDecision`], [`ImplementedDecision`],
//!   [`SupersededDecision`], [`DeprecatedDecision`].
//! - [`AdrDecisionEntry`]: heterogeneous-collection boundary enum wrapping the
//!   five typestates, used in `Vec<AdrDecisionEntry>`.
//! - [`AdrDecisionCommon`]: shared grounds/identity value object embedded in
//!   every typestate.
//! - [`DecisionGrounds`]: orthogonal classification of a decision's grounds
//!   (used by `AdrSignalEvaluator` in T002).
//!
//! All types are serde-free — serialization and deserialization live in the
//! infrastructure adapter per the CN-05 hexagonal architecture rule.

pub mod common;
pub mod entry;
pub mod grounds;
pub mod state;

pub use common::{AdrDecisionCommon, AdrDecisionCommonError};
pub use entry::AdrDecisionEntry;
pub use grounds::DecisionGrounds;
pub use state::{
    AcceptedDecision, DeprecatedDecision, ImplementedDecision, ProposedDecision, SupersededDecision,
};
