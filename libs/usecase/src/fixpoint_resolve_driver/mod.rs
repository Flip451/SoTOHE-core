//! Driver-level application service for `sotp track fixpoint-resolve`.
//!
//! [`FixpointResolveDriverInteractor`] performs the entire fixpoint-resolve
//! orchestration itself (ADR 2026-06-21-1328 D4): it validates the raw
//! `track_id` / `current_branch` strings, enforces the `TrackNotActive`
//! branch-mismatch guard, resolves the workspace context, loads the dry-check
//! config, runs the D4 dry gate, builds the review / ref-verify gate adapters,
//! and finally invokes the existing [`crate::fixpoint_resolve::FixpointResolveService`]
//! directly — no infrastructure adapter sits between this interactor and the
//! domain-level resolve call. This mirrors [`crate::fixpoint_resolve::FixpointDryGateInteractor`]'s
//! multi-fine-grained-port-factory shape, not [`crate::dry_driver::DryDriverInteractor`]'s
//! single-port pass-through shape.
//!
//! This module is purely additive: it does not modify
//! [`crate::fixpoint_resolve`] or any of its ports/services — it only
//! consumes them.
//!
//! Design: ADR 2026-06-21-1328 D2 / D4 / D7, IN-12, AC-17, CN-07.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::DryCheckConfigFingerprint;

use crate::dry_check::DryCheckConfig;
use crate::fixpoint_resolve::{
    FixpointCurrentBranch, FixpointDryGateCommand, FixpointDryGateService, FixpointResolveCommand,
    FixpointResolveError, FixpointResolveInteractor, FixpointResolveService as _,
    RefVerifyGateStatePort, ReviewGateStatePort,
};

// ── FixpointResolveDriverInput ────────────────────────────────────────────────

/// Input for `sotp track fixpoint-resolve` (driver boundary).
///
/// Field-for-field mirror of the raw CLI arguments; no domain validation has
/// happened yet — [`FixpointResolveDriverInteractor::fixpoint_resolve`]
/// performs it.
#[derive(Debug, Clone)]
pub struct FixpointResolveDriverInput {
    /// Active track ID (directory name under `items_dir/<id>`).
    pub track_id: String,
    /// Current git branch label (e.g. `"track/my-feature-2026"`).
    pub current_branch: String,
    /// Path to the `track/items` directory.
    pub items_dir: PathBuf,
}

// ── FixpointResolveDriverOutcome ──────────────────────────────────────────────

/// Outcome of `sotp track fixpoint-resolve` (driver boundary).
///
/// Mirrors the four [`domain::track_phase::FixpointStep`] variants plus a
/// `Failure` variant carrying an opaque diagnostic message, so `cli_driver`
/// can render the result without depending on `domain` or `usecase` error
/// types directly.
#[derive(Debug, Clone)]
pub enum FixpointResolveDriverOutcome {
    /// The DRY gate is open; the caller must run DFP next.
    RunDfp,
    /// One or more review scopes are stale; the caller must run RFP next.
    RunRfp {
        /// Scope labels that must run RFP, in sorted order.
        scopes: Vec<String>,
    },
    /// The ref-verify gate is blocked; the caller must run ref-verify next.
    RunRefVerify,
    /// All gates are green; the caller may commit.
    Commit,
    /// Validation, workspace resolution, or a gate query failed.
    Failure {
        /// Human-readable diagnostic message.
        message: String,
    },
}

// ── FixpointWorkspaceContext ──────────────────────────────────────────────────

/// Resolved workspace paths and effective base branch for a fixpoint-resolve run.
#[derive(Debug, Clone)]
pub struct FixpointWorkspaceContext {
    /// Git repository root (trust anchor for all git operations).
    pub repo_root: PathBuf,
    /// Canonicalized, containment-checked `track/items` directory.
    pub canonical_items_dir: PathBuf,
    /// Canonical project root (parent of `track/`).
    pub canonical_root: PathBuf,
    /// The active track's configured base branch (from `branch_strategy_snapshot`).
    pub base_branch: String,
}

// ── FixpointWorkspaceContextError ─────────────────────────────────────────────

/// Error returned by [`FixpointWorkspaceContextPort::resolve_context`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum FixpointWorkspaceContextError {
    /// Workspace resolution failed (git discovery, path containment, or metadata read/decode).
    #[error("{0}")]
    Unavailable(String),
}

