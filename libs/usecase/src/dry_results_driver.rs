//! Driver-level application service for `sotp dry results`.
//!
//! [`DryResultsDriverInteractor`] performs the entire read-only orchestration
//! itself (ADR 2026-06-21-1328 D4): it validates the raw `track_id` string,
//! parses the verdict filter, resolves the workspace, and invokes the
//! existing [`crate::dry_check::DryCheckResultsInteractor`] directly — no
//! infrastructure adapter sits between this interactor and the domain-level
//! read call.
//!
//! Ordered after [`crate::dry_driver_shared`] (T024): reuses its
//! `DryRepoRootPort` and `DryCheckStorageFactoryPort`. `dry results` needs no
//! `base_branch`, so `DryBaseBranchPort` is not injected here per CN-07
//! no-new-metadata-dependency.
//!
//! This module is purely additive: nothing outside it references these types
//! yet, so it compiles standalone.
//!
//! Design: ADR 2026-06-21-1328 D2 / D4 / D7, IN-14, AC-20, CN-07.

use std::sync::Arc;

use domain::TrackId;
use domain::dry_check::VerdictFilter;

use crate::dry_check::{DryCheckResultsInteractor, DryCheckResultsService as _};
use crate::dry_driver::DryResultsDriverInput;
use crate::dry_driver_shared::{DryCheckStorageFactoryPort, DryRepoRootPort};

// ── DryResultsVerdictSummary ──────────────────────────────────────────────────

/// Usecase-level mirror of [`domain::dry_check::DryCheckVerdict`]'s three
/// variants, kept typed rather than flattened to a `String` because the T032
/// render step branches on its 3-way structure.
#[derive(Debug, Clone)]
pub enum DryResultsVerdictSummary {
    /// False positive: the agent determined this pair is not a DRY violation.
    NotAViolation,
    /// Agent-approved duplication: acceptable similarity.
    Accepted,
    /// Genuine DRY violation carrying the non-empty refactor proposal text.
    Violation {
        /// Non-empty refactor proposal text.
        refactor_proposal: String,
    },
}

// ── DryResultsRecordSummary ───────────────────────────────────────────────────

/// Flattened `String`/`f32` mirror of [`domain::dry_check::DryCheckRecord`]'s
/// `pair_key`/`changed_path`/`verdict`/`similarity_score`/`threshold`/
/// `base_commit`/`rationale`/`recorded_at` accessors.
#[derive(Debug, Clone)]
pub struct DryResultsRecordSummary {
    /// Lower fragment's repo-relative path (by `(path, content_hash)` order).
    pub low_path: String,
    /// Lower fragment's SHA-256 content hash.
    pub low_content_hash: String,
    /// Higher fragment's repo-relative path (by `(path, content_hash)` order).
    pub high_path: String,
    /// Higher fragment's SHA-256 content hash.
    pub high_content_hash: String,
    /// Display-only changed path (one of the pair's two paths).
    pub changed_path: String,
    /// The recorded verdict.
    pub verdict: DryResultsVerdictSummary,
    /// Cosine similarity score at judgment time.
    pub similarity_score: f32,
    /// The similarity threshold active at judgment time.
    pub threshold: f32,
    /// The diff base commit hash the pair was judged against.
    pub base_commit: String,
    /// The agent's rationale text.
    pub rationale: String,
    /// ISO-8601 record timestamp.
    pub recorded_at: String,
}

// ── DryResultsOutcome ──────────────────────────────────────────────────────────

/// Outcome of `sotp dry results` (driver boundary).
#[derive(Debug, Clone)]
pub enum DryResultsOutcome {
    /// Records matching the requested [`domain::dry_check::VerdictFilter`].
    Success {
        /// The matching latest-per-pair records.
        records: Vec<DryResultsRecordSummary>,
    },
    /// Validation, workspace resolution, or a port call failed.
    Failure {
        /// Human-readable diagnostic message.
        message: String,
    },
}

// ── DryResultsDriverService ───────────────────────────────────────────────────

/// Application service (primary port) for `sotp dry results`.
pub trait DryResultsDriverService: Send + Sync {
    /// Run `sotp dry results`.
    fn dry_results(&self, input: DryResultsDriverInput) -> DryResultsOutcome;
}

// ── DryResultsDriverInteractor ────────────────────────────────────────────────

/// Interactor implementing [`DryResultsDriverService`].
///
/// Holds the two secondary ports below as private `Arc<dyn Port>` fields and
/// performs the entire read-only orchestration itself.
pub struct DryResultsDriverInteractor {
    repo_root: Arc<dyn DryRepoRootPort>,
    storage_factory: Arc<dyn DryCheckStorageFactoryPort>,
}

