//! Application service trait definitions for the dry-check use case.

use domain::CommitHash;
use domain::dry_check::{
    DryCheckApprovalVerdict, DryCheckFinding, DryCheckReaderError, VerdictFilter,
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

/// Application service for the dry-check gate (D10 gate operation, AC-04).
///
/// Receives the full workspace corpus (`corpus_fragments`) plus diff fragments
/// and threshold. Builds a fresh whole-codebase index from `corpus_fragments`,
/// queries each diff fragment at `threshold`, then reads all history via
/// `DryCheckReader.read_records()`, derives the latest-per-pair verdicts by
/// computing `FragmentRef`s (SHA-256 of `content()`) for each pair and building
/// `DryCheckPairKey` for identity matching (CN-07 identifier-based
/// invalidation: if the `DryCheckPairKey` built from current fragments matches
/// an existing record's `pair_key`, the pair is verified), and returns
/// `Approved` only when all above-threshold pairs are verified as
/// not-a-violation or accepted. Blocks otherwise.
///
/// Does NOT record new verdicts — no `base_commit` param needed.
///
/// Implemented by `DryCheckApprovalInteractor` (T005).
pub trait DryCheckApprovalService {
    /// Evaluate the dry-check gate for the current diff scope.
    ///
    /// # Errors
    ///
    /// Returns [`DryCheckCycleError`] on embedding, index, or reader failures.
    fn check_approved(
        &self,
        corpus_fragments: Vec<CodeFragment>,
        diff_fragments: &[CodeFragment],
        threshold: SimilarityThreshold,
    ) -> Result<DryCheckApprovalVerdict, DryCheckCycleError>;
}