// ── DryCheckConfigLoaderError ─────────────────────────────────────────────────

/// Error returned by [`DryCheckConfigLoaderPort::load`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryCheckConfigLoaderError {
    /// The dry-check config could not be loaded or lifted into the usecase newtypes.
    #[error("{0}")]
    Unavailable(String),
}

// ── FixpointWorkspaceContextPort ──────────────────────────────────────────────

/// Secondary port resolving the workspace context for a fixpoint-resolve run.
///
/// Abstracts git repo discovery, `items_dir` canonicalization/containment,
/// project-root derivation, and the `base_branch` read from the track's
/// `metadata.json#branch_strategy_snapshot`, keeping the usecase interactor
/// free of `std::fs` / `std::env` / git process calls.
pub trait FixpointWorkspaceContextPort: Send + Sync {
    /// Resolve the workspace context for `track_id` given the caller-supplied `items_dir`.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointWorkspaceContextError::Unavailable`] on git discovery,
    /// path containment, or metadata read/decode failure.
    fn resolve_context(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<FixpointWorkspaceContext, FixpointWorkspaceContextError>;
}

// ── DryCheckConfigLoaderPort ──────────────────────────────────────────────────

/// Secondary port loading `.harness/config/dry-check.json` and lifting it into
/// the usecase [`DryCheckConfig`] newtypes.
pub trait DryCheckConfigLoaderPort: Send + Sync {
    /// Load the dry-check config from `repo_root` and return it alongside its
    /// SHA-256 fingerprint.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckConfigLoaderError::Unavailable`] on load or validation failure.
    fn load(
        &self,
        repo_root: &Path,
    ) -> Result<(DryCheckConfig, DryCheckConfigFingerprint), DryCheckConfigLoaderError>;
}

// ── FixpointDryGateFactoryPort ────────────────────────────────────────────────

/// Secondary port constructing a [`FixpointDryGateService`] for the given base branch.
pub trait FixpointDryGateFactoryPort: Send + Sync {
    /// Build a [`FixpointDryGateService`] wired for `base_branch`.
    fn build(&self, base_branch: &str) -> Arc<dyn FixpointDryGateService>;
}

// ── FixpointGateStateFactoryPort ──────────────────────────────────────────────

/// Secondary port constructing the review and ref-verify gate-state adapters.
pub trait FixpointGateStateFactoryPort: Send + Sync {
    /// Build a [`ReviewGateStatePort`] anchored to `items_dir`, evaluating diff
    /// bases against `base_branch`.
    fn build_review_gate(
        &self,
        items_dir: &Path,
        base_branch: &str,
    ) -> Arc<dyn ReviewGateStatePort>;

