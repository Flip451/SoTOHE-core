//! Shared DTOs and secondary ports for the three IN-14 `dry` driver interactors
//! (`DryWriteDriverInteractor` / `DryResultsDriverInteractor` /
//! `DryCheckApprovedDriverInteractor`, added by T027/T029/T030).
//!
//! [`DryRepoWorkspace`] mirrors [`crate::fixpoint_resolve_driver::FixpointWorkspaceContext`]'s
//! shape minus `base_branch` — `base_branch` resolution is a separate port
//! ([`DryBaseBranchPort`]) so `dry results` (which never reads `metadata.json`
//! today) does not gain a new fail-closed metadata dependency (CN-07).
//!
//! This module is purely additive: nothing outside it references these types
//! yet, so it compiles standalone.
//!
//! Design: ADR 2026-06-21-1328 D2 / D4 / D7, IN-14, AC-20.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::TrackId;

// ── DryRepoWorkspace ──────────────────────────────────────────────────────────

/// Resolved repository/workspace paths for a `dry` driver run.
///
/// Mirrors [`crate::fixpoint_resolve_driver::FixpointWorkspaceContext`]'s shape
/// minus `base_branch` (resolved separately via [`DryBaseBranchPort`]).
#[derive(Debug, Clone)]
pub struct DryRepoWorkspace {
    /// Git repository root (trust anchor for all git operations).
    pub repo_root: PathBuf,
    /// Canonicalized project root (`repo_root` canonicalized).
    pub canonical_root: PathBuf,
    /// Canonicalized, containment-checked `track/items` directory.
    pub canonical_items_dir: PathBuf,
}

// ── DryRepoWorkspaceError ─────────────────────────────────────────────────────

/// Error returned by [`DryRepoRootPort::resolve`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryRepoWorkspaceError {
    /// Repo discovery, canonicalization, or `items_dir` containment failed.
    #[error("{0}")]
    Unavailable(String),
}

// ── DryBaseBranchError ────────────────────────────────────────────────────────

/// Error returned by [`DryBaseBranchPort::resolve_base_branch`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryBaseBranchError {
    /// `metadata.json` is missing, unreadable, or its `branch_strategy_snapshot`
    /// could not be decoded. Fail-closed per IN-06/IN-07: there is no
    /// global branch-strategy config fallback.
    #[error("{0}")]
    Unavailable(String),
}

// ── DryCheckStorageHandle ─────────────────────────────────────────────────────

/// Bundle of dry-check storage ports bound to a single track directory.
#[derive(Clone)]
pub struct DryCheckStorageHandle {
    /// Read port for `dry-check.json`.
    pub reader: Arc<dyn domain::dry_check::DryCheckReader>,
    /// Write port for `dry-check.json`.
    pub writer: Arc<dyn domain::dry_check::DryCheckWriter>,
    /// Coverage manifest port for `dry-check-coverage.json`.
    pub coverage: Arc<dyn crate::dry_check::DryCheckCoveragePort>,
}

// ── DryRepoRootPort ───────────────────────────────────────────────────────────

/// Secondary port resolving the repository/workspace paths for a `dry` driver run.
///
/// Abstracts CWD-anchored git repo discovery plus canonicalization/containment
/// validation of `items_dir`, reproducing the repo-root-discovery preamble
/// currently duplicated inline at the top of `DryCompositionRoot::dry_write` /
/// `dry_results` / `dry_check_approved`.
pub trait DryRepoRootPort: Send + Sync {
    /// Resolve the repository/workspace paths for the given `items_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`DryRepoWorkspaceError::Unavailable`] on git discovery,
    /// canonicalization, or `items_dir` containment failure.
    fn resolve(&self, items_dir: &Path) -> Result<DryRepoWorkspace, DryRepoWorkspaceError>;
}

// ── DryBaseBranchPort ─────────────────────────────────────────────────────────

/// Secondary port resolving the active track's configured base branch.
///
/// Reads `<track_dir>/metadata.json#branch_strategy_snapshot.base_branch`.
/// Fail-closed per IN-06/IN-07: a missing `metadata.json` or any decode error
/// propagates as [`DryBaseBranchError::Unavailable`] — there is no
/// `.harness/config/branch-strategy.json` fallback and no hardcoded
/// branch-name default.
pub trait DryBaseBranchPort: Send + Sync {
    /// Resolve the configured base branch for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`DryBaseBranchError::Unavailable`] when `metadata.json` is
    /// missing, unreadable, or cannot be decoded.
    fn resolve_base_branch(
        &self,
        canonical_root: &Path,
        track_dir: &Path,
        track_id: &TrackId,
    ) -> Result<String, DryBaseBranchError>;
}

// ── DryCheckStorageFactoryPort ────────────────────────────────────────────────

/// Secondary port constructing the reader/writer/coverage trio bound to a
/// track's `dry-check.json` / `dry-check-coverage.json`.
///
/// Infallible pure construction — no I/O occurs at `build()` time.
pub trait DryCheckStorageFactoryPort: Send + Sync {
    /// Build a [`DryCheckStorageHandle`] for the given track directory.
    fn build(&self, track_dir: &Path, canonical_root: &Path) -> DryCheckStorageHandle;
}

// ── DryDiffBaseFactoryPort ────────────────────────────────────────────────────

/// Secondary port constructing a [`crate::fixpoint_resolve::DiffBaseResolverPort`]
/// bound to a given base branch.
///
/// [`crate::fixpoint_resolve::DiffBaseResolverPort`]'s only existing adapter
/// (`FsDiffBaseResolverAdapter`) requires `base_branch` fixed at construction
/// time, but the driver interactors resolve `base_branch` themselves at
/// request time via [`DryBaseBranchPort`] and are wired once per CLI process
/// before any `track_id` is known — a permanently-injected
/// `Arc<dyn DiffBaseResolverPort>` field would bind every request to whichever
/// `base_branch` happened to be current at wiring time. This factory port
/// mirrors [`crate::fixpoint_resolve_driver::FixpointDryGateFactoryPort`]'s
/// already-established construction-time-vs-request-time solution for the
/// same underlying adapter.
///
/// Infallible — the underlying adapter performs no fallible I/O at
/// construction time.
pub trait DryDiffBaseFactoryPort: Send + Sync {
    /// Build a [`crate::fixpoint_resolve::DiffBaseResolverPort`] wired for `base_branch`.
    fn build(&self, base_branch: &str) -> Arc<dyn crate::fixpoint_resolve::DiffBaseResolverPort>;
}
