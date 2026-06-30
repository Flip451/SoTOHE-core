//! Mechanized fixpoint resolution for the DFP→RFP→ref-verify→commit loop.
//!
//! The [`FixpointResolveInteractor`] implements [`FixpointResolveService`] by querying
//! three gate ports in priority order (dry → review → ref-verify) and returning the
//! single [`domain::track_phase::FixpointStep`] the orchestrator must execute next.
//!
//! [`FixpointDryGateInteractor`] implements [`FixpointDryGateService`], the D4
//! extraction that encapsulates the dry-gate enabled/disabled branching,
//! corpus-root manifest branching, and `DryCheckApprovalInteractor` construction
//! previously embedded in `cli_composition::track::fixpoint_resolve`.
//!
//! Design decisions: D2 / D4 / IN-02 / AC-03 / AC-04 / CN-02 / CN-07.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use domain::CommitHash;
use domain::TrackId;
use domain::dry_check::{
    DryCheckApprovalVerdict, DryCheckConfigFingerprint, DryCheckCorpusFingerprint, FragmentRef,
};
use domain::track_phase::{FixpointStep, ReviewScopeSet};
use thiserror::Error;

use crate::d4_orchestration::D4OrchestrationError;
use crate::dry_check::{
    DryCheckApprovalService, DryCheckConfig, DryFragmentPipelineCommand, DryFragmentPipelineService,
};

// ── FixpointCurrentBranch ─────────────────────────────────────────────────────

/// Validated opaque current git branch label for fixpoint resolution.
///
/// `try_new` rejects empty or whitespace-only strings with
/// [`FixpointResolveError::InvalidCurrentBranch`]; branch naming remains an
/// infrastructure/orchestrator concern so the usecase only validates non-emptiness.
///
/// # Errors
///
/// [`try_new`](FixpointCurrentBranch::try_new) returns
/// [`FixpointResolveError::InvalidCurrentBranch`] when `value` is empty or
/// consists solely of whitespace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FixpointCurrentBranch(String);

