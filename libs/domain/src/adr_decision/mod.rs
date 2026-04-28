//! ADR decision lifecycle typestate cluster, value objects, and signal evaluation.
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
//! - [`DecisionGrounds`]: orthogonal classification of a decision's grounds.
//! - [`AdrFrontMatter`] / [`AdrFrontMatterError`]: parsed YAML front-matter aggregate consumed by the
//!   usecase layer.
//! - [`AdrVerifyReport`]: aggregate signal counts produced by
//!   `verify-adr-signals`.
//! - [`evaluate_adr_decision`]: pure free function classifying a single
//!   [`AdrDecisionEntry`] into a [`DecisionGrounds`] signal.
//! - [`AdrFilePort`] / [`AdrFilePortError`]: domain secondary port for
//!   adapter-backed ADR file enumeration and front-matter parsing.
//!
//! All types are serde-free — serialization and deserialization live in the
//! infrastructure adapter per the CN-05 hexagonal architecture rule.

pub mod common;
pub mod entry;
pub mod evaluator;
pub mod file_port;
pub mod front_matter;
pub mod grounds;
pub mod state;
pub mod verify_report;

pub use common::{AdrDecisionCommon, AdrDecisionCommonError};
pub use entry::AdrDecisionEntry;
pub use evaluator::evaluate_adr_decision;
pub use file_port::{AdrFilePort, AdrFilePortError};
pub use front_matter::{AdrFrontMatter, AdrFrontMatterError};
pub use grounds::DecisionGrounds;
pub use state::{
    AcceptedDecision, DeprecatedDecision, ImplementedDecision, ProposedDecision, SupersededDecision,
};
pub use verify_report::AdrVerifyReport;
