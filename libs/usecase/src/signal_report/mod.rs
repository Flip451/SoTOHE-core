//! Typed read-only boundary for signal-occurrence reports.

use std::fmt;
use std::sync::Arc;

use domain::NonEmptyString;
use domain::review_v2::types::FilePath;
use thiserror::Error;

/// A signal chain selectable by a report query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalReportChain {
    /// Chain ⓪: ADR decisions grounded by user evidence.
    AdrUser,
    /// Chain ①: specification references to ADRs.
    SpecAdr,
    /// Chain ②: catalogue references to the specification.
    CatalogSpec,
    /// Chain ③: implementation references to the catalogue.
    ImplCatalog,
}

/// A non-empty chain selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReportChainSelection {
    /// Include all report chains.
    All,
    /// Include one report chain.
    One(SignalReportChain),
}

/// A reportable non-blue signal level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalReportLevel {
    /// A yellow signal.
    Yellow,
    /// A red signal.
    Red,
}

/// A non-empty report-level selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignalReportLevelSelection {
    /// Include only yellow signals.
    YellowOnly,
    /// Include only red signals.
    RedOnly,
    /// Include both yellow and red signals.
    YellowAndRed,
}

/// A typed occurrence entry identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReportEntryId(NonEmptyString);

impl SignalReportEntryId {
    /// Creates a typed occurrence entry identifier.
    #[must_use]
    pub fn new(value: NonEmptyString) -> Self {
        Self(value)
    }
}

impl fmt::Display for SignalReportEntryId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A typed occurrence reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReportReference(NonEmptyString);

impl SignalReportReference {
    /// Creates a typed occurrence reference.
    #[must_use]
    pub fn new(value: NonEmptyString) -> Self {
        Self(value)
    }
}

impl fmt::Display for SignalReportReference {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A typed occurrence reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReportReason(NonEmptyString);

impl SignalReportReason {
    /// Creates a typed occurrence reason.
    #[must_use]
    pub fn new(value: NonEmptyString) -> Self {
        Self(value)
    }
}

impl fmt::Display for SignalReportReason {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// A typed repo-relative occurrence location.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReportLocation(FilePath);

impl SignalReportLocation {
    /// Creates a typed occurrence location.
    #[must_use]
    pub fn new(value: FilePath) -> Self {
        Self(value)
    }
}

impl fmt::Display for SignalReportLocation {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// Typed query parameters for a signal-occurrence report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalReportQuery {
    /// The chains to load.
    pub chain: SignalReportChainSelection,
    /// The signal levels to retain.
    pub levels: SignalReportLevelSelection,
}

/// One signal occurrence ready for presentation by an outer adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReportOccurrence {
    /// The chain that produced the occurrence.
    pub chain: SignalReportChain,
    /// The occurrence's non-blue signal level.
    pub level: SignalReportLevel,
    /// The affected entry identifier.
    pub entry_id: SignalReportEntryId,
    /// The reference evaluated for the entry.
    pub reference: SignalReportReference,
    /// The evaluation reason.
    pub reason: SignalReportReason,
    /// The affected repository location.
    pub location: SignalReportLocation,
}

/// Output from a completed signal-occurrence report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReportOutput {
    /// All occurrences matching the query.
    pub occurrences: Vec<SignalReportOccurrence>,
}

/// Errors reported by the signal-report boundary.
#[derive(Debug, Error)]
pub enum SignalReportError {
    /// The source could not load a selected chain.
    #[error("signal report source unavailable for {0:?}")]
    SourceUnavailable(SignalReportChain),
    /// The source returned an occurrence for a chain other than the requested one.
    #[error("signal report source returned an invalid occurrence for {0:?}")]
    InvalidOccurrence(SignalReportChain),
}

/// Application service for reading a typed signal-occurrence report.
pub trait SignalReportService: Send + Sync {
    /// Loads and filters occurrences for `query`.
    ///
    /// # Errors
    ///
    /// Returns [`SignalReportError`] when a selected source cannot provide valid
    /// occurrences.
    fn report(&self, query: SignalReportQuery) -> Result<SignalReportOutput, SignalReportError>;
}

/// Driven port that supplies occurrences for one signal chain.
pub trait SignalReportSourcePort: Send + Sync {
    /// Loads occurrences for one chain without persisting report data.
    ///
    /// # Errors
    ///
    /// Returns [`SignalReportError`] when the source is unavailable or cannot
    /// provide a valid occurrence for `chain`.
    fn load(
        &self,
        chain: SignalReportChain,
    ) -> Result<Vec<SignalReportOccurrence>, SignalReportError>;
}

