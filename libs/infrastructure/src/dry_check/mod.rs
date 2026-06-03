//! Dry-check infrastructure adapters.
//!
//! Provides four adapters (CN-01: all dry-check-owned, independent from
//! `review_v2` adapters):
//!
//! - [`FsDryCheckStore`]: filesystem persistence for dry-check records.
//! - [`GitDryCheckDiffGetter`]: git-based diff-source returning hunk ranges.
//! - [`FsDryCheckCommitHashStore`]: filesystem reader for the `.commit_hash` file.
//! - [`CodexDryChecker`]: Codex-backed agent adapter implementing `DryCheckAgentPort`.

mod codec;
mod codex_dry_checker;
mod commit_hash_store;
mod diff_getter;
mod store;

pub use codex_dry_checker::CodexDryChecker;
pub use commit_hash_store::{DryCheckCommitHashError, FsDryCheckCommitHashStore};
pub use diff_getter::GitDryCheckDiffGetter;
pub use store::FsDryCheckStore;