    /// Build a [`RefVerifyGateStatePort`] anchored to `items_dir`.
    fn build_ref_verify_gate(&self, items_dir: &Path) -> Arc<dyn RefVerifyGateStatePort>;
}

// ── FixpointResolveDriverService ──────────────────────────────────────────────

/// Application service (primary port) for `sotp track fixpoint-resolve`.
pub trait FixpointResolveDriverService: Send + Sync {
    /// Resolve the next fixpoint step for the given input.
    ///
    /// Never panics; all failure modes are reported via
    /// [`FixpointResolveDriverOutcome::Failure`].
    fn fixpoint_resolve(&self, input: FixpointResolveDriverInput) -> FixpointResolveDriverOutcome;
}

// ── FixpointResolveDriverInteractor ───────────────────────────────────────────

/// Interactor implementing [`FixpointResolveDriverService`].
///
/// Holds the four secondary ports above as private `Arc<dyn Port>` fields and
/// performs the entire fixpoint-resolve orchestration itself.
pub struct FixpointResolveDriverInteractor {
    workspace_context: Arc<dyn FixpointWorkspaceContextPort>,
    dry_config_loader: Arc<dyn DryCheckConfigLoaderPort>,
    dry_gate_factory: Arc<dyn FixpointDryGateFactoryPort>,
    gate_state_factory: Arc<dyn FixpointGateStateFactoryPort>,
}

impl FixpointResolveDriverInteractor {
    /// Construct a new [`FixpointResolveDriverInteractor`] with all required ports.
    #[must_use]
    pub fn new(
        workspace_context: Arc<dyn FixpointWorkspaceContextPort>,
        dry_config_loader: Arc<dyn DryCheckConfigLoaderPort>,
        dry_gate_factory: Arc<dyn FixpointDryGateFactoryPort>,
        gate_state_factory: Arc<dyn FixpointGateStateFactoryPort>,
    ) -> Self {
        Self { workspace_context, dry_config_loader, dry_gate_factory, gate_state_factory }
    }
}

impl FixpointResolveDriverService for FixpointResolveDriverInteractor {
    fn fixpoint_resolve(&self, input: FixpointResolveDriverInput) -> FixpointResolveDriverOutcome {
        let FixpointResolveDriverInput { track_id, current_branch, items_dir } = input;

        // ── Validate track_id and current_branch ──────────────────────────────
        let track_id = match TrackId::try_new(track_id) {
            Ok(id) => id,
            Err(e) => return FixpointResolveDriverOutcome::Failure { message: e.to_string() },
        };
        let current_branch = match FixpointCurrentBranch::try_new(current_branch) {
            Ok(b) => b,
            Err(e) => return FixpointResolveDriverOutcome::Failure { message: e.to_string() },
        };

        // ── Track-not-active guard ─────────────────────────────────────────────
        let expected_branch = format!("track/{}", track_id.as_ref());
        if current_branch.as_str() != expected_branch {
            return FixpointResolveDriverOutcome::Failure {
                message: FixpointResolveError::TrackNotActive { branch: expected_branch }
                    .to_string(),
            };
        }

        // ── Resolve workspace context ──────────────────────────────────────────
        let context = match self.workspace_context.resolve_context(&items_dir, &track_id) {
            Ok(c) => c,
            Err(e) => return FixpointResolveDriverOutcome::Failure { message: e.to_string() },
        };

        // ── Load dry-check config ──────────────────────────────────────────────
        let (dry_config, current_config_fingerprint) =
            match self.dry_config_loader.load(&context.repo_root) {
                Ok(v) => v,
                Err(e) => return FixpointResolveDriverOutcome::Failure { message: e.to_string() },
            };

        // ── Run the D4 dry gate ─────────────────────────────────────────────────
        let dry_gate = self.dry_gate_factory.build(&context.base_branch);
        let track_dir = context.canonical_items_dir.join(track_id.as_ref());
        let dry_gate_output = match dry_gate.resolve_dry_gate(FixpointDryGateCommand {
            track_dir,
            canonical_root: context.canonical_root.clone(),
            repo_root: context.repo_root.clone(),
            dry_config: dry_config.clone(),
            current_config_fingerprint,
        }) {
            Ok(output) => output,
            Err(e) => return FixpointResolveDriverOutcome::Failure { message: e.to_string() },
        };

        // ── Build review / ref-verify gate adapters ─────────────────────────────
        let review_gate = self
            .gate_state_factory
            .build_review_gate(&context.canonical_items_dir, &context.base_branch);
        let ref_verify_gate =
            self.gate_state_factory.build_ref_verify_gate(&context.canonical_items_dir);

        // ── Invoke the existing FixpointResolveService directly ────────────────
        let interactor = FixpointResolveInteractor::new(
            dry_config,
            dry_gate_output.dry_approval,
            review_gate,
            ref_verify_gate,
        );
        let cmd = FixpointResolveCommand {
            track_id,
            current_branch,
            current_fragment_refs: dry_gate_output.current_fragment_refs,
        };

        match interactor.resolve(&cmd) {
            Ok(domain::track_phase::FixpointStep::RunDfp) => FixpointResolveDriverOutcome::RunDfp,
            Ok(domain::track_phase::FixpointStep::RunRfp { scopes }) => {
                FixpointResolveDriverOutcome::RunRfp {
                    scopes: scopes.as_set().iter().cloned().collect(),
                }
            }
            Ok(domain::track_phase::FixpointStep::RunRefVerify) => {
                FixpointResolveDriverOutcome::RunRefVerify
            }
            Ok(domain::track_phase::FixpointStep::Commit) => FixpointResolveDriverOutcome::Commit,
            Err(e) => FixpointResolveDriverOutcome::Failure { message: e.to_string() },
        }
    }
}

#[cfg(test)]
mod tests;