impl DryResultsDriverInteractor {
    /// Construct a new [`DryResultsDriverInteractor`] with all required ports.
    #[must_use]
    pub fn new(
        repo_root: Arc<dyn DryRepoRootPort>,
        storage_factory: Arc<dyn DryCheckStorageFactoryPort>,
    ) -> Self {
        Self { repo_root, storage_factory }
    }
}

/// Parse a verdict filter string to [`VerdictFilter`].
///
/// Accepted values (case-insensitive): `"all"`, `"not-a-violation"`,
/// `"accepted"`, `"violation"`.
fn parse_verdict_filter(s: &str) -> Result<VerdictFilter, String> {
    match s.to_ascii_lowercase().as_str() {
        "all" => Ok(VerdictFilter::All),
        "not-a-violation" => Ok(VerdictFilter::NotAViolation),
        "accepted" => Ok(VerdictFilter::Accepted),
        "violation" => Ok(VerdictFilter::Violation),
        other => Err(format!(
            "invalid --filter '{other}' (expected: all / not-a-violation / accepted / violation)"
        )),
    }
}

impl DryResultsDriverService for DryResultsDriverInteractor {
    fn dry_results(&self, input: DryResultsDriverInput) -> DryResultsOutcome {
        // ── (1) Validate track_id. ───────────────────────────────────────────────
        let track_id = match TrackId::try_new(input.track_id) {
            Ok(id) => id,
            Err(e) => return DryResultsOutcome::Failure { message: e.to_string() },
        };

        // ── (2) Parse the verdict filter (pure logic). ──────────────────────────
        let filter = match parse_verdict_filter(&input.filter) {
            Ok(f) => f,
            Err(message) => return DryResultsOutcome::Failure { message },
        };

        // ── (3) Resolve workspace paths. ─────────────────────────────────────────
        let workspace = match self.repo_root.resolve(&input.items_dir) {
            Ok(w) => w,
            Err(e) => return DryResultsOutcome::Failure { message: e.to_string() },
        };

        // ── (4) Derive track_dir. ────────────────────────────────────────────────
        let track_dir = workspace.canonical_items_dir.join(track_id.as_ref());

        // ── (5) Build the storage trio (reader only). ────────────────────────────
        let storage = self.storage_factory.build(&track_dir, &workspace.canonical_root);

        // ── (6) Invoke the domain-level read call directly. ─────────────────────
        let interactor = DryCheckResultsInteractor::new(storage.reader);
        let results = match interactor.get_results(filter) {
            Ok(r) => r,
            Err(e) => return DryResultsOutcome::Failure { message: e.to_string() },
        };

        // ── (7) Map each record into a DryResultsRecordSummary. ─────────────────
        let records: Vec<DryResultsRecordSummary> = results
            .records
            .into_iter()
            .map(|record| DryResultsRecordSummary {
                low_path: record.pair_key().low().path().as_str().to_owned(),
                low_content_hash: record.pair_key().low().content_hash().as_str().to_owned(),
                high_path: record.pair_key().high().path().as_str().to_owned(),
                high_content_hash: record.pair_key().high().content_hash().as_str().to_owned(),
                changed_path: record.changed_path().as_str().to_owned(),
                verdict: match record.verdict() {
                    domain::dry_check::DryCheckVerdict::NotAViolation => {
                        DryResultsVerdictSummary::NotAViolation
                    }
                    domain::dry_check::DryCheckVerdict::Accepted => {
                        DryResultsVerdictSummary::Accepted
                    }
                    domain::dry_check::DryCheckVerdict::Violation { refactor_proposal } => {
                        DryResultsVerdictSummary::Violation {
                            refactor_proposal: refactor_proposal.as_str().to_owned(),
                        }
                    }
                },
                similarity_score: record.similarity_score().value(),
                threshold: record.threshold().value(),
                base_commit: record.base_commit().as_ref().to_owned(),
                rationale: record.rationale().as_str().to_owned(),
                recorded_at: record.recorded_at().as_str().to_owned(),
            })
            .collect();

        DryResultsOutcome::Success { records }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::path::{Path, PathBuf};

    use domain::Timestamp;
    use domain::dry_check::{
        DryCheckCoverageRecord, DryCheckEntry, DryCheckPairKey, DryCheckReaderError,
        DryCheckRecord, DryCheckVerdict, FragmentContentHash, FragmentRef, Rationale,
    };
    use domain::review_v2::FilePath;
    use domain::semantic_dup::{SimilarityScore, SimilarityThreshold};

    use super::*;
    use crate::dry_check::DryCheckCycleError;
    use crate::dry_driver_shared::{
        DryBaseBranchError, DryBaseBranchPort, DryCheckStorageHandle, DryRepoWorkspace,
        DryRepoWorkspaceError,
    };

    // ── Test doubles ─────────────────────────────────────────────────────────

    struct StubRepoRoot {
        result: Result<DryRepoWorkspace, DryRepoWorkspaceError>,
    }

    impl DryRepoRootPort for StubRepoRoot {
        fn resolve(&self, _items_dir: &Path) -> Result<DryRepoWorkspace, DryRepoWorkspaceError> {
            self.result.clone()
        }
    }

    struct StubReader {
        records: Vec<DryCheckRecord>,
    }

    impl domain::dry_check::DryCheckReader for StubReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Ok(self.records.clone())
        }
    }

    struct ErrorReader;

    impl domain::dry_check::DryCheckReader for ErrorReader {
        fn read_records(&self) -> Result<Vec<DryCheckRecord>, DryCheckReaderError> {
            Err(DryCheckReaderError::Io {
                path: "dry-check.json".to_owned(),
                detail: "boom".to_owned(),
            })
        }
    }

    struct NoopWriter;

    impl domain::dry_check::DryCheckWriter for NoopWriter {
        fn append_record(
            &self,
            _entry: &DryCheckEntry,
        ) -> Result<(), domain::dry_check::DryCheckWriterError> {
            Ok(())
        }
    }

    struct NoopCoverage;

    impl crate::dry_check::DryCheckCoveragePort for NoopCoverage {
        fn read_coverage(
            &self,
            _track_id: &TrackId,
        ) -> Result<Option<DryCheckCoverageRecord>, DryCheckCycleError> {
            Ok(None)
        }

        fn write_coverage(
            &self,
            _track_id: &TrackId,
            _record: DryCheckCoverageRecord,
        ) -> Result<(), DryCheckCycleError> {
            Ok(())
        }
    }

    struct StubStorageFactory {
        records: Vec<DryCheckRecord>,
        fail_reader: bool,
    }

    impl DryCheckStorageFactoryPort for StubStorageFactory {
        fn build(&self, _track_dir: &Path, _canonical_root: &Path) -> DryCheckStorageHandle {
            let reader: Arc<dyn domain::dry_check::DryCheckReader> = if self.fail_reader {
                Arc::new(ErrorReader)
            } else {
                Arc::new(StubReader { records: self.records.clone() })
            };
            DryCheckStorageHandle {
                reader,
                writer: Arc::new(NoopWriter),
                coverage: Arc::new(NoopCoverage),
            }
        }
    }

    #[allow(dead_code)]
    struct UnusedBaseBranch;

    impl DryBaseBranchPort for UnusedBaseBranch {
        fn resolve_base_branch(
            &self,
            _canonical_root: &Path,
            _track_dir: &Path,
            _track_id: &TrackId,
        ) -> Result<String, DryBaseBranchError> {
            panic!("dry_results must never resolve base_branch (CN-07)")
        }
    }

    // ── Helpers ──────────────────────────────────────────────────────────────

    fn make_fragment_ref(path: &str, hash_char: char) -> FragmentRef {
        FragmentRef::new(
            FilePath::new(path).unwrap(),
            FragmentContentHash::new(hash_char.to_string().repeat(64)).unwrap(),
        )
    }

    fn make_record(verdict: DryCheckVerdict) -> DryCheckRecord {
        let low = make_fragment_ref("src/a.rs", 'a');
        let high = make_fragment_ref("src/b.rs", 'b');
        let pair_key = DryCheckPairKey::new(low.clone(), high).unwrap();
        let entry = DryCheckEntry::new(
            pair_key,
            low.path().clone(),
            verdict,
            SimilarityScore::new(0.9).unwrap(),
            SimilarityThreshold::new(0.85).unwrap(),
            domain::CommitHash::try_new("c".repeat(40)).unwrap(),
            Rationale::new("test rationale").unwrap(),
            domain::dry_check::DryCheckConfigFingerprint::new("d".repeat(64)).unwrap(),
        )
        .unwrap();
        DryCheckRecord::from_entry_and_timestamp(
            entry,
            Timestamp::new("2026-06-30T00:00:00Z").unwrap(),
        )
        .unwrap()
    }

    fn make_workspace() -> DryRepoWorkspace {
        DryRepoWorkspace {
            repo_root: PathBuf::from("/repo"),
            canonical_root: PathBuf::from("/repo"),
            canonical_items_dir: PathBuf::from("/repo/track/items"),
        }
    }

    fn make_input(filter: &str) -> DryResultsDriverInput {
        DryResultsDriverInput {
            track_id: "test-track-2026".to_owned(),
            filter: filter.to_owned(),
            items_dir: PathBuf::from("track/items"),
        }
    }

    fn make_interactor(
        records: Vec<DryCheckRecord>,
        fail_reader: bool,
    ) -> DryResultsDriverInteractor {
        DryResultsDriverInteractor::new(
            Arc::new(StubRepoRoot { result: Ok(make_workspace()) }),
            Arc::new(StubStorageFactory { records, fail_reader }),
        )
    }

    // ── Success path ─────────────────────────────────────────────────────────

    #[test]
    fn dry_results_success_maps_record_fields() {
        let record = make_record(DryCheckVerdict::Violation {
            refactor_proposal: domain::dry_check::RefactorProposal::new("Extract helper.").unwrap(),
        });
        let interactor = make_interactor(vec![record], false);

        let outcome = interactor.dry_results(make_input("all"));

        match outcome {
            DryResultsOutcome::Success { records } => {
                assert_eq!(records.len(), 1);
                let r = &records[0];
                assert_eq!(r.low_path, "src/a.rs");
                assert_eq!(r.low_content_hash, "a".repeat(64));
                assert_eq!(r.high_path, "src/b.rs");
                assert_eq!(r.high_content_hash, "b".repeat(64));
                assert_eq!(r.changed_path, "src/a.rs");
                assert!((r.similarity_score - 0.9_f32).abs() < f32::EPSILON);
                assert!((r.threshold - 0.85_f32).abs() < f32::EPSILON);
                assert_eq!(r.base_commit, "c".repeat(40));
                assert_eq!(r.rationale, "test rationale");
                assert_eq!(r.recorded_at, "2026-06-30T00:00:00Z");
                match &r.verdict {
                    DryResultsVerdictSummary::Violation { refactor_proposal } => {
                        assert_eq!(refactor_proposal, "Extract helper.");
                    }
                    other => panic!("expected Violation, got {other:?}"),
                }
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    #[test]
    fn dry_results_not_a_violation_and_accepted_map_correctly() {
        let interactor = make_interactor(
            vec![
                make_record(DryCheckVerdict::NotAViolation),
                make_record(DryCheckVerdict::Accepted),
            ],
            false,
        );
        let outcome = interactor.dry_results(make_input("all"));
        match outcome {
            DryResultsOutcome::Success { records } => {
                assert_eq!(records.len(), 1, "same pair_key: latest record wins (Accepted)");
                assert!(matches!(records[0].verdict, DryResultsVerdictSummary::Accepted));
            }
            other => panic!("expected Success, got {other:?}"),
        }
    }

    // ── Filter parsing ───────────────────────────────────────────────────────

    #[test]
    fn dry_results_filter_all_variants_parse_case_insensitively() {
        for filter in ["all", "ALL", "not-a-violation", "Accepted", "VIOLATION"] {
            let interactor = make_interactor(vec![], false);
            let outcome = interactor.dry_results(make_input(filter));
            assert!(
                matches!(outcome, DryResultsOutcome::Success { .. }),
                "filter '{filter}' must parse successfully, got {outcome:?}"
            );
        }
    }

    #[test]
    fn dry_results_unrecognized_filter_returns_failure() {
        let interactor = make_interactor(vec![], false);
        let outcome = interactor.dry_results(make_input("bogus"));
        match outcome {
            DryResultsOutcome::Failure { message } => {
                assert!(message.contains("invalid --filter"), "got: {message}");
            }
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    // ── Port failure propagation ─────────────────────────────────────────────

    #[test]
    fn dry_results_invalid_track_id_returns_failure() {
        let interactor = make_interactor(vec![], false);
        let mut input = make_input("all");
        input.track_id = String::new();
        let outcome = interactor.dry_results(input);
        assert!(matches!(outcome, DryResultsOutcome::Failure { .. }));
    }

    #[test]
    fn dry_results_repo_root_failure_returns_failure() {
        let interactor = DryResultsDriverInteractor::new(
            Arc::new(StubRepoRoot {
                result: Err(DryRepoWorkspaceError::Unavailable("repo boom".to_owned())),
            }),
            Arc::new(StubStorageFactory { records: vec![], fail_reader: false }),
        );
        let outcome = interactor.dry_results(make_input("all"));
        match outcome {
            DryResultsOutcome::Failure { message } => assert!(message.contains("repo boom")),
            other => panic!("expected Failure, got {other:?}"),
        }
    }

    #[test]
    fn dry_results_reader_failure_returns_failure() {
        let interactor = make_interactor(vec![], true);
        let outcome = interactor.dry_results(make_input("all"));
        assert!(matches!(outcome, DryResultsOutcome::Failure { .. }));
    }
}