/// Interactor implementing [`SignalReportService`] through an injected source.
pub struct SignalReportInteractor {
    source: Arc<dyn SignalReportSourcePort>,
}

impl SignalReportInteractor {
    /// Creates a signal-report interactor with its occurrence source.
    #[must_use]
    pub fn new(source: Arc<dyn SignalReportSourcePort>) -> Self {
        Self { source }
    }
}

impl SignalReportService for SignalReportInteractor {
    fn report(&self, query: SignalReportQuery) -> Result<SignalReportOutput, SignalReportError> {
        const ALL_CHAINS: [SignalReportChain; 4] = [
            SignalReportChain::AdrUser,
            SignalReportChain::SpecAdr,
            SignalReportChain::CatalogSpec,
            SignalReportChain::ImplCatalog,
        ];

        let chains = match query.chain {
            SignalReportChainSelection::All => ALL_CHAINS.to_vec(),
            SignalReportChainSelection::One(chain) => vec![chain],
        };
        let mut occurrences = Vec::new();

        for chain in chains {
            for occurrence in self.source.load(chain)? {
                if occurrence.chain != chain {
                    return Err(SignalReportError::InvalidOccurrence(chain));
                }
                let selected = match query.levels {
                    SignalReportLevelSelection::YellowOnly => {
                        occurrence.level == SignalReportLevel::Yellow
                    }
                    SignalReportLevelSelection::RedOnly => {
                        occurrence.level == SignalReportLevel::Red
                    }
                    SignalReportLevelSelection::YellowAndRed => true,
                };
                if selected {
                    occurrences.push(occurrence);
                }
            }
        }

        Ok(SignalReportOutput { occurrences })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct RecordingSource {
        occurrences: Vec<SignalReportOccurrence>,
        unavailable_chain: Option<SignalReportChain>,
        loaded_chains: Mutex<Vec<SignalReportChain>>,
    }

    impl RecordingSource {
        fn available(occurrences: Vec<SignalReportOccurrence>) -> Self {
            Self { occurrences, unavailable_chain: None, loaded_chains: Mutex::new(Vec::new()) }
        }

        fn unavailable(chain: SignalReportChain) -> Self {
            Self {
                occurrences: Vec::new(),
                unavailable_chain: Some(chain),
                loaded_chains: Mutex::new(Vec::new()),
            }
        }
    }

    impl SignalReportSourcePort for RecordingSource {
        fn load(
            &self,
            chain: SignalReportChain,
        ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
            self.loaded_chains.lock().expect("test mutex must not be poisoned").push(chain);
            if self.unavailable_chain == Some(chain) {
                return Err(SignalReportError::SourceUnavailable(chain));
            }
            Ok(self
                .occurrences
                .iter()
                .filter(|occurrence| occurrence.chain == chain)
                .cloned()
                .collect())
        }
    }

    fn occurrence(chain: SignalReportChain, level: SignalReportLevel) -> SignalReportOccurrence {
        SignalReportOccurrence {
            chain,
            level,
            entry_id: SignalReportEntryId::new(NonEmptyString::try_new("entry-id").unwrap()),
            reference: SignalReportReference::new(NonEmptyString::try_new("ref").unwrap()),
            reason: SignalReportReason::new(NonEmptyString::try_new("reason").unwrap()),
            location: SignalReportLocation::new(FilePath::new("track/items/test.json").unwrap()),
        }
    }

    #[test]
    fn test_report_all_chains_and_levels_returns_occurrences() {
        let source = Arc::new(RecordingSource::available(vec![
            occurrence(SignalReportChain::AdrUser, SignalReportLevel::Yellow),
            occurrence(SignalReportChain::SpecAdr, SignalReportLevel::Red),
            occurrence(SignalReportChain::CatalogSpec, SignalReportLevel::Yellow),
            occurrence(SignalReportChain::ImplCatalog, SignalReportLevel::Red),
        ]));
        let interactor =
            SignalReportInteractor::new(Arc::clone(&source) as Arc<dyn SignalReportSourcePort>);

        let output = interactor
            .report(SignalReportQuery {
                chain: SignalReportChainSelection::All,
                levels: SignalReportLevelSelection::YellowAndRed,
            })
            .unwrap();

        assert_eq!(output.occurrences.len(), 4);
        assert_eq!(
            *source.loaded_chains.lock().unwrap(),
            [
                SignalReportChain::AdrUser,
                SignalReportChain::SpecAdr,
                SignalReportChain::CatalogSpec,
                SignalReportChain::ImplCatalog,
            ]
        );
    }

    #[test]
    fn test_report_propagates_typed_query_selections() {
        let source = Arc::new(RecordingSource::available(vec![
            occurrence(SignalReportChain::SpecAdr, SignalReportLevel::Yellow),
            occurrence(SignalReportChain::SpecAdr, SignalReportLevel::Red),
        ]));
        let interactor =
            SignalReportInteractor::new(Arc::clone(&source) as Arc<dyn SignalReportSourcePort>);

        let output = interactor
            .report(SignalReportQuery {
                chain: SignalReportChainSelection::One(SignalReportChain::SpecAdr),
                levels: SignalReportLevelSelection::RedOnly,
            })
            .unwrap();

        assert_eq!(output.occurrences.len(), 1);
        assert!(matches!(
            output.occurrences.as_slice(),
            [SignalReportOccurrence {
                chain: SignalReportChain::SpecAdr,
                level: SignalReportLevel::Red,
                ..
            }]
        ));
        assert_eq!(*source.loaded_chains.lock().unwrap(), [SignalReportChain::SpecAdr]);
    }

    #[test]
    fn test_report_filters_each_signal_report_level() {
        let source = Arc::new(RecordingSource::available(vec![
            occurrence(SignalReportChain::SpecAdr, SignalReportLevel::Yellow),
            occurrence(SignalReportChain::SpecAdr, SignalReportLevel::Red),
        ]));
        let interactor =
            SignalReportInteractor::new(Arc::clone(&source) as Arc<dyn SignalReportSourcePort>);

        let yellow_output = interactor
            .report(SignalReportQuery {
                chain: SignalReportChainSelection::One(SignalReportChain::SpecAdr),
                levels: SignalReportLevelSelection::YellowOnly,
            })
            .unwrap();
        let red_output = interactor
            .report(SignalReportQuery {
                chain: SignalReportChainSelection::One(SignalReportChain::SpecAdr),
                levels: SignalReportLevelSelection::RedOnly,
            })
            .unwrap();

        assert!(matches!(
            yellow_output.occurrences.as_slice(),
            [SignalReportOccurrence { level: SignalReportLevel::Yellow, .. }]
        ));
        assert!(matches!(
            red_output.occurrences.as_slice(),
            [SignalReportOccurrence { level: SignalReportLevel::Red, .. }]
        ));
    }

    #[test]
    fn test_report_preserves_entry_id_and_reference_in_occurrence_output() {
        let mut expected = occurrence(SignalReportChain::CatalogSpec, SignalReportLevel::Yellow);
        expected.entry_id =
            SignalReportEntryId::new(NonEmptyString::try_new("catalogue-entry-42").unwrap());
        expected.reference = SignalReportReference::new(
            NonEmptyString::try_new("track/items/example/spec.json#IN-02").unwrap(),
        );
        let source = Arc::new(RecordingSource::available(vec![expected.clone()]));
        let interactor = SignalReportInteractor::new(source as Arc<dyn SignalReportSourcePort>);

        let output = interactor
            .report(SignalReportQuery {
                chain: SignalReportChainSelection::One(SignalReportChain::CatalogSpec),
                levels: SignalReportLevelSelection::YellowAndRed,
            })
            .unwrap();

        assert_eq!(output.occurrences, vec![expected]);
    }

    #[test]
    fn test_report_with_unavailable_source_returns_error() {
        let source = Arc::new(RecordingSource::unavailable(SignalReportChain::CatalogSpec));
        let interactor = SignalReportInteractor::new(source as Arc<dyn SignalReportSourcePort>);

        let error = interactor
            .report(SignalReportQuery {
                chain: SignalReportChainSelection::One(SignalReportChain::CatalogSpec),
                levels: SignalReportLevelSelection::YellowAndRed,
            })
            .unwrap_err();

        assert!(matches!(
            error,
            SignalReportError::SourceUnavailable(SignalReportChain::CatalogSpec)
        ));
    }

    #[test]
    fn test_report_with_mismatched_occurrence_chain_returns_error() {
        struct MismatchedSource;

        impl SignalReportSourcePort for MismatchedSource {
            fn load(
                &self,
                _chain: SignalReportChain,
            ) -> Result<Vec<SignalReportOccurrence>, SignalReportError> {
                Ok(vec![occurrence(SignalReportChain::SpecAdr, SignalReportLevel::Yellow)])
            }
        }

        let interactor = SignalReportInteractor::new(Arc::new(MismatchedSource));
        let error = interactor
            .report(SignalReportQuery {
                chain: SignalReportChainSelection::One(SignalReportChain::AdrUser),
                levels: SignalReportLevelSelection::YellowAndRed,
            })
            .unwrap_err();

        assert!(matches!(error, SignalReportError::InvalidOccurrence(SignalReportChain::AdrUser)));
    }
}