impl FixpointCurrentBranch {
    /// Construct a [`FixpointCurrentBranch`] from a non-empty, non-whitespace string.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::InvalidCurrentBranch`] when `value` is empty
    /// or whitespace-only.
    pub fn try_new(value: String) -> Result<Self, FixpointResolveError> {
        if value.trim().is_empty() {
            return Err(FixpointResolveError::InvalidCurrentBranch {
                message: "current branch must be non-empty and non-whitespace".to_owned(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the branch label as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

// ── FixpointResolveCommand ────────────────────────────────────────────────────

/// Command input for [`FixpointResolveService::resolve`].
///
/// Identifies the active track, the caller's current git branch, and the current
/// diff [`FragmentRef`] set needed by the dry gate public API.
///
/// `current_branch` is a validated opaque [`FixpointCurrentBranch`] value so empty
/// branch labels cannot enter the interactor.  The CLI composition layer (T015) is
/// responsible for asserting that `current_branch` matches the track's recorded
/// branch and returning [`FixpointResolveError::TrackNotActive`] before calling
/// [`FixpointResolveService::resolve`]; the interactor itself composes only the
/// three gate ports (dry / review / ref-verify) per CN-02.
///
/// `current_fragment_refs` is computed by the composition layer from the current diff
/// and passed through so the interactor does not acquire diff fragments itself (CN-02).
#[derive(Debug, Clone)]
pub struct FixpointResolveCommand {
    /// The track whose fixpoint is being resolved.
    pub track_id: domain::TrackId,
    /// Validated current git branch label.
    ///
    /// Validated by [`FixpointCurrentBranch::try_new`] to be non-empty and
    /// non-whitespace.  Branch-against-track consistency is enforced by the CLI
    /// composition layer before [`FixpointResolveService::resolve`] is called.
    pub current_branch: FixpointCurrentBranch,
    /// Current diff [`FragmentRef`] set computed by the composition layer.
    pub current_fragment_refs: BTreeSet<FragmentRef>,
}

// ── FixpointResolveError ──────────────────────────────────────────────────────

/// Error variants for [`FixpointResolveService::resolve`].
///
/// `GateQueryFailed` carries an opaque gate name string (e.g. `"dry"`, `"review"`,
/// `"ref_verify"`) and a message; both are labels used only for human-readable
/// diagnostics.
///
/// `TrackNotActive` is intended for the CLI composition layer (T015): the composition
/// must assert that [`FixpointResolveCommand::current_branch`] matches the track's
/// recorded branch before calling `resolve`, and return this variant when it does not.
/// The interactor itself does not perform this check because branch-against-track
/// consistency is an orchestration/infrastructure concern, not a gate-composition
/// concern (CN-02).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FixpointResolveError {
    /// The supplied track ID failed validation.
    #[error("invalid track ID: {message}")]
    InvalidTrackId {
        /// Human-readable validation failure detail.
        message: String,
    },
    /// The supplied current branch label is empty or whitespace-only.
    #[error("invalid current branch: {message}")]
    InvalidCurrentBranch {
        /// Human-readable validation failure detail.
        message: String,
    },
    /// The track's branch does not match the active branch.
    ///
    /// Returned by the CLI composition layer (T015) when it detects that the
    /// caller's `current_branch` does not match the track's recorded branch.
    /// Not produced by [`FixpointResolveInteractor::resolve`] itself.
    #[error("track is not active on branch '{branch}'")]
    TrackNotActive {
        /// The branch on which the track was expected to be active.
        branch: String,
    },
    /// One of the gate queries failed.
    #[error("gate '{gate}' query failed: {message}")]
    GateQueryFailed {
        /// Opaque gate name label (e.g. `"dry"`, `"review"`, `"ref_verify"`).
        gate: String,
        /// Human-readable error detail.
        message: String,
    },
}

// ── ReviewGateStatus ──────────────────────────────────────────────────────────

/// Review gate status consumed by the fixpoint resolver (D2).
///
/// - [`Approved`](ReviewGateStatus::Approved): no review scope is stale or has
///   findings remaining.
/// - [`NeedsReview`](ReviewGateStatus::NeedsReview): carries the non-empty
///   deterministic set of opaque review scope labels that must run through RFP.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewGateStatus {
    /// All review scopes are approved; no RFP is needed.
    Approved,
    /// One or more review scopes require RFP.
    NeedsReview {
        /// Non-empty set of scope labels that must run RFP.
        scopes: ReviewScopeSet,
    },
}

// ── RefVerifyGateStatus ───────────────────────────────────────────────────────

/// Ref-verify gate status consumed by the fixpoint resolver (D2).
///
/// - [`Approved`](RefVerifyGateStatus::Approved): ref-verify passed.
/// - [`Blocked`](RefVerifyGateStatus::Blocked): ref-verify must run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefVerifyGateStatus {
    /// Ref-verify gate is clear.
    Approved,
    /// Ref-verify must run before committing.
    Blocked,
}

// ── ReviewGateStatePort ───────────────────────────────────────────────────────

/// Secondary port used by [`FixpointResolveInteractor`] to query the review gate
/// through its public read API.
///
/// Returns an enum status rather than exposing `review.json` internals (CN-02).
///
/// # Errors
///
/// Implementations return [`FixpointResolveError::GateQueryFailed`] on any
/// underlying I/O or parse failure.
pub trait ReviewGateStatePort: Send + Sync {
    /// Query the current review gate status for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] on query failure.
    fn review_status(&self, track_id: &TrackId) -> Result<ReviewGateStatus, FixpointResolveError>;
}

// ── RefVerifyGateStatePort ────────────────────────────────────────────────────

/// Secondary port used by [`FixpointResolveInteractor`] to query the ref-verify
/// gate through its public read API.
///
/// Returns only gate status and never exposes ref-verify cache internals (CN-02).
///
/// # Errors
///
/// Implementations return [`FixpointResolveError::GateQueryFailed`] on any
/// underlying I/O or parse failure.
pub trait RefVerifyGateStatePort: Send + Sync {
    /// Query the current ref-verify gate status for `track_id`.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] on query failure.
    fn ref_verify_status(
        &self,
        track_id: &TrackId,
    ) -> Result<RefVerifyGateStatus, FixpointResolveError>;
}

// ── FixpointResolveService ────────────────────────────────────────────────────

/// Application service for mechanized fixpoint resolution (D2).
///
/// Queries the three gate states (dry / review / ref-verify) via their public APIs
/// and returns the next required phase as a [`domain::track_phase::FixpointStep`].
///
/// CN-02: no direct access to internal gate file formats.
///
/// # Errors
///
/// Returns [`FixpointResolveError`] on validation failures or gate query errors.
pub trait FixpointResolveService: Send + Sync {
    /// Determine the next convergence step for the given command.
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError`] if gate queries fail or command input is
    /// invalid.
    fn resolve(&self, cmd: &FixpointResolveCommand) -> Result<FixpointStep, FixpointResolveError>;
}

// ── FixpointResolveInteractor ─────────────────────────────────────────────────

/// Implements [`FixpointResolveService`].
///
/// Holds a [`DryCheckConfig`] (gate-enable flag) and injected port dependencies
/// (dry approval port, review state port, ref-verify result port) as private
/// `Arc<dyn Port>` fields.
/// Queries each gate's public API only — no direct access to `dry-check.json`,
/// `review.json`, or ref-verify cache internals (CN-02).
///
/// When `dry_config.enabled` is `false`, the dry gate call in `resolve` is
/// skipped entirely and treated as `Approved` (D2 / IN-05 / CN-06).
///
/// The branch-against-track consistency check (`TrackNotActive`) is the
/// responsibility of the CLI composition layer (T015) and is performed before
/// this interactor is called.
pub struct FixpointResolveInteractor {
    dry_config: DryCheckConfig,
    dry_approval: Arc<dyn DryCheckApprovalService + Send + Sync>,
    review_state: Arc<dyn ReviewGateStatePort>,
    ref_verify_results: Arc<dyn RefVerifyGateStatePort>,
}

impl FixpointResolveInteractor {
    /// Create a new [`FixpointResolveInteractor`].
    ///
    /// # Parameters
    ///
    /// - `dry_config`: usecase config including the `enabled` gate flag. When
    ///   `enabled` is `false`, `resolve` skips the dry gate and proceeds directly
    ///   to the review and ref-verify gate evaluation (D2 / IN-05 / CN-06).
    /// - `dry_approval`: pure-read dry gate via [`DryCheckApprovalService::check_approved`].
    /// - `review_state`: review gate status via [`ReviewGateStatePort::review_status`].
    /// - `ref_verify_results`: ref-verify gate status via
    ///   [`RefVerifyGateStatePort::ref_verify_status`].
    #[must_use]
    pub fn new(
        dry_config: DryCheckConfig,
        dry_approval: Arc<dyn DryCheckApprovalService + Send + Sync>,
        review_state: Arc<dyn ReviewGateStatePort>,
        ref_verify_results: Arc<dyn RefVerifyGateStatePort>,
    ) -> Self {
        Self { dry_config, dry_approval, review_state, ref_verify_results }
    }
}

impl FixpointResolveService for FixpointResolveInteractor {
    /// Evaluate all gate states in priority order and return the next required step.
    ///
    /// Algorithm (CN-02 — public API composition only):
    ///
    /// 1. Query dry gate via `dry_approval.check_approved(track_id, current_fragment_refs)`.
    /// 2. If [`DryCheckApprovalVerdict::Blocked`] → return [`FixpointStep::RunDfp`].
    /// 3. If `Approved` → query review gate via `review_state.review_status(track_id)`.
    /// 4. If [`ReviewGateStatus::NeedsReview { scopes }`] → return
    ///    [`FixpointStep::RunRfp { scopes }`].
    /// 5. If `Approved` → query ref-verify via `ref_verify_results.ref_verify_status(track_id)`.
    /// 6. If [`RefVerifyGateStatus::Blocked`] → return [`FixpointStep::RunRefVerify`].
    /// 7. If `Approved` → return [`FixpointStep::Commit`].
    ///
    /// Note: `cmd.current_branch` is validated by [`FixpointCurrentBranch::try_new`]
    /// before this method is called; branch-against-track consistency is enforced by
    /// the CLI composition layer (T015) via [`FixpointResolveError::TrackNotActive`].
    ///
    /// # Errors
    ///
    /// Returns [`FixpointResolveError::GateQueryFailed`] when any gate port returns
    /// an error, carrying the gate name (`"dry"`, `"review"`, or `"ref_verify"`) and
    /// the error detail.
    fn resolve(&self, cmd: &FixpointResolveCommand) -> Result<FixpointStep, FixpointResolveError> {
        // ── Step 1–2: Dry gate. ───────────────────────────────────────────────
        //
        // When `dry_config.enabled` is `false`, skip the dry gate entirely and
        // treat it as Approved (D2 / IN-05 / CN-06). The `dry_approval` port is
        // not called; this is the interactor-side bypass that complements the
        // CLI composition early-return (T006).
        if self.dry_config.enabled {
            let dry_verdict = self
                .dry_approval
                .check_approved(&cmd.track_id, &cmd.current_fragment_refs)
                .map_err(|e| FixpointResolveError::GateQueryFailed {
                    gate: "dry".to_owned(),
                    message: e.to_string(),
                })?;

            if matches!(dry_verdict, DryCheckApprovalVerdict::Blocked { .. }) {
                return Ok(FixpointStep::RunDfp);
            }
        }

        // ── Step 3–4: Review gate. ────────────────────────────────────────────
        let review_status = self.review_state.review_status(&cmd.track_id).map_err(|e| {
            FixpointResolveError::GateQueryFailed {
                gate: "review".to_owned(),
                message: e.to_string(),
            }
        })?;

        if let ReviewGateStatus::NeedsReview { scopes } = review_status {
            return Ok(FixpointStep::RunRfp { scopes });
        }

        // ── Step 5–6: Ref-verify gate. ────────────────────────────────────────
        let ref_verify_status =
            self.ref_verify_results.ref_verify_status(&cmd.track_id).map_err(|e| {
                FixpointResolveError::GateQueryFailed {
                    gate: "ref_verify".to_owned(),
                    message: e.to_string(),
                }
            })?;

        if matches!(ref_verify_status, RefVerifyGateStatus::Blocked) {
            return Ok(FixpointStep::RunRefVerify);
        }

        // ── Step 7: All gates green. ──────────────────────────────────────────
        Ok(FixpointStep::Commit)
    }
}

// ── DiffBaseResolverPort ──────────────────────────────────────────────────────

/// Error returned by [`DiffBaseResolverPort::resolve_diff_base`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DiffBaseResolverError {
    /// `.commit_hash` is corrupt and the `git rev-parse main` fallback also failed.
    #[error("{0}")]
    Unavailable(String),
}

/// Secondary port for resolving the dry-check diff-base commit for a track.
///
/// Abstracts the filesystem + git logic that reads the per-track `.commit_hash`
/// file with a fallback to `git rev-parse main`, keeping the usecase interactor
/// free of `std::fs` and `std::env`.
pub trait DiffBaseResolverPort: Send + Sync {
    /// Resolve the diff-base commit for the given track directory.
    ///
    /// # Errors
    ///
    /// Returns [`DiffBaseResolverError`] when `.commit_hash` is corrupt and
    /// the `git rev-parse main` fallback also fails.
    fn resolve_diff_base(
        &self,
        track_dir: &std::path::Path,
        canonical_root: &std::path::Path,
        repo_root: &std::path::Path,
    ) -> Result<CommitHash, DiffBaseResolverError>;
}

// ── DryCorpusMetaPort ─────────────────────────────────────────────────────────

/// Error returned by [`DryCorpusMetaPort::resolve_corpus_meta`].
#[derive(Debug, Clone, thiserror::Error)]
pub enum DryCorpusMetaError {
    /// The manifest exists but its workspace root cannot be resolved.
    #[error("{0}")]
    Unavailable(String),
}

/// Secondary port for resolving the corpus workspace root and current corpus
/// fingerprint given the track's `dry-check-corpus-root.json` manifest state.
///
/// Abstracts `std::fs::symlink_metadata` (corpus-root manifest existence check)
/// and the corpus fingerprint computation, keeping the usecase interactor free
/// of `std::fs`.
pub trait DryCorpusMetaPort: Send + Sync {
    /// Resolve `(approval_workspace_root, current_corpus_fingerprint)` for
    /// the given track directory.
    ///
    /// Implementations check whether `<track_dir>/dry-check-corpus-root.json`
    /// exists:
    /// - Missing → use `canonical_root` as workspace root; compute fingerprint
    ///   from `canonical_root`.
    /// - Present → resolve the recorded workspace root; compute fingerprint
    ///   from `repo_root`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCorpusMetaError`] when the manifest exists but its
    /// workspace root cannot be resolved.
    fn resolve_corpus_meta(
        &self,
        track_dir: &std::path::Path,
        canonical_root: &std::path::Path,
        repo_root: &std::path::Path,
    ) -> Result<(PathBuf, DryCheckCorpusFingerprint), DryCorpusMetaError>;
}

// ── DryApprovalFactoryPort ────────────────────────────────────────────────────

/// Secondary port that constructs a `DryCheckApprovalService` from resolved
/// corpus metadata and configuration, keeping infrastructure adapter
/// construction (`FsDryCheckStore`, `FsDryCheckCoverageAdapter`) out of the
/// usecase interactor (CN-02 / CN-07).
pub trait DryApprovalFactoryPort: Send + Sync {
    /// Build a `DryCheckApprovalService` for the given track.
    ///
    /// Parameters mirror the inputs to `DryCheckApprovalInteractor::new`:
    /// - `track_dir`: per-track items directory (e.g. `track/items/<id>`).
    /// - `canonical_root`: project root used for store paths.
    /// - `dry_config`: usecase dry-check configuration.
    /// - `config_fingerprint`: SHA-256 fingerprint of the current `dry-check.json`.
    /// - `corpus_fingerprint`: SHA-256 fingerprint of the current corpus.
    fn build_approval(
        &self,
        track_dir: &std::path::Path,
        canonical_root: &std::path::Path,
        dry_config: DryCheckConfig,
        config_fingerprint: DryCheckConfigFingerprint,
        corpus_fingerprint: DryCheckCorpusFingerprint,
    ) -> Arc<dyn DryCheckApprovalService + Send + Sync>;
}

// ── FixpointDryGateCommand ────────────────────────────────────────────────────

/// CQRS command for the D4 fixpoint-resolve dry-gate orchestration.
///
/// Carries the resolved path trio (track directory, canonical project root,
/// git repository root) and the validated usecase dry-check configuration.
/// The CLI composition layer resolves all four from its validated inputs before
/// calling [`FixpointDryGateService::resolve_dry_gate`].
#[derive(Debug, Clone)]
pub struct FixpointDryGateCommand {
    /// Per-track items directory (e.g. `track/items/<id>`).
    pub track_dir: PathBuf,
    /// Canonical project root (parent of `track/`).
    pub canonical_root: PathBuf,
    /// Git repository root (CWD-discovered, trust anchor for all git ops).
    pub repo_root: PathBuf,
    /// Validated usecase dry-check configuration including the `enabled` flag.
    pub dry_config: DryCheckConfig,
    /// SHA-256 fingerprint of the current `.harness/config/dry-check.json`.
    pub current_config_fingerprint: DryCheckConfigFingerprint,
}

// ── FixpointDryGateOutput ─────────────────────────────────────────────────────

/// Output DTO for the D4 dry-gate orchestration.
///
/// When `dry_config.enabled` is `false` the gate is bypassed and
/// `current_fragment_refs` will be empty; `dry_approval` returns `Approved`
/// unconditionally (via `NoOpDryApprovalService`).
pub struct FixpointDryGateOutput {
    /// Current diff fragment refs computed by the pipeline (empty when gate is
    /// disabled).
    pub current_fragment_refs: BTreeSet<FragmentRef>,
    /// Arc-wrapped approval service ready for injection into
    /// [`FixpointResolveInteractor`].
    pub dry_approval: Arc<dyn DryCheckApprovalService + Send + Sync>,
    /// Workspace root used by the approval service for corpus-root fingerprint
    /// comparisons.
    pub approval_workspace_root: PathBuf,
}

// ── FixpointDryGateService ────────────────────────────────────────────────────

/// Application service (primary port) for D4 fixpoint-resolve dry-gate
/// orchestration.
///
/// Owns the enabled/disabled gate branching and the multi-step setup
/// (diff-base resolution → corpus meta resolution → fragment pipeline →
/// approval service construction) previously embedded in
/// `cli_composition::track::fixpoint_resolve`.
///
/// # Errors
///
/// Returns [`D4OrchestrationError::DryGate`] on diff-base, corpus-meta, or
/// fragment-pipeline failures.
pub trait FixpointDryGateService: Send + Sync {
    /// Resolve the dry gate for the given command.
    ///
    /// When `cmd.dry_config.enabled` is `false` returns a disabled-gate output
    /// (empty fragment refs, no-op approval) immediately without touching any
    /// secondary ports.
    ///
    /// # Errors
    ///
    /// Returns [`D4OrchestrationError::DryGate`] on any port failure.
    fn resolve_dry_gate(
        &self,
        cmd: FixpointDryGateCommand,
    ) -> Result<FixpointDryGateOutput, D4OrchestrationError>;
}

// ── FixpointDryGateInteractor ─────────────────────────────────────────────────

/// Interactor implementing [`FixpointDryGateService`].
///
/// Holds injected secondary ports:
/// - `noop_approval`: no-op approval service returned when gate is disabled.
/// - `diff_base_resolver`: resolves the diff-base commit per track.
/// - `corpus_meta`: resolves corpus workspace root + fingerprint.
/// - `fragment_pipeline`: derives current fragment refs from the diff.
/// - `approval_factory`: constructs the `DryCheckApprovalInteractor` from
///   resolved corpus metadata.
///
/// CWD guard (needed by `GitDryCheckDiffGetter`) is managed by the
/// **caller** in `cli_composition`: the caller sets `CWD = repo_root`,
/// then calls `resolve_dry_gate`, then restores CWD. This keeps
/// `std::env::set_current_dir` out of the usecase layer (CN-07).
pub struct FixpointDryGateInteractor {
    noop_approval: Arc<dyn DryCheckApprovalService + Send + Sync>,
    diff_base_resolver: Arc<dyn DiffBaseResolverPort>,
    corpus_meta: Arc<dyn DryCorpusMetaPort>,
    fragment_pipeline: Arc<dyn DryFragmentPipelineService>,
    approval_factory: Arc<dyn DryApprovalFactoryPort>,
}

impl FixpointDryGateInteractor {
    /// Construct a new [`FixpointDryGateInteractor`] with all required ports.
    #[must_use]
    pub fn new(
        noop_approval: Arc<dyn DryCheckApprovalService + Send + Sync>,
        diff_base_resolver: Arc<dyn DiffBaseResolverPort>,
        corpus_meta: Arc<dyn DryCorpusMetaPort>,
        fragment_pipeline: Arc<dyn DryFragmentPipelineService>,
        approval_factory: Arc<dyn DryApprovalFactoryPort>,
    ) -> Self {
        Self { noop_approval, diff_base_resolver, corpus_meta, fragment_pipeline, approval_factory }
    }
}

impl FixpointDryGateService for FixpointDryGateInteractor {
    fn resolve_dry_gate(
        &self,
        cmd: FixpointDryGateCommand,
    ) -> Result<FixpointDryGateOutput, D4OrchestrationError> {
        let FixpointDryGateCommand {
            track_dir,
            canonical_root,
            repo_root,
            dry_config,
            current_config_fingerprint,
        } = cmd;

        if !dry_config.enabled {
            // Gate disabled: return empty fragment refs and no-op approval service.
            return Ok(FixpointDryGateOutput {
                current_fragment_refs: BTreeSet::new(),
                dry_approval: Arc::clone(&self.noop_approval),
                approval_workspace_root: canonical_root,
            });
        }

        // ── Resolve diff-base commit ──────────────────────────────────────────
        let base = self
            .diff_base_resolver
            .resolve_diff_base(&track_dir, &canonical_root, &repo_root)
            .map_err(|e| D4OrchestrationError::DryGate(format!("diff-base resolution: {e}")))?;

        // ── Resolve corpus workspace root + fingerprint ───────────────────────
        let (approval_workspace_root, current_corpus_fingerprint) = self
            .corpus_meta
            .resolve_corpus_meta(&track_dir, &canonical_root, &repo_root)
            .map_err(|e| D4OrchestrationError::DryGate(format!("corpus meta resolution: {e}")))?;

        // ── Build current fragment refs via the pipeline ──────────────────────
        // CWD contract: caller has already set CWD = repo_root before invoking
        // resolve_dry_gate; the fragment pipeline inherits that CWD for git
        // discovery via GitDryCheckDiffGetter.
        let pipeline_cmd = DryFragmentPipelineCommand {
            canonical_root: approval_workspace_root.clone(),
            repo_root: repo_root.clone(),
            base,
        };
        let pipeline_output = self
            .fragment_pipeline
            .derive_current_refs(pipeline_cmd)
            .map_err(|e| D4OrchestrationError::DryGate(format!("fragment pipeline: {e}")))?;

        let current_fragment_refs = pipeline_output.fragment_refs;

        // ── Construct the approval service ────────────────────────────────────
        let dry_approval = self.approval_factory.build_approval(
            &track_dir,
            &canonical_root,
            dry_config,
            current_config_fingerprint,
            current_corpus_fingerprint,
        );

        Ok(FixpointDryGateOutput { current_fragment_refs, dry_approval, approval_workspace_root })
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::collections::BTreeSet;

    use domain::TrackId;
    use domain::dry_check::DryCheckApprovalVerdict;
    use domain::track_phase::ReviewScopeSet;

    use crate::dry_check::{
        DryCheckConfig, DryCheckCycleError, DryCheckParallelism, DryCheckPercent,
    };

    use super::*;

    // ── Test doubles ──────────────────────────────────────────────────────────

    /// Stub dry approval that returns a fixed verdict or a simulated error.
    ///
    /// `fail` controls whether the port returns an error; when `false` the
    /// `verdict` field is returned. Using an enum flag avoids the need for
    /// `DryCheckCycleError: Clone`.
    struct StubDryApproval {
        verdict: DryCheckApprovalVerdict,
        /// When `true`, the port returns a `CoveragePort` error.
        fail: bool,
    }

    impl DryCheckApprovalService for StubDryApproval {
        fn check_approved(
            &self,
            _track_id: &TrackId,
            _current_fragment_refs: &BTreeSet<domain::dry_check::FragmentRef>,
        ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> {
            if self.fail {
                return Err(DryCheckCycleError::CoveragePort("simulated error".to_owned()));
            }
            Ok(self.verdict.clone())
        }
    }

    struct StubReviewGate {
        status: Result<ReviewGateStatus, FixpointResolveError>,
    }

    impl ReviewGateStatePort for StubReviewGate {
        fn review_status(
            &self,
            _track_id: &TrackId,
        ) -> Result<ReviewGateStatus, FixpointResolveError> {
            self.status.clone()
        }
    }

    struct StubRefVerifyGate {
        status: Result<RefVerifyGateStatus, FixpointResolveError>,
    }

    impl RefVerifyGateStatePort for StubRefVerifyGate {
        fn ref_verify_status(
            &self,
            _track_id: &TrackId,
        ) -> Result<RefVerifyGateStatus, FixpointResolveError> {
            self.status.clone()
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn make_track_id() -> TrackId {
        TrackId::try_new("test-track-2026").unwrap()
    }

    fn make_cmd() -> FixpointResolveCommand {
        FixpointResolveCommand {
            track_id: make_track_id(),
            current_branch: FixpointCurrentBranch::try_new("track/test".to_owned()).unwrap(),
            current_fragment_refs: BTreeSet::new(),
        }
    }

    /// Build a [`DryCheckConfig`] with the given `enabled` flag and reasonable defaults.
    fn test_dry_check_config(enabled: bool) -> DryCheckConfig {
        DryCheckConfig::new(
            DryCheckPercent::try_new(10).unwrap(),
            DryCheckPercent::try_new(90).unwrap(),
            DryCheckParallelism::try_new(4).unwrap(),
            enabled,
        )
    }

    fn make_interactor(
        dry_verdict: DryCheckApprovalVerdict,
        review: Result<ReviewGateStatus, FixpointResolveError>,
        ref_verify: Result<RefVerifyGateStatus, FixpointResolveError>,
    ) -> FixpointResolveInteractor {
        FixpointResolveInteractor::new(
            test_dry_check_config(true),
            Arc::new(StubDryApproval { verdict: dry_verdict, fail: false }),
            Arc::new(StubReviewGate { status: review }),
            Arc::new(StubRefVerifyGate { status: ref_verify }),
        )
    }

    fn make_interactor_with_dry_error(
        review: Result<ReviewGateStatus, FixpointResolveError>,
        ref_verify: Result<RefVerifyGateStatus, FixpointResolveError>,
    ) -> FixpointResolveInteractor {
        FixpointResolveInteractor::new(
            test_dry_check_config(true),
            Arc::new(StubDryApproval { verdict: DryCheckApprovalVerdict::Approved, fail: true }),
            Arc::new(StubReviewGate { status: review }),
            Arc::new(StubRefVerifyGate { status: ref_verify }),
        )
    }

    // ── FixpointCurrentBranch ─────────────────────────────────────────────────

    #[test]
    fn fixpoint_current_branch_try_new_with_empty_string_returns_invalid_current_branch() {
        let result = FixpointCurrentBranch::try_new(String::new());
        assert!(matches!(result, Err(FixpointResolveError::InvalidCurrentBranch { .. })));
    }

    #[test]
    fn fixpoint_current_branch_try_new_with_whitespace_only_returns_invalid_current_branch() {
        let result = FixpointCurrentBranch::try_new("   ".to_owned());
        assert!(matches!(result, Err(FixpointResolveError::InvalidCurrentBranch { .. })));
    }

    #[test]
    fn fixpoint_current_branch_try_new_with_valid_label_succeeds_and_as_str_round_trips() {
        let result = FixpointCurrentBranch::try_new("track/foo".to_owned());
        assert!(result.is_ok());
        assert_eq!(result.unwrap().as_str(), "track/foo");
    }

    // ── FixpointResolveInteractor — gate priority sequence ────────────────────

    #[test]
    fn interactor_resolve_dry_blocked_returns_run_dfp() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 2 },
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Approved),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::RunDfp);
    }

    #[test]
    fn interactor_resolve_dry_approved_review_needs_review_returns_run_rfp_with_scopes() {
        let mut scope_set = BTreeSet::new();
        scope_set.insert("impl-plan".to_owned());
        scope_set.insert("code".to_owned());
        let scopes = ReviewScopeSet::try_new(scope_set).unwrap();

        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::NeedsReview { scopes: scopes.clone() }),
            Ok(RefVerifyGateStatus::Approved),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::RunRfp { scopes });
    }

