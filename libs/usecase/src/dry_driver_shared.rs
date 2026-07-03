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

// ── IoFailureDetail ────────────────────────────────────────────────────────────

/// Opaque diagnostic-text carrier for a [`std::io::Error`] encountered during a
/// filesystem operation (canonicalize / read / write / create_dir / symlink
/// inspection) at a driver-error construction site.
///
/// `std::io::Error` itself is not `Clone` and infrastructure adapters cannot
/// pass their own error types across the infrastructure→usecase boundary, so
/// the adapter renders the `io::Error` via `Display` and wraps the resulting
/// text in this usecase-local newtype.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IoFailureDetail(String);

impl IoFailureDetail {
    /// Wrap the rendered `Display` text of an infrastructure I/O error.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for IoFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── GitDiscoveryFailureDetail ─────────────────────────────────────────────────

/// Opaque diagnostic-text carrier for `infrastructure::git_cli::GitError`
/// produced by a failed `SystemGitRepo::discover()` call.
///
/// `GitError` is infrastructure-only and not `Clone`, so it cannot cross the
/// infrastructure→usecase boundary directly; the adapter renders it via
/// `Display` and wraps the resulting text here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitDiscoveryFailureDetail(String);

impl GitDiscoveryFailureDetail {
    /// Wrap the rendered `Display` text of a failed git-discovery call.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for GitDiscoveryFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── MetadataDecodeFailureDetail ───────────────────────────────────────────────

/// Opaque diagnostic-text carrier for `infrastructure::track::codec::CodecError`
/// produced when `metadata.json` fails to decode into `TrackMetadata` (JSON
/// parse error, domain validation error, invalid field, or pre-v6 schema
/// mismatch — IN-07 fail-closed).
///
/// `CodecError` is infrastructure-only and not `Clone` (it wraps
/// `serde_json::Error`), so it cannot cross the infrastructure→usecase
/// boundary directly; the adapter renders it via `Display` and wraps the
/// resulting text here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MetadataDecodeFailureDetail(String);

impl MetadataDecodeFailureDetail {
    /// Wrap the rendered `Display` text of a failed `metadata.json` decode.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for MetadataDecodeFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

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
    /// `SystemGitRepo::discover()` failed.
    #[error("git discover: {detail}")]
    GitDiscoveryFailed {
        /// Rendered `GitError` diagnostic text.
        detail: GitDiscoveryFailureDetail,
    },
    /// The discovered repo root could not be canonicalized.
    #[error("failed to canonicalize repo root '{}': {detail}", repo_root.display())]
    RepoRootCanonicalizeFailed {
        /// The repo root that failed to canonicalize.
        repo_root: PathBuf,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// The symlink guard rejected `items_dir` — either an I/O error
    /// inspecting it, or a definitively-found symlink.
    #[error("symlink guard items_dir '{}': {detail}", items_dir.display())]
    ItemsDirSymlinkRejected {
        /// The rejected `items_dir` path.
        items_dir: PathBuf,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// `items_dir` does not exist, is not a directory, or resolves outside
    /// the repository root.
    #[error(
        "items_dir '{}' must be an existing directory under the repository root",
        items_dir.display()
    )]
    ItemsDirInvalid {
        /// The invalid `items_dir` path.
        items_dir: PathBuf,
    },
}

// ── DryBaseBranchError ────────────────────────────────────────────────────────

/// Error returned by [`DryBaseBranchPort::resolve_base_branch`].
///
/// Fail-closed per IN-06/IN-07: there is no `.harness/config/branch-strategy.json`
/// fallback.
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryBaseBranchError {
    /// The resolved `metadata.json` path escapes `canonical_root`.
    #[error(
        "metadata.json path '{}' resolves outside repo root '{}'",
        metadata_path.display(),
        canonical_root.display()
    )]
    MetadataPathOutsideRepo {
        /// The out-of-bounds `metadata.json` path.
        metadata_path: PathBuf,
        /// The canonicalized repo root the path escaped.
        canonical_root: PathBuf,
    },
    /// The symlink guard rejected `metadata.json`.
    #[error("symlink guard metadata.json for '{}': {detail}", track_id.as_ref())]
    MetadataSymlinkRejected {
        /// The active track.
        track_id: TrackId,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// `metadata.json` does not exist.
    #[error("metadata.json for '{}' not found", track_id.as_ref())]
    MetadataNotFound {
        /// The active track.
        track_id: TrackId,
    },
    /// `metadata.json` exists but could not be read.
    #[error("read metadata.json for '{}': {detail}", track_id.as_ref())]
    MetadataReadFailed {
        /// The active track.
        track_id: TrackId,
        /// Rendered `io::Error` diagnostic text.
        detail: IoFailureDetail,
    },
    /// `metadata.json` was read but failed to decode into `TrackMetadata`.
    #[error(
        "decode metadata.json for '{}': {detail} (track metadata schema v6 required)",
        track_id.as_ref()
    )]
    MetadataDecodeFailed {
        /// The active track.
        track_id: TrackId,
        /// Rendered `CodecError` diagnostic text.
        detail: MetadataDecodeFailureDetail,
    },
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
    /// Returns a [`DryRepoWorkspaceError`] variant on git discovery,
    /// canonicalization, or `items_dir` containment failure.
    fn resolve(&self, items_dir: &Path) -> Result<DryRepoWorkspace, DryRepoWorkspaceError>;
}

// ── DryBaseBranchPort ─────────────────────────────────────────────────────────

/// Secondary port resolving the active track's configured base branch.
///
/// Reads `<track_dir>/metadata.json#branch_strategy_snapshot.base_branch`.
/// Fail-closed per IN-06/IN-07: a missing `metadata.json` or any decode error
/// propagates as a [`DryBaseBranchError`] variant — there is no
/// `.harness/config/branch-strategy.json` fallback and no hardcoded
/// branch-name default.
pub trait DryBaseBranchPort: Send + Sync {
    /// Resolve the configured base branch for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns a [`DryBaseBranchError`] variant when `metadata.json` is
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
