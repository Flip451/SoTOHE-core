//! Review System v2 infrastructure adapters.
//!
//! Implements usecase and domain port traits using git CLI and filesystem I/O.

pub mod codex_reviewer;
pub mod diff_getter;
pub mod hasher;
pub mod persistence;
pub mod scope_config_loader;

pub use codex_reviewer::CodexReviewer;
pub use diff_getter::GitDiffGetter;
pub use hasher::SystemReviewHasher;
pub use persistence::{FsCommitHashStore, FsReviewStore};
pub use scope_config_loader::{ScopeConfigLoadError, load_v2_scope_config};
