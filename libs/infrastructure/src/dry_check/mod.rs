//! Dry-check infrastructure adapters.
//!
//! Provides four adapters (CN-01: all dry-check-owned, independent from
//! `review_v2` adapters):
//!
//! - [`FsDryCheckStore`]: filesystem persistence for dry-check records.
//! - [`GitDryCheckDiffGetter`]: git-based diff-source returning hunk ranges.
//! - [`FsDryCheckCommitHashStore`]: filesystem reader for the `.commit_hash` file.
//! - [`CodexDryChecker`]: Codex-backed agent adapter implementing `DryCheckAgentPort`.
//!
//! Also provides the `.harness/config/dry-check.json` loader:
//!
//! - [`DryCheckConfig`]: loaded dry-check configuration (threshold, schema_version).
//! - [`DryCheckConfigError`]: errors from [`DryCheckConfig::load`].

mod codec;
mod codex_dry_checker;
mod commit_hash_store;
pub mod config;
pub mod corpus;
mod coverage;
mod diff_getter;
pub mod recording_agent;
mod store;

pub use codex_dry_checker::CodexDryChecker;
pub use commit_hash_store::{DryCheckCommitHashError, FsDryCheckCommitHashStore};
pub use config::{DryCheckConfig, DryCheckConfigError};
pub use corpus::compute_corpus_fingerprint;
pub use coverage::FsDryCheckCoverageAdapter;
pub use diff_getter::GitDryCheckDiffGetter;
pub use recording_agent::RecordingDryAgent;
pub use store::FsDryCheckStore;
