//! Typed filter conversion for the `signal report` primary-adapter boundary.

use std::sync::Arc;

use usecase::signal_report::{
    SignalReportChain, SignalReportChainSelection, SignalReportError, SignalReportLevel,
    SignalReportLevelSelection, SignalReportOccurrence, SignalReportQuery, SignalReportService,
};

use crate::render::CommandOutcome;

/// A transport-level filter selecting report chains.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReportChainFilter {
    /// Include every report chain.
    All,
    /// Include only the ADR-to-user chain.
    AdrUser,
    /// Include only the specification-to-ADR chain.
    SpecAdr,
    /// Include only the catalogue-to-specification chain.
    CatalogSpec,
    /// Include only the implementation-to-catalogue chain.
    ImplCatalog,
}

/// A transport-level filter selecting report signal levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReportLevelFilter {
    /// Include only yellow occurrences.
    YellowOnly,
    /// Include only red occurrences.
    RedOnly,
    /// Include both yellow and red occurrences.
    YellowAndRed,
}

/// Typed `signal report` input accepted by the CLI driver.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalReportInput {
    /// The requested report-chain filter.
    pub chain: SignalReportChainFilter,
    /// The requested signal-level filter.
    pub levels: SignalReportLevelFilter,
}

fn to_signal_report_query(input: SignalReportInput) -> SignalReportQuery {
    let chain = match input.chain {
        SignalReportChainFilter::All => SignalReportChainSelection::All,
        SignalReportChainFilter::AdrUser => {
            SignalReportChainSelection::One(SignalReportChain::AdrUser)
        }
        SignalReportChainFilter::SpecAdr => {
            SignalReportChainSelection::One(SignalReportChain::SpecAdr)
        }
        SignalReportChainFilter::CatalogSpec => {
            SignalReportChainSelection::One(SignalReportChain::CatalogSpec)
        }
        SignalReportChainFilter::ImplCatalog => {
            SignalReportChainSelection::One(SignalReportChain::ImplCatalog)
        }
    };
    let levels = match input.levels {
        SignalReportLevelFilter::YellowOnly => SignalReportLevelSelection::YellowOnly,
        SignalReportLevelFilter::RedOnly => SignalReportLevelSelection::RedOnly,
        SignalReportLevelFilter::YellowAndRed => SignalReportLevelSelection::YellowAndRed,
    };

    SignalReportQuery { chain, levels }
}

/// Primary adapter for rendering typed signal-report occurrences.
pub struct SignalReportDriver {
    service: Arc<dyn SignalReportService>,
}

impl SignalReportDriver {
    /// Creates a report driver backed by the supplied application service.
    #[must_use]
    pub fn new(service: Arc<dyn SignalReportService>) -> Self {
        Self { service }
    }

    /// Queries and renders every selected signal occurrence.
    #[must_use]
    pub fn handle(&self, input: SignalReportInput) -> CommandOutcome {
        match self.service.report(to_signal_report_query(input)) {
            Ok(output) => CommandOutcome::success(Some(render_occurrences(&output.occurrences))),
            Err(error) => CommandOutcome::failure(Some(render_error(error))),
        }
    }
}

fn render_occurrences(occurrences: &[SignalReportOccurrence]) -> String {
    if occurrences.is_empty() {
        return "signal report: no matching occurrences".to_owned();
    }

    let mut lines = Vec::with_capacity(occurrences.len() + 1);
    lines.push(format!("signal report: {} occurrence(s)", occurrences.len()));
    for occurrence in occurrences {
        lines.push(format!(
            "chain={} level={} entry_id={} reference={} reason={} location={}",
            chain_label(occurrence.chain),
            level_label(occurrence.level),
            occurrence.entry_id,
            occurrence.reference,
            occurrence.reason,
            occurrence.location,
        ));
    }
    lines.join("\n")
}

fn render_error(error: SignalReportError) -> String {
    match error {
        SignalReportError::SourceUnavailable(chain) => {
            format!("signal report: source unavailable for {}", chain_label(chain))
        }
        SignalReportError::InvalidOccurrence(chain) => {
            format!(
                "signal report: source returned an invalid occurrence for {}",
                chain_label(chain)
            )
        }
    }
}

