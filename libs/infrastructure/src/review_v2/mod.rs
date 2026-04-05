//! Review System v2 infrastructure adapters.
//!
//! Implements usecase and domain port traits using git CLI and filesystem I/O.

pub mod diff_getter;
pub mod hasher;
pub mod persistence;

pub use diff_getter::GitDiffGetter;
pub use hasher::SystemReviewHasher;
pub use persistence::{FsCommitHashStore, FsReviewStore};
