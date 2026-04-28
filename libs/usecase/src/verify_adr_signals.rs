//! Verify-ADR-signals application service (usecase layer).
//!
//! Orchestrates the ADR signal-evaluation pipeline as a pure usecase
//! workflow. The interactor lists ADR file paths through the
//! [`AdrFilePort`] secondary port, reads each file's parsed
//! [`domain::AdrFrontMatter`] aggregate (the port already absorbs YAML
//! decoding inside the adapter — usecase never touches `serde_yaml`),
//! evaluates each [`domain::AdrDecisionEntry`] through the infallible
//! [`evaluate_adr_decision`] domain free function, and aggregates the
//! resulting [`DecisionGrounds`] counts into a single
//! [`AdrVerifyReport`] return value.
//!
//! Hexagonal boundaries (CN-05):
//! - The port surface returns domain values only — the usecase never
//!   sees `std::io::Error`, `serde_yaml::Error`, or `PathBuf` traversal.
//! - `evaluate_adr_decision` is infallible; no `?` propagation needed
//!   for the per-decision classification step.

use std::sync::Arc;

use domain::{
    AdrFilePort, AdrFilePortError, AdrVerifyReport, DecisionGrounds, evaluate_adr_decision,
};
use thiserror::Error;

/// Application-service-level error for [`VerifyAdrSignals::verify`].
///
/// Two concrete failure modes — both originate from the [`AdrFilePort`]
/// secondary port. Front-matter decode failures are absorbed inside the
/// adapter into [`AdrFilePortError::ReadFile`], so this enum does not
/// surface a separate decode variant. Signal evaluation
/// ([`evaluate_adr_decision`]) is infallible by domain contract.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerifyAdrSignalsError {
    /// Listing ADR file paths via [`AdrFilePort::list_adr_paths`] failed.
    #[error("failed to list ADR files: {0}")]
    AdrFileListing(String),
    /// Reading or parsing a single ADR file via
    /// [`AdrFilePort::read_adr_frontmatter`] failed.
    #[error("failed to read ADR file: {0}")]
    AdrFileRead(String),
}

/// CQRS command for the verify-ADR-signals operation.
///
/// Currently carries no inputs — the ADR directory is fixed by the
/// adapter implementation injected at the composition root (e.g. CLI
/// `verify adr-signals` constructs `FsAdrFileAdapter::new("knowledge/adr")`).
/// Reserved as a unit struct so future flags (e.g. `--strict`) can be
/// added without breaking callers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct VerifyAdrSignalsCommand;

/// Application service trait for the verify-ADR-signals operation.
///
/// Implementations encapsulate the orchestration; the trait surface is
/// what the CLI composition root binds to.
pub trait VerifyAdrSignals {
    /// Execute the verification and return an aggregate [`AdrVerifyReport`].
    ///
    /// The CLI layer is responsible for translating the report's
    /// `red_count` into a process exit code (AC-01).
    ///
    /// # Errors
    ///
    /// - [`VerifyAdrSignalsError::AdrFileListing`] if the port cannot
    ///   list the ADR directory.
    /// - [`VerifyAdrSignalsError::AdrFileRead`] if any single file read
    ///   or parse fails.
    fn verify(
        &self,
        command: VerifyAdrSignalsCommand,
    ) -> Result<AdrVerifyReport, VerifyAdrSignalsError>;
}

/// Concrete implementation of [`VerifyAdrSignals`].
///
/// Holds an `Arc<dyn AdrFilePort>` so callers can swap adapters (real
/// filesystem in production, mock in tests) without changing wiring.
pub struct VerifyAdrSignalsInteractor {
    port: Arc<dyn AdrFilePort>,
}

impl VerifyAdrSignalsInteractor {
    /// Create a new interactor bound to the given file port.
    #[must_use]
    pub fn new(port: Arc<dyn AdrFilePort>) -> Self {
        Self { port }
    }
}

