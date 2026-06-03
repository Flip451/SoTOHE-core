//! Dry-check infrastructure adapters.
//!
//! Provides three adapters (CN-01: all dry-check-owned, independent from
//! `review_v2` adapters):
//!
//! - [`FsDryCheckStore`]: filesystem persistence for dry-check records.
//! - [`GitDryCheckDiffGetter`]: git-based diff-source returning hunk ranges.
//! - [`FsDryCheckCommitHashStore`]: filesystem reader for the `.commit_hash` file.

mod codec;
mod commit_hash_store;
mod diff_getter;
mod store;

pub use commit_hash_store::{DryCheckCommitHashError, FsDryCheckCommitHashStore};
pub use diff_getter::GitDryCheckDiffGetter;
pub use store::FsDryCheckStore;
