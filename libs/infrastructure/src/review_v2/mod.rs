//! Review System v2 infrastructure adapters.
//!
//! Implements usecase port traits using git CLI and filesystem I/O.

pub mod diff_getter;
pub mod hasher;

pub use diff_getter::GitDiffGetter;
pub use hasher::SystemReviewHasher;