impl VerifyAdrSignals for VerifyAdrSignalsInteractor {
    fn verify(
        &self,
        _command: VerifyAdrSignalsCommand,
    ) -> Result<AdrVerifyReport, VerifyAdrSignalsError> {
        let paths = self
            .port
            .list_adr_paths()
            .map_err(|e| VerifyAdrSignalsError::AdrFileListing(map_port_error(&e)))?;

        let mut blue_count = 0usize;
        let mut yellow_count = 0usize;
        let mut red_count = 0usize;
        let mut grandfathered_count = 0usize;

        for path in paths {
            let front_matter = self
                .port
                .read_adr_frontmatter(path)
                .map_err(|e| VerifyAdrSignalsError::AdrFileRead(map_port_error(&e)))?;
            for entry in front_matter.into_decisions() {
                tally(
                    evaluate_adr_decision(entry),
                    &mut blue_count,
                    &mut yellow_count,
                    &mut red_count,
                    &mut grandfathered_count,
                );
            }
        }

        Ok(AdrVerifyReport::new(blue_count, yellow_count, red_count, grandfathered_count))
    }
}

fn tally(
    grounds: DecisionGrounds,
    blue: &mut usize,
    yellow: &mut usize,
    red: &mut usize,
    grandfathered: &mut usize,
) {
    match grounds {
        // `knowledge/conventions/adr.md` §grandfathered (D4) excludes these
        // entries from signal evaluation. Counted in their own band — not
        // 🔵 — so back-fill debt remains observable for operators.
        DecisionGrounds::Grandfathered => *grandfathered += 1,
        DecisionGrounds::UserDecisionRef => *blue += 1,
        DecisionGrounds::ReviewFindingRef => *yellow += 1,
        DecisionGrounds::NoGrounds => *red += 1,
    }
}