    #[test]
    fn interactor_resolve_dry_approved_review_approved_ref_verify_blocked_returns_run_ref_verify() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Blocked),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::RunRefVerify);
    }

    #[test]
    fn interactor_resolve_all_gates_approved_returns_commit() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Approved),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(step, FixpointStep::Commit);
    }

    // ── FixpointResolveInteractor — gate error propagation ────────────────────

    #[test]
    fn interactor_resolve_dry_gate_error_returns_gate_query_failed_for_dry() {
        let interactor = make_interactor_with_dry_error(
            Ok(ReviewGateStatus::Approved),
            Ok(RefVerifyGateStatus::Approved),
        );
        let err = interactor.resolve(&make_cmd()).unwrap_err();
        assert!(
            matches!(&err, FixpointResolveError::GateQueryFailed { gate, .. } if gate == "dry"),
            "expected GateQueryFailed for 'dry' but got {err:?}"
        );
    }

    #[test]
    fn interactor_resolve_review_gate_error_returns_gate_query_failed_for_review() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Err(FixpointResolveError::GateQueryFailed {
                gate: "review".to_owned(),
                message: "simulated error".to_owned(),
            }),
            Ok(RefVerifyGateStatus::Approved),
        );
        let err = interactor.resolve(&make_cmd()).unwrap_err();
        assert!(
            matches!(&err, FixpointResolveError::GateQueryFailed { gate, .. } if gate == "review"),
            "expected GateQueryFailed for 'review' but got {err:?}"
        );
    }

    #[test]
    fn interactor_resolve_ref_verify_gate_error_returns_gate_query_failed_for_ref_verify() {
        let interactor = make_interactor(
            DryCheckApprovalVerdict::Approved,
            Ok(ReviewGateStatus::Approved),
            Err(FixpointResolveError::GateQueryFailed {
                gate: "ref_verify".to_owned(),
                message: "simulated error".to_owned(),
            }),
        );
        let err = interactor.resolve(&make_cmd()).unwrap_err();
        assert!(
            matches!(&err, FixpointResolveError::GateQueryFailed { gate, .. } if gate == "ref_verify"),
            "expected GateQueryFailed for 'ref_verify' but got {err:?}"
        );
    }

    // ── T011: enabled=false dry gate bypass ───────────────────────────────────

    /// When `dry_config.enabled` is `false` and the dry stub would return `Blocked`,
    /// the dry gate must be bypassed entirely and the step must reflect the review
    /// and ref-verify gate states instead.
    ///
    /// Sub-case: review Approved, ref-verify Approved → `Commit`.
    #[test]
    fn interactor_resolve_dry_disabled_with_blocked_dry_returns_commit_when_all_other_gates_green()
    {
        // dry stub would return Blocked, but enabled=false means it is never called.
        let interactor = FixpointResolveInteractor::new(
            test_dry_check_config(false),
            Arc::new(StubDryApproval {
                verdict: DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 5 },
                fail: false,
            }),
            Arc::new(StubReviewGate { status: Ok(ReviewGateStatus::Approved) }),
            Arc::new(StubRefVerifyGate { status: Ok(RefVerifyGateStatus::Approved) }),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(
            step,
            FixpointStep::Commit,
            "enabled=false must bypass dry gate; all other gates green → Commit"
        );
    }

    /// When `dry_config.enabled` is `false` and review needs review, the step must
    /// be `RunRfp` (dry gate bypassed, review gate takes over).
    #[test]
    fn interactor_resolve_dry_disabled_with_blocked_dry_returns_run_rfp_when_review_needs_review() {
        let mut scope_set = BTreeSet::new();
        scope_set.insert("code".to_owned());
        let scopes = ReviewScopeSet::try_new(scope_set).unwrap();

        let interactor = FixpointResolveInteractor::new(
            test_dry_check_config(false),
            Arc::new(StubDryApproval {
                verdict: DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 3 },
                fail: false,
            }),
            Arc::new(StubReviewGate {
                status: Ok(ReviewGateStatus::NeedsReview { scopes: scopes.clone() }),
            }),
            Arc::new(StubRefVerifyGate { status: Ok(RefVerifyGateStatus::Approved) }),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(
            step,
            FixpointStep::RunRfp { scopes },
            "enabled=false must bypass dry gate; review NeedsReview → RunRfp"
        );
    }

    /// When `dry_config.enabled` is `false`, review Approved, and ref-verify Blocked,
    /// the step must be `RunRefVerify`.
    #[test]
    fn interactor_resolve_dry_disabled_returns_run_ref_verify_when_ref_verify_blocked() {
        let interactor = FixpointResolveInteractor::new(
            test_dry_check_config(false),
            Arc::new(StubDryApproval {
                verdict: DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 1 },
                fail: false,
            }),
            Arc::new(StubReviewGate { status: Ok(ReviewGateStatus::Approved) }),
            Arc::new(StubRefVerifyGate { status: Ok(RefVerifyGateStatus::Blocked) }),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(
            step,
            FixpointStep::RunRefVerify,
            "enabled=false must bypass dry gate; ref-verify Blocked → RunRefVerify"
        );
    }

    /// When `dry_config.enabled` is `true` and the dry stub returns `Blocked`,
    /// the step must be `RunDfp` (unchanged behavior).
    #[test]
    fn interactor_resolve_dry_enabled_with_blocked_dry_returns_run_dfp() {
        let interactor = FixpointResolveInteractor::new(
            test_dry_check_config(true),
            Arc::new(StubDryApproval {
                verdict: DryCheckApprovalVerdict::Blocked { unresolved_pair_count: 2 },
                fail: false,
            }),
            Arc::new(StubReviewGate { status: Ok(ReviewGateStatus::Approved) }),
            Arc::new(StubRefVerifyGate { status: Ok(RefVerifyGateStatus::Approved) }),
        );
        let step = interactor.resolve(&make_cmd()).unwrap();
        assert_eq!(
            step,
            FixpointStep::RunDfp,
            "enabled=true with dry Blocked must return RunDfp (unchanged behavior)"
        );
    }

    // ── T008: FixpointDryGateInteractor tests ─────────────────────────────────

    use std::path::PathBuf;

    use domain::dry_check::{DryCheckConfigFingerprint, DryCheckCorpusFingerprint};

    use crate::dry_check::DryFragmentPipelineOutput;
    use crate::fixpoint_resolve::{
        DiffBaseResolverPort, DryApprovalFactoryPort, DryCorpusMetaPort, FixpointDryGateCommand,
        FixpointDryGateInteractor,
    };

    // ── Test doubles for FixpointDryGate ─────────────────────────────────────

    struct StubDiffBaseResolver {
        result: Result<domain::CommitHash, DiffBaseResolverError>,
    }

    impl DiffBaseResolverPort for StubDiffBaseResolver {
        fn resolve_diff_base(
            &self,
            _track_dir: &std::path::Path,
            _canonical_root: &std::path::Path,
            _repo_root: &std::path::Path,
        ) -> Result<domain::CommitHash, DiffBaseResolverError> {
            self.result.clone()
        }
    }

    struct StubCorpusMeta {
        result: Result<(PathBuf, DryCheckCorpusFingerprint), DryCorpusMetaError>,
    }

    impl DryCorpusMetaPort for StubCorpusMeta {
        fn resolve_corpus_meta(
            &self,
            _track_dir: &std::path::Path,
            _canonical_root: &std::path::Path,
            _repo_root: &std::path::Path,
        ) -> Result<(PathBuf, DryCheckCorpusFingerprint), DryCorpusMetaError> {
            self.result.clone()
        }
    }

    struct StubFragmentPipeline {
        result: Result<DryFragmentPipelineOutput, crate::d4_orchestration::D4OrchestrationError>,
    }

    impl crate::dry_check::fragment_pipeline::DryFragmentPipelineService for StubFragmentPipeline {
        fn derive_current_refs(
            &self,
            _cmd: DryFragmentPipelineCommand,
        ) -> Result<DryFragmentPipelineOutput, crate::d4_orchestration::D4OrchestrationError>
        {
            self.result.clone()
        }
    }

    struct StubApprovalFactory {
        verdict: domain::dry_check::DryCheckApprovalVerdict,
    }

    struct StubApprovalServiceForFactory {
        verdict: domain::dry_check::DryCheckApprovalVerdict,
    }

    impl DryCheckApprovalService for StubApprovalServiceForFactory {
        fn check_approved(
            &self,
            _track_id: &TrackId,
            _current_fragment_refs: &BTreeSet<domain::dry_check::FragmentRef>,
        ) -> Result<domain::dry_check::DryCheckApprovalVerdict, DryCheckCycleError> {
            Ok(self.verdict.clone())
        }
    }

    impl DryApprovalFactoryPort for StubApprovalFactory {
        fn build_approval(
            &self,
            _track_dir: &std::path::Path,
            _canonical_root: &std::path::Path,
            _dry_config: DryCheckConfig,
            _config_fingerprint: DryCheckConfigFingerprint,
            _corpus_fingerprint: DryCheckCorpusFingerprint,
        ) -> Arc<dyn DryCheckApprovalService + Send + Sync> {
            Arc::new(StubApprovalServiceForFactory { verdict: self.verdict.clone() })
        }
    }

    fn make_commit_hash() -> domain::CommitHash {
        domain::CommitHash::try_new("a".repeat(40)).unwrap()
    }

    fn make_corpus_fingerprint() -> DryCheckCorpusFingerprint {
        DryCheckCorpusFingerprint::new("b".repeat(64)).unwrap()
    }

    fn make_config_fingerprint() -> DryCheckConfigFingerprint {
        DryCheckConfigFingerprint::new("c".repeat(64)).unwrap()
    }

    fn make_dry_gate_cmd(enabled: bool) -> FixpointDryGateCommand {
        FixpointDryGateCommand {
            track_dir: PathBuf::from("/track/items/test-track"),
            canonical_root: PathBuf::from("/project"),
            repo_root: PathBuf::from("/repo"),
            dry_config: test_dry_check_config(enabled),
            current_config_fingerprint: make_config_fingerprint(),
        }
    }

    fn make_noop_approval() -> Arc<dyn DryCheckApprovalService + Send + Sync> {
        Arc::new(StubApprovalServiceForFactory {
            verdict: domain::dry_check::DryCheckApprovalVerdict::Approved,
        })
    }

    fn make_gate_interactor(
        diff_base: Result<domain::CommitHash, DiffBaseResolverError>,
        corpus_meta: Result<(PathBuf, DryCheckCorpusFingerprint), DryCorpusMetaError>,
        pipeline_result: Result<
            DryFragmentPipelineOutput,
            crate::d4_orchestration::D4OrchestrationError,
        >,
        factory_verdict: domain::dry_check::DryCheckApprovalVerdict,
    ) -> FixpointDryGateInteractor {
        FixpointDryGateInteractor::new(
            make_noop_approval(),
            Arc::new(StubDiffBaseResolver { result: diff_base }),
            Arc::new(StubCorpusMeta { result: corpus_meta }),
            Arc::new(StubFragmentPipeline { result: pipeline_result }),
            Arc::new(StubApprovalFactory { verdict: factory_verdict }),
        )
    }

    /// When gate is disabled, resolve_dry_gate returns empty refs and no-op approval.
    #[test]
    fn fixpoint_dry_gate_disabled_returns_empty_refs_and_noop_approval() {
        let interactor = make_gate_interactor(
            Ok(make_commit_hash()),
            Ok((PathBuf::from("/project"), make_corpus_fingerprint())),
            Ok(DryFragmentPipelineOutput {
                fragment_refs: BTreeSet::new(),
                records_before: 0,
                records_after: 0,
                records_appended: 0,
            }),
            domain::dry_check::DryCheckApprovalVerdict::Approved,
        );
        let cmd = make_dry_gate_cmd(false);
        let output = interactor.resolve_dry_gate(cmd).unwrap();
        assert!(
            output.current_fragment_refs.is_empty(),
            "disabled gate must return empty fragment refs"
        );
        // noop approval returns Approved unconditionally
        let verdict =
            output.dry_approval.check_approved(&make_track_id(), &BTreeSet::new()).unwrap();
        assert!(
            matches!(verdict, domain::dry_check::DryCheckApprovalVerdict::Approved),
            "disabled gate noop must return Approved"
        );
    }

    /// When gate is enabled and pipeline succeeds, fragment refs are returned.
    #[test]
    fn fixpoint_dry_gate_enabled_with_successful_pipeline_returns_refs() {
        let mut refs = BTreeSet::new();
        // Insert a dummy FragmentRef via domain API
        let frag = domain::semantic_dup::CodeFragment::new(
            PathBuf::from("src/a.rs"),
            "fn a() {}".to_owned(),
            1,
            5,
        )
        .unwrap();
        let fref = crate::dry_check::fragment_ref_of(&frag).unwrap();
        refs.insert(fref);

        let interactor = make_gate_interactor(
            Ok(make_commit_hash()),
            Ok((PathBuf::from("/project"), make_corpus_fingerprint())),
            Ok(DryFragmentPipelineOutput {
                fragment_refs: refs.clone(),
                records_before: 0,
                records_after: 0,
                records_appended: 0,
            }),
            domain::dry_check::DryCheckApprovalVerdict::Approved,
        );
        let cmd = make_dry_gate_cmd(true);
        let output = interactor.resolve_dry_gate(cmd).unwrap();
        assert_eq!(
            output.current_fragment_refs.len(),
            1,
            "enabled gate must return pipeline's fragment refs"
        );
    }

    /// When diff-base resolution fails, resolve_dry_gate returns DryGate error.
    #[test]
    fn fixpoint_dry_gate_diff_base_failure_returns_dry_gate_error() {
        let interactor = make_gate_interactor(
            Err(DiffBaseResolverError::Unavailable("git rev-parse main failed".to_owned())),
            Ok((PathBuf::from("/project"), make_corpus_fingerprint())),
            Ok(DryFragmentPipelineOutput {
                fragment_refs: BTreeSet::new(),
                records_before: 0,
                records_after: 0,
                records_appended: 0,
            }),
            domain::dry_check::DryCheckApprovalVerdict::Approved,
        );
        let cmd = make_dry_gate_cmd(true);
        // Use map(|_| ()) to drop the non-Debug FixpointDryGateOutput before calling
        // unwrap_err(): Result<(), D4OrchestrationError> can use unwrap_err directly.
        let err = interactor.resolve_dry_gate(cmd).map(|_| ()).unwrap_err();
        assert!(
            matches!(err, crate::d4_orchestration::D4OrchestrationError::DryGate(_)),
            "diff-base failure must map to DryGate error, got {err:?}"
        );
    }

    /// When corpus meta resolution fails, resolve_dry_gate returns DryGate error.
    #[test]
    fn fixpoint_dry_gate_corpus_meta_failure_returns_dry_gate_error() {
        let interactor = make_gate_interactor(
            Ok(make_commit_hash()),
            Err(DryCorpusMetaError::Unavailable("corpus root manifest not found".to_owned())),
            Ok(DryFragmentPipelineOutput {
                fragment_refs: BTreeSet::new(),
                records_before: 0,
                records_after: 0,
                records_appended: 0,
            }),
            domain::dry_check::DryCheckApprovalVerdict::Approved,
        );
        let cmd = make_dry_gate_cmd(true);
        let err = interactor.resolve_dry_gate(cmd).map(|_| ()).unwrap_err();
        assert!(
            matches!(err, crate::d4_orchestration::D4OrchestrationError::DryGate(_)),
            "corpus meta failure must map to DryGate error, got {err:?}"
        );
    }

    /// When fragment-ref derivation fails, resolve_dry_gate returns DryGate error.
    #[test]
    fn fixpoint_dry_gate_fragment_pipeline_failure_returns_dry_gate_error() {
        let interactor = make_gate_interactor(
            Ok(make_commit_hash()),
            Ok((PathBuf::from("/project"), make_corpus_fingerprint())),
            Err(crate::d4_orchestration::D4OrchestrationError::DiffFragment(
                "fragment extraction failed".to_owned(),
            )),
            domain::dry_check::DryCheckApprovalVerdict::Approved,
        );
        let cmd = make_dry_gate_cmd(true);
        let err = interactor.resolve_dry_gate(cmd).map(|_| ()).unwrap_err();
        assert!(
            matches!(err, crate::d4_orchestration::D4OrchestrationError::DryGate(_)),
            "fragment pipeline failure must map to DryGate error, got {err:?}"
        );
    }
}
