//! Query result type for the dry-check read path.

use domain::dry_check::DryCheckRecord;

/// Informational query output from [`super::DryCheckResultsService`]
/// (AC-07/D10 read path).
///
/// Carries the list of [`DryCheckRecord`] entries that passed the
/// `VerdictFilter` applied by `get_results()`. Each record uses the canonical
/// sorted model: `pair_key` holds two `FragmentRef`s `(path, content_hash)`
/// sorted lexicographically by `(path, content_hash)` order (`low <= high`);
/// `changed_path` is a display-only field (diff fragment side); `verdict`
/// (enum-first: `Violation { refactor_proposal }` / `NotAViolation` /
/// `Accepted`); `similarity_score`, `threshold`, `base_commit`, `rationale`,
/// and `recorded_at`.
///
/// For `VerdictFilter::All` the list is the latest-per-pair records from the
/// full history; for a specific filter variant (`NotAViolation` / `Accepted`
/// / `Violation`) only matching records are included.
///
/// The CLI read command surfaces `refactor_proposal` for `Violation` records
/// by matching on `DryCheckVerdict::Violation { refactor_proposal }` — no
/// separate field required.
///
/// The authoritative current-scope gate is
/// [`super::DryCheckApprovalService::check_approved`] (AC-04), which takes
/// `corpus_fragments + diff_fragments + threshold`. The write path
/// (`DryCheckService::run_dry_check`) surfaces live `DryCheckFinding`
/// proposals to dfl — that is a separate concern.
#[derive(Debug, Clone)]
pub struct DryCheckResults {
    /// Filtered latest-per-pair dry-check records.
    pub records: Vec<DryCheckRecord>,
}
