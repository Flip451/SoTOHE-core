//! Secondary port traits for the dry-check use case layer.

use domain::CommitHash;
use domain::TrackId;
use domain::dry_check::{DiffFileHunks, DryCheckCoverageRecord};
use domain::semantic_dup::CodeFragment;

use super::errors::{DryCheckAgentError, DryCheckCycleError, DryCheckDiffError};
use super::judgment::DryCheckAgentJudgment;

// ── DryCheckJudgeTier ────────────────────────────────────────────────────────

/// Tier selector for the 2-stage DRY-check calibration barrier (D4 / T012).
///
/// `Fast` uses the lower-cost fast model; `Final` uses the higher-accuracy
/// final model. The interactor selects the tier per-call during the judgment phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DryCheckJudgeTier {
    /// Fast tier: low-cost model for the initial screening pass.
    Fast,
    /// Final tier: high-accuracy model for escalated or all-final runs.
    Final,
}

// ── DryCheckAgentPort ─────────────────────────────────────────────────────────

/// Usecase port for the dry-checker agent capability.
///
/// The agent reads both code fragments, applies precision judgment (rejects
/// self-match, acceptable similarity, out-of-scope dup), and returns a
/// [`DryCheckAgentJudgment`]. Every variant of the judgment carries a required
/// rationale (`domain::dry_check::Rationale` — validated non-empty newtype).
/// A `Violation` judgment additionally carries a `DryCheckFinding` with
/// `RefactorProposal` and the changed/candidate `FragmentRef`s
/// (IN-03/AC-03).
///
/// The interactor converts the judgment to `DryCheckVerdict` for persistence:
/// `Violation` maps to `DryCheckVerdict::Violation { refactor_proposal }`
/// (enum-first D9), extracting `finding.refactor_proposal` (already a
/// `RefactorProposal`); `NotAViolation`/`Accepted` map to their respective
/// unit variants. `Rationale` is extracted for `DryCheckEntry.rationale`.
///
/// The interactor computes `FragmentContentHash` from each
/// `CodeFragment.content()` to build the `FragmentRef`s for `DryCheckPairKey`
/// — the agent receives `CodeFragment` directly.
///
/// Analogous to `Reviewer`. Placed in usecase because agent invocation is an
/// infrastructure capability with no domain entity semantics.
pub trait DryCheckAgentPort: Send + Sync {
    /// Judge whether `changed_fragment` duplicates `candidate_fragment`.
    ///
    /// `tier` selects the model/reasoning-effort combination:
    /// - `DryCheckJudgeTier::Fast`: fast/cheap model for initial screening.
    /// - `DryCheckJudgeTier::Final`: accurate model for escalation or all-final runs.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckAgentError`] on agent abort, timeout, illegal output,
    /// or unexpected failure.
    fn judge(
        &self,
        changed_fragment: &CodeFragment,
        candidate_fragment: &CodeFragment,
        tier: DryCheckJudgeTier,
    ) -> Result<DryCheckAgentJudgment, DryCheckAgentError>;
}

// ── DryCheckDiffSource ────────────────────────────────────────────────────────

/// Dry-check's own secondary port for retrieving changed file paths with their
/// added/changed hunk line ranges, relative to a base commit.
///
/// CN-01: this is an independent interface for the DRY gate — it is NOT
/// `review_v2`'s `DiffGetter`. Returns `Vec<DiffFileHunks>` (each element
/// carries a `FilePath` and a non-empty `Vec<DiffHunkRange>`) instead of bare
/// `Vec<FilePath>`, enabling hunk-level overlap detection (D4 hunk-scope).
///
/// Only files with at least one added/changed hunk appear in the result
/// (deletion-only files and unmodified-but-staged files are excluded
/// structurally by `DiffFileHunks::new` rejecting empty hunk lists).
///
/// Behavior mirrors `GitDiffGetter`'s 4-source union (merge-base..HEAD +
/// staged + unstaged + untracked) but owned by dry-check so both gates evolve
/// independently. Implemented by `infrastructure::GitDryCheckDiffGetter`.
pub trait DryCheckDiffSource: Send + Sync {
    /// Return the changed file hunks relative to `base`.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckDiffError`] when the underlying git / I/O operation
    /// fails.
    fn list_changed_hunks(
        &self,
        base: &CommitHash,
        repo_root: &std::path::Path,
    ) -> Result<Vec<DiffFileHunks>, DryCheckDiffError>;
}

// ── DryCheckCoveragePort ──────────────────────────────────────────────────────

/// Secondary port for persisting and retrieving the [`DryCheckCoverageRecord`]
/// that backs the read-only `dry check-approved` staleness gate (D5).
///
/// `dry write` records the set of `FragmentRef`s it processed via
/// [`write_coverage`](DryCheckCoveragePort::write_coverage); `dry check-approved`
/// reads it via [`read_coverage`](DryCheckCoveragePort::read_coverage) and
/// compares each current diff fragment's `FragmentRef` against the recorded set.
///
/// CN-08: when no coverage manifest exists yet, `read_coverage` returns
/// `Ok(None)` — the calling interactor treats `None` as Blocked (fail-closed),
/// NOT as an error. Genuine I/O / serialization failures are surfaced as
/// [`DryCheckCycleError::CoveragePort`].
///
/// Implemented by `infrastructure::FsDryCheckCoverageAdapter`.
pub trait DryCheckCoveragePort: Send + Sync {
    /// Read the coverage record for `track_id`, or `Ok(None)` when no manifest
    /// has been written yet.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError::CoveragePort`] on I/O / deserialization
    /// failure (a missing manifest is `Ok(None)`, not an error).
    fn read_coverage(
        &self,
        track_id: &TrackId,
    ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError>;

    /// Persist the coverage `record` for `track_id`, replacing any prior record.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError::CoveragePort`] on I/O / serialization
    /// failure.
    fn write_coverage(
        &self,
        track_id: &TrackId,
        record: DryCheckCoverageRecord,
    ) -> Result<(), DryCheckCycleError>;
}