fn map_port_error(error: &AdrFilePortError) -> String {
    error.to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use domain::{
        AcceptedDecision, AdrDecisionCommon, AdrDecisionEntry, AdrFrontMatter, ProposedDecision,
    };

    use super::*;

    /// In-memory mock implementing `AdrFilePort` for unit tests.
    struct MockPort {
        list_result: Result<Vec<PathBuf>, AdrFilePortError>,
        read_results: Mutex<Vec<Result<AdrFrontMatter, AdrFilePortError>>>,
    }

    impl AdrFilePort for MockPort {
        fn list_adr_paths(&self) -> Result<Vec<PathBuf>, AdrFilePortError> {
            self.list_result.clone()
        }
        fn read_adr_frontmatter(&self, _path: PathBuf) -> Result<AdrFrontMatter, AdrFilePortError> {
            self.read_results.lock().unwrap().remove(0)
        }
    }

    fn proposed_with(
        id: &str,
        user_ref: Option<&str>,
        review_ref: Option<&str>,
    ) -> AdrDecisionEntry {
        AdrDecisionEntry::ProposedDecision(ProposedDecision::new(
            AdrDecisionCommon::new(
                id,
                user_ref.map(str::to_string),
                review_ref.map(str::to_string),
                None,
                false,
            )
            .unwrap(),
        ))
    }

    fn accepted_no_grounds(id: &str) -> AdrDecisionEntry {
        AdrDecisionEntry::AcceptedDecision(AcceptedDecision::new(
            AdrDecisionCommon::new(id, None, None, None, false).unwrap(),
        ))
    }

    fn fm(adr_id: &str, decisions: Vec<AdrDecisionEntry>) -> AdrFrontMatter {
        AdrFrontMatter::new(adr_id, decisions).unwrap()
    }

    #[test]
    fn test_verify_with_all_blue_decisions_returns_zero_red() {
        let port = Arc::new(MockPort {
            list_result: Ok(vec![PathBuf::from("a.md")]),
            read_results: Mutex::new(vec![Ok(fm(
                "a",
                vec![
                    proposed_with("D1", Some("chat:2026-04-25"), None),
                    proposed_with("D2", Some("chat:2026-04-26"), None),
                ],
            ))]),
        });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let report = interactor.verify(VerifyAdrSignalsCommand).unwrap();
        assert_eq!(report.blue_count(), 2);
        assert_eq!(report.yellow_count(), 0);
        assert_eq!(report.red_count(), 0);
        assert_eq!(report.grandfathered_count(), 0);
    }

    #[test]
    fn test_verify_with_red_decision_returns_red_count_in_report() {
        let port = Arc::new(MockPort {
            list_result: Ok(vec![PathBuf::from("a.md")]),
            read_results: Mutex::new(vec![Ok(fm(
                "a",
                vec![
                    proposed_with("D1", Some("chat:2026-04-25"), None), // Blue
                    accepted_no_grounds("D2"),                          // Red
                ],
            ))]),
        });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let report = interactor.verify(VerifyAdrSignalsCommand).unwrap();
        assert_eq!(report.blue_count(), 1);
        assert_eq!(report.yellow_count(), 0);
        assert_eq!(report.red_count(), 1);
        assert_eq!(report.grandfathered_count(), 0);
    }

    #[test]
    fn test_verify_with_yellow_decision_returns_yellow_count() {
        let port = Arc::new(MockPort {
            list_result: Ok(vec![PathBuf::from("a.md")]),
            read_results: Mutex::new(vec![Ok(fm(
                "a",
                vec![proposed_with("D1", None, Some("RF-12"))],
            ))]),
        });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let report = interactor.verify(VerifyAdrSignalsCommand).unwrap();
        assert_eq!(report.blue_count(), 0);
        assert_eq!(report.yellow_count(), 1);
        assert_eq!(report.red_count(), 0);
        assert_eq!(report.grandfathered_count(), 0);
    }

    #[test]
    fn test_verify_aggregates_decisions_across_multiple_files() {
        let port = Arc::new(MockPort {
            list_result: Ok(vec![PathBuf::from("a.md"), PathBuf::from("b.md")]),
            read_results: Mutex::new(vec![
                Ok(fm("a", vec![proposed_with("D1", Some("chat"), None)])),
                Ok(fm(
                    "b",
                    vec![proposed_with("D1", None, Some("RF-1")), accepted_no_grounds("D2")],
                )),
            ]),
        });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let report = interactor.verify(VerifyAdrSignalsCommand).unwrap();
        assert_eq!(report.blue_count(), 1);
        assert_eq!(report.yellow_count(), 1);
        assert_eq!(report.red_count(), 1);
        assert_eq!(report.grandfathered_count(), 0);
    }

    #[test]
    fn test_verify_with_empty_directory_returns_all_zeros() {
        let port = Arc::new(MockPort { list_result: Ok(vec![]), read_results: Mutex::new(vec![]) });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let report = interactor.verify(VerifyAdrSignalsCommand).unwrap();
        assert_eq!(report.blue_count(), 0);
        assert_eq!(report.yellow_count(), 0);
        assert_eq!(report.red_count(), 0);
        assert_eq!(report.grandfathered_count(), 0);
    }

    #[test]
    fn test_verify_with_listing_error_returns_adr_file_listing_error() {
        let port = Arc::new(MockPort {
            list_result: Err(AdrFilePortError::ListPaths("dir missing".to_string())),
            read_results: Mutex::new(vec![]),
        });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let err = interactor.verify(VerifyAdrSignalsCommand).unwrap_err();
        assert!(matches!(err, VerifyAdrSignalsError::AdrFileListing(_)));
    }

    #[test]
    fn test_verify_with_read_error_returns_adr_file_read_error() {
        let port = Arc::new(MockPort {
            list_result: Ok(vec![PathBuf::from("bad.md")]),
            read_results: Mutex::new(vec![Err(AdrFilePortError::ReadFile(
                "file not found".to_string(),
            ))]),
        });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let err = interactor.verify(VerifyAdrSignalsCommand).unwrap_err();
        assert!(matches!(err, VerifyAdrSignalsError::AdrFileRead(_)));
    }

    #[test]
    fn test_verify_grandfathered_counts_separately_from_blue() {
        let entry = AdrDecisionEntry::ProposedDecision(ProposedDecision::new(
            AdrDecisionCommon::new("D1", None, None, None, true).unwrap(),
        ));
        let port = Arc::new(MockPort {
            list_result: Ok(vec![PathBuf::from("a.md")]),
            read_results: Mutex::new(vec![Ok(fm("a", vec![entry]))]),
        });
        let interactor = VerifyAdrSignalsInteractor::new(port);
        let report = interactor.verify(VerifyAdrSignalsCommand).unwrap();
        assert_eq!(report.blue_count(), 0);
        assert_eq!(report.yellow_count(), 0);
        assert_eq!(report.red_count(), 0);
        assert_eq!(report.grandfathered_count(), 1);
    }
}