const fn chain_label(chain: SignalReportChain) -> &'static str {
    match chain {
        SignalReportChain::AdrUser => "adr_user",
        SignalReportChain::SpecAdr => "spec_adr",
        SignalReportChain::CatalogSpec => "catalog_spec",
        SignalReportChain::ImplCatalog => "impl_catalog",
    }
}

const fn level_label(level: SignalReportLevel) -> &'static str {
    match level {
        SignalReportLevel::Yellow => "yellow",
        SignalReportLevel::Red => "red",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use domain::NonEmptyString;
    use domain::review_v2::types::FilePath;
    use usecase::signal_report::{
        SignalReportEntryId, SignalReportLocation, SignalReportOccurrence, SignalReportOutput,
        SignalReportReason, SignalReportReference,
    };

    use super::*;

    struct StubSignalReportService {
        result: Mutex<Option<Result<SignalReportOutput, SignalReportError>>>,
        query: Mutex<Option<SignalReportQuery>>,
    }

    impl StubSignalReportService {
        fn successful(occurrences: Vec<SignalReportOccurrence>) -> Self {
            Self {
                result: Mutex::new(Some(Ok(SignalReportOutput { occurrences }))),
                query: Mutex::new(None),
            }
        }

        fn failing(error: SignalReportError) -> Self {
            Self { result: Mutex::new(Some(Err(error))), query: Mutex::new(None) }
        }
    }

    impl SignalReportService for StubSignalReportService {
        fn report(
            &self,
            query: SignalReportQuery,
        ) -> Result<SignalReportOutput, SignalReportError> {
            *self.query.lock().expect("test mutex must not be poisoned") = Some(query);
            self.result
                .lock()
                .expect("test mutex must not be poisoned")
                .take()
                .unwrap_or(Err(SignalReportError::SourceUnavailable(SignalReportChain::AdrUser)))
        }
    }

    fn occurrence(
        chain: SignalReportChain,
        level: SignalReportLevel,
        entry_id: &str,
        reference: &str,
        reason: &str,
        location: &str,
    ) -> SignalReportOccurrence {
        SignalReportOccurrence {
            chain,
            level,
            entry_id: SignalReportEntryId::new(NonEmptyString::try_new(entry_id).unwrap()),
            reference: SignalReportReference::new(NonEmptyString::try_new(reference).unwrap()),
            reason: SignalReportReason::new(NonEmptyString::try_new(reason).unwrap()),
            location: SignalReportLocation::new(FilePath::new(location).unwrap()),
        }
    }

    #[test]
    fn test_signal_report_input_each_chain_filter_converts_to_expected_selection() {
        let cases = [
            (SignalReportChainFilter::All, SignalReportChainSelection::All),
            (
                SignalReportChainFilter::AdrUser,
                SignalReportChainSelection::One(SignalReportChain::AdrUser),
            ),
            (
                SignalReportChainFilter::SpecAdr,
                SignalReportChainSelection::One(SignalReportChain::SpecAdr),
            ),
            (
                SignalReportChainFilter::CatalogSpec,
                SignalReportChainSelection::One(SignalReportChain::CatalogSpec),
            ),
            (
                SignalReportChainFilter::ImplCatalog,
                SignalReportChainSelection::One(SignalReportChain::ImplCatalog),
            ),
        ];

        for (filter, expected_chain) in cases {
            let query = to_signal_report_query(SignalReportInput {
                chain: filter,
                levels: SignalReportLevelFilter::YellowAndRed,
            });

            assert_eq!(query.chain, expected_chain);
            assert_eq!(query.levels, SignalReportLevelSelection::YellowAndRed);
        }
    }

    #[test]
    fn test_signal_report_input_each_level_filter_converts_to_expected_selection() {
        let cases = [
            (SignalReportLevelFilter::YellowOnly, SignalReportLevelSelection::YellowOnly),
            (SignalReportLevelFilter::RedOnly, SignalReportLevelSelection::RedOnly),
            (SignalReportLevelFilter::YellowAndRed, SignalReportLevelSelection::YellowAndRed),
        ];

        for (filter, expected_levels) in cases {
            let query = to_signal_report_query(SignalReportInput {
                chain: SignalReportChainFilter::All,
                levels: filter,
            });

            assert_eq!(query.chain, SignalReportChainSelection::All);
            assert_eq!(query.levels, expected_levels);
        }
    }

    #[test]
    fn test_signal_report_driver_renders_complete_occurrences_in_service_order() {
        let service = Arc::new(StubSignalReportService::successful(vec![
            occurrence(
                SignalReportChain::CatalogSpec,
                SignalReportLevel::Red,
                "CatalogEntry",
                "spec.json#IN-02",
                "missing reference",
                "track/items/example/cli_driver-types.json",
            ),
            occurrence(
                SignalReportChain::AdrUser,
                SignalReportLevel::Yellow,
                "ADR-01",
                "adr.md#D1",
                "needs evidence",
                "knowledge/adr/example.md",
            ),
            occurrence(
                SignalReportChain::SpecAdr,
                SignalReportLevel::Red,
                "IN-02",
                "adr.md#D1",
                "unresolved ADR reference",
                "track/items/example/spec.json",
            ),
            occurrence(
                SignalReportChain::ImplCatalog,
                SignalReportLevel::Yellow,
                "SignalReportDriver",
                "cli_driver-types.json#AC-02",
                "implementation reference pending review",
                "apps/cli-driver/src/signal_report.rs",
            ),
        ]));
        let driver = SignalReportDriver::new(service.clone());

        let outcome = driver.handle(SignalReportInput {
            chain: SignalReportChainFilter::All,
            levels: SignalReportLevelFilter::YellowAndRed,
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "signal report: 4 occurrence(s)\n\
                 chain=catalog_spec level=red entry_id=CatalogEntry reference=spec.json#IN-02 reason=missing reference location=track/items/example/cli_driver-types.json\n\
                 chain=adr_user level=yellow entry_id=ADR-01 reference=adr.md#D1 reason=needs evidence location=knowledge/adr/example.md\n\
                 chain=spec_adr level=red entry_id=IN-02 reference=adr.md#D1 reason=unresolved ADR reference location=track/items/example/spec.json\n\
                 chain=impl_catalog level=yellow entry_id=SignalReportDriver reference=cli_driver-types.json#AC-02 reason=implementation reference pending review location=apps/cli-driver/src/signal_report.rs"
            )
        );
        assert_eq!(
            *service.query.lock().expect("test mutex must not be poisoned"),
            Some(SignalReportQuery {
                chain: SignalReportChainSelection::All,
                levels: SignalReportLevelSelection::YellowAndRed,
            })
        );
    }

    #[test]
    fn test_signal_report_driver_renders_empty_success_output() {
        let driver = SignalReportDriver::new(Arc::new(StubSignalReportService::successful(vec![])));

        let outcome = driver.handle(SignalReportInput {
            chain: SignalReportChainFilter::All,
            levels: SignalReportLevelFilter::YellowAndRed,
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.stdout.as_deref(), Some("signal report: no matching occurrences"));
    }

    #[test]
    fn test_signal_report_driver_returns_bounded_failure_for_usecase_error() {
        let driver = SignalReportDriver::new(Arc::new(StubSignalReportService::failing(
            SignalReportError::SourceUnavailable(SignalReportChain::ImplCatalog),
        )));

        let outcome = driver.handle(SignalReportInput {
            chain: SignalReportChainFilter::ImplCatalog,
            levels: SignalReportLevelFilter::YellowAndRed,
        });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("signal report: source unavailable for impl_catalog")
        );
    }

    #[test]
    fn test_signal_report_driver_returns_bounded_failure_for_invalid_occurrence_error() {
        let driver = SignalReportDriver::new(Arc::new(StubSignalReportService::failing(
            SignalReportError::InvalidOccurrence(SignalReportChain::SpecAdr),
        )));

        let outcome = driver.handle(SignalReportInput {
            chain: SignalReportChainFilter::SpecAdr,
            levels: SignalReportLevelFilter::YellowAndRed,
        });

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("signal report: source returned an invalid occurrence for spec_adr")
        );
    }
}
