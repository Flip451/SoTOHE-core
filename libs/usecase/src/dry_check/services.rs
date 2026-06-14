//! Application service trait definitions for the dry-check use case.

use std::collections::BTreeSet;

use domain::CommitHash;
use domain::TrackId;
use domain::dry_check::{
    DryCheckApprovalVerdict, DryCheckFinding, DryCheckReaderError, FragmentRef, VerdictFilter,
};
use domain::semantic_dup::{CodeFragment, SimilarityThreshold};

use super::errors::DryCheckCycleError;
use super::results::DryCheckResults;

// ── DryCheckService ───────────────────────────────────────────────────────────

/// Application service for the dry-check use case (write path, D10).
///
/// Receives the full workspace corpus (`corpus_fragments` — all Rust source
/// fragments extracted by the CLI, needed to build the single whole-codebase
/// index per IN-02/D4/spec OS-03) plus the diff fragments to query, the
/// similarity threshold, and the git base commit the diff was computed against
/// (`base_commit` — resolved by the CLI via the per-track `.commit_hash`
/// mechanism, ancestor-checked with main-tip fallback, with `--base-commit` as
/// optional override; IN-02/AC-02).
///
/// Orchestrates index build from corpus, diff fragment query at threshold,
/// agent judgment, and verdict persistence via `DryCheckWriter.append_record`.
///
/// Returns the findings (with refactor proposals and `FragmentRef`s) for any
/// genuine violations discovered in this run (IN-03/AC-03), so dfl can act on
/// them. An empty `Vec` means no unresolved violations were found.
///
/// Implemented by `DryCheckInteractor` (T004).
pub trait DryCheckService {
    /// Run the dry-check write cycle.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError`] on embedding, index, agent, persistence,
    /// diff, or entry construction failures.
    fn run_dry_check(
        &self,
        corpus_fragments: Vec<CodeFragment>,
        diff_fragments: Vec<CodeFragment>,
        threshold: SimilarityThreshold,
        base_commit: CommitHash,
    ) -> Result<Vec<DryCheckFinding>, DryCheckCycleError>;
}

// ── DryCheckResultsService ────────────────────────────────────────────────────

/// Application service for the dry-check results read path (D10 read operation,
/// AC-07).
///
/// Accepts a [`VerdictFilter`] to restrict results to a specific classification
/// (`All` / `NotAViolation` / `Accepted` / `Violation`). Returns
/// [`DryCheckResults`] carrying the matching latest-per-pair
/// [`domain::dry_check::DryCheckRecord`] list from `dry-check.json`.
///
/// No direct `dry-check.json` read — implemented by `DryCheckResultsInteractor`
/// (T005) via `DryCheckReader` port.
///
/// The authoritative current-scope gate is
/// [`DryCheckApprovalService::check_approved`] (AC-04) which takes
/// `corpus_fragments + diff_fragments + threshold`.
pub trait DryCheckResultsService {
    /// Read and filter the latest-per-pair dry-check records.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckReaderError`] on I/O, codec, invalid data, or schema
    /// incompatibility failures.
    fn get_results(&self, filter: VerdictFilter) -> Result<DryCheckResults, DryCheckReaderError>;
}

// ── DryCheckApprovalService ───────────────────────────────────────────────────

/// Application service for the dry-check gate (D5 / IN-05 / AC-10 / AC-12 /
/// CN-08 / CN-09).
///
/// Pure read-only staleness + all-resolved gate — no embedding, no similarity
/// search, no agent invocation. Composition computes the set of current diff
/// fragments' `FragmentRef`s (path + content_hash) and passes them in.
///
/// Algorithm:
///
/// 1. `coverage.read_coverage(track_id)`:
///    - `Ok(None)` → return `Blocked` (CN-08 fail-closed: no coverage manifest).
///    - `Ok(Some(record))` → continue.
/// 2. Staleness: each `current_fragment_refs` entry must be present in the
///    coverage record. Any miss → `Blocked` (matched at FragmentRef =
///    (path + content_hash); an identical content_hash at a different path is
///    NOT covered — IN-06 / CN-08).
/// 3. All-resolved: read all records via `reader.read_records()`, build the
///    latest-per-pair map keyed by `DryCheckPairKey`, then for each record
///    whose pair touches any current `FragmentRef`, the latest verdict must be
///    `NotAViolation` or `Accepted`. Any `Violation` → `Blocked`. Past
///    `Violation` followed by a later `Accepted` / `NotAViolation` is
///    resolved.
/// 4. All steps pass → `Approved`.
///
/// Implemented by `DryCheckApprovalInteractor` (T003).
pub trait DryCheckApprovalService {
    /// Evaluate the dry-check gate for the current diff scope (pure-read).
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError::Reader`] on history read failures or
    /// [`DryCheckCycleError::CoveragePort`] on coverage-manifest failures.
    fn check_approved(
        &self,
        track_id: &TrackId,
        current_fragment_refs: &BTreeSet<FragmentRef>,
    ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError>;
}
