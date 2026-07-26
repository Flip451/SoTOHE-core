//! Error vocabulary for the test-obligation gate.
//!
//! [`TestObligationRulesLoadError`] is produced when
//! `.harness/config/test-obligation-rules.json` fails the fail-closed load-time
//! totality validation (IN-04 / CN-05 / AC-02). The remaining errors are the
//! use-case-level failures for the four CLI subcommands
//! ([`ObligationDeriveError`], [`ObligationCheckError`],
//! [`ObligationEvaluateError`], [`ObligationResultsError`]) and the port-level
//! errors they wrap ([`ArtifactCodecError`], [`TestSourceScanError`],
//! [`VerifyCacheError`], [`SemanticVerifierError`]) (IN-05 / IN-06 / IN-08 /
//! IN-09 / IN-11 / IN-12 / CN-08).

use thiserror::Error;

use crate::SpecDocumentLoadError;
use crate::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderError;
use crate::tddd::test_obligation::drift::{NonEmptyDrifts, NonEmptyEdgeVerdictRecords};
use crate::tddd::test_obligation::ids::{DiagnosticMessage, NonEmptyEdgeIds, RoleName};
use crate::tddd::test_obligation::verdict::{
    FulfillmentCacheLookupError, FulfillmentCacheReevaluationReason,
};

/// Error raised while loading and validating the test-obligation rules config.
///
/// Each role-scoped variant carries the [`RoleName`] it concerns so the CLI can
/// report exactly which config entry failed (IN-04 / CN-05 / AC-02).
#[derive(Debug)]
pub enum TestObligationRulesLoadError {
    /// A role enum variant has no entry in the config (totality violation).
    RoleNotCovered {
        /// The role that the config failed to declare.
        role_name: RoleName,
    },
    /// A role entry omitted the `obligations` field instead of declaring `[]`.
    ObligationsFieldOmitted {
        /// The role whose entry omitted `obligations`.
        role_name: RoleName,
    },
    /// The config declared a key that does not resolve to a known role.
    UnknownRoleName {
        /// The unrecognised role key found in the config.
        role_name: RoleName,
    },
    /// A rule value was syntactically present but semantically invalid.
    InvalidRuleValue {
        /// The role whose rule failed validation.
        role_name: RoleName,
        /// Human-readable detail describing the invalid value.
        message: DiagnosticMessage,
    },
    /// The config file could not be read.
    IoError(DiagnosticMessage),
    /// The config file was not valid JSON for the expected schema.
    MalformedJson(DiagnosticMessage),
}

/// Error from an artifact codec loading or persisting an obligations /
/// test-bindings document (IN-05 / IN-06).
#[derive(Debug, Error)]
pub enum ArtifactCodecError {
    /// The artifact file could not be read or written.
    #[error("artifact I/O error: {}", .0.as_str())]
    Io(DiagnosticMessage),
    /// The artifact file was not valid JSON for the expected schema.
    #[error("malformed artifact JSON: {}", .0.as_str())]
    MalformedJson(DiagnosticMessage),
    /// A decoded artifact value violated a domain invariant.
    #[error("artifact violates a domain invariant: {}", .0.as_str())]
    DomainInvariant(DiagnosticMessage),
}

/// Error from scanning a test's source body from the worktree (IN-06 / IN-09).
#[derive(Debug, Error)]
pub enum TestSourceScanError {
    /// The source file could not be read.
    #[error("test source I/O error: {}", .0.as_str())]
    Io(DiagnosticMessage),
    /// The source file could not be parsed to locate the test body.
    #[error("test source parse error: {}", .0.as_str())]
    Parse(DiagnosticMessage),
}

/// Error from loading or persisting a verdict cache document (IN-09 / CN-04).
#[derive(Debug, Error)]
pub enum VerifyCacheError {
    /// The cache file could not be read or written.
    #[error("verdict cache I/O error: {}", .0.as_str())]
    Io(DiagnosticMessage),
    /// The cache file was not valid JSON for the expected schema.
    #[error("malformed verdict cache JSON: {}", .0.as_str())]
    MalformedJson(DiagnosticMessage),
}

/// Error surfaced by a semantic-verifier port invocation (IN-11 / CN-08).
#[derive(Debug, Error)]
pub enum SemanticVerifierError {
    /// The underlying verifier provider failed to return a verdict.
    #[error("semantic verifier port error: {}", .0.as_str())]
    VerifierPort(DiagnosticMessage),
}

/// Failure of `test-obligation derive` (IN-05 / IN-07 / AC-03).
#[derive(Debug)]
pub enum ObligationDeriveError {
    /// The decision-table config failed load-time validation.
    RulesLoad(TestObligationRulesLoadError),
    /// The command ran off a non-active track branch.
    TrackNotActive {
        /// The branch the command was invoked on.
        branch: DiagnosticMessage,
    },
    /// Loading a per-layer catalogue document failed.
    CatalogueLoad(CatalogueDocumentLoaderError),
    /// Loading and parsing the spec document failed.
    SpecLoad(SpecDocumentLoadError),
    /// The catalogue was in a state derivation cannot proceed from.
    InvalidCatalogueState(DiagnosticMessage),
    /// Encoding the derived obligations artifact failed.
    ArtifactCodec(ArtifactCodecError),
    /// Writing the derived obligations artifact failed.
    ArtifactWrite(DiagnosticMessage),
}

/// Failure of `test-obligation check` (IN-08 / AC-04 / AC-10).
#[derive(Debug)]
pub enum ObligationCheckError {
    /// The decision-table config failed load-time validation. `check` loads and
    /// validates the rules document up front so a malformed or role-incomplete
    /// `.harness/config/test-obligation-rules.json` fails closed instead of
    /// silently passing on stale obligations / bindings / caches (IN-08).
    RulesLoad(TestObligationRulesLoadError),
    /// The obligations artifact is absent while the test-bindings artifact is
    /// present (half-materialised scope, fail-closed — AC-10).
    ObligationsAbsent,
    /// The test-bindings artifact is absent while the obligations artifact is
    /// present (half-materialised scope, fail-closed — AC-10).
    BindingsAbsent,
    /// The persisted obligations artifact differs from the current in-memory
    /// derivation and must be regenerated with `test-obligation derive`.
    StaleObligationsArtifact {
        /// Counts for missing, unexpected, and changed obligations.
        detail: DiagnosticMessage,
    },
    /// One or more drifts were detected.
    DriftsDetected {
        /// The detected drifts (non-empty by construction).
        drifts: NonEmptyDrifts,
    },
    /// One or more edges were not resolved by any binding.
    UnresolvedEdges {
        /// The unresolved edges (non-empty by construction).
        edges: NonEmptyEdgeIds,
    },
    /// One or more edges have a stale (hash-mismatched) verdict.
    StaleVerdicts {
        /// The edges whose verdicts are stale (non-empty by construction).
        edges: NonEmptyEdgeIds,
    },
    /// Loading a per-layer catalogue document failed.
    CatalogueLoad(CatalogueDocumentLoaderError),
    /// Loading and parsing the spec document failed.
    SpecLoad(SpecDocumentLoadError),
    /// The catalogue was in a state the check gate cannot proceed from.
    InvalidCatalogueState(DiagnosticMessage),
    /// Decoding an artifact failed.
    ArtifactCodec(ArtifactCodecError),
    /// Scanning a bound test's source failed.
    SourceScan(TestSourceScanError),
    /// Reading a verdict cache failed.
    CacheIo(VerifyCacheError),
    /// A derived obligation or cited edge could not be attributed to a task
    /// status through the track's task-contract and implementation plan.
    TaskAttribution(DiagnosticMessage),
    /// Resolving the current fulfillment-cache entry was ambiguous.
    FulfillmentCacheLookup(FulfillmentCacheLookupError),
    /// A binding violates the derived-obligation ownership invariant.
    BindingConsistency(TestBindingConsistencyError),
    /// The fulfillment cache must be reevaluated before it can resolve an edge.
    FulfillmentCacheRequiresEvaluation(FulfillmentCacheReevaluationReason),
}

/// Failure of `test-obligation evaluate` (IN-09 / IN-12 / AC-06 / AC-07).
#[derive(Debug)]
pub enum ObligationEvaluateError {
    /// The command ran off a non-active track branch.
    TrackNotActive {
        /// The branch the command was invoked on.
        branch: DiagnosticMessage,
    },
    /// Loading a per-layer catalogue document failed.
    CatalogueLoad(CatalogueDocumentLoaderError),
    /// Loading and parsing the spec document failed.
    SpecLoad(SpecDocumentLoadError),
    /// Loading an obligations / test-bindings artifact failed.
    ArtifactLoad(ArtifactCodecError),
    /// Scanning a bound test's source failed.
    TestSourceScan(TestSourceScanError),
    /// A semantic-verifier port invocation failed.
    VerifierPort(SemanticVerifierError),
    /// Loading or persisting a verdict cache failed.
    CachePersistence(VerifyCacheError),
    /// Evaluation confirmed one or more semantic failures.
    SemanticFailuresConfirmed {
        /// The per-edge records that failed (non-empty by construction).
        records: NonEmptyEdgeVerdictRecords,
    },
    /// Evaluation could not confirm and requires human escalation.
    HumanEscalationRequired {
        /// The per-edge records requiring escalation (non-empty by construction).
        records: NonEmptyEdgeVerdictRecords,
    },
}

/// Inconsistent test-binding data that cannot satisfy the obligation gate.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TestBindingConsistencyError {
    /// A voluntary binding was recorded for an edge with a derived obligation.
    #[error("voluntary binding owns derived obligation edge {edge_id:?}")]
    VoluntaryBindingOwnsDerivedObligation {
        /// The edge that already owns a derived obligation.
        edge_id: crate::tddd::test_obligation::ids::TestObligationEdgeId,
    },
}

/// Failure of `test-obligation results` (IN-10 / AC-09).
#[derive(Debug)]
pub enum ObligationResultsError {
    /// A results artifact could not be read.
    IoError(DiagnosticMessage),
    /// A results artifact was malformed.
    MalformedArtifact(DiagnosticMessage),
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_role_not_covered_carries_role_name() {
        let err = TestObligationRulesLoadError::RoleNotCovered {
            role_name: RoleName::try_new("ValueObject".to_owned()).unwrap(),
        };
        match err {
            TestObligationRulesLoadError::RoleNotCovered { role_name } => {
                assert_eq!(role_name.as_str(), "ValueObject");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_invalid_rule_value_carries_message() {
        let err = TestObligationRulesLoadError::InvalidRuleValue {
            role_name: RoleName::try_new("UseCase".to_owned()).unwrap(),
            message: DiagnosticMessage::try_new("unknown kind 'frobnicate'".to_owned()).unwrap(),
        };
        match err {
            TestObligationRulesLoadError::InvalidRuleValue { role_name, message } => {
                assert_eq!(role_name.as_str(), "UseCase");
                assert_eq!(message.as_str(), "unknown kind 'frobnicate'");
            }
            other => panic!("unexpected variant: {other:?}"),
        }
    }

    #[test]
    fn test_malformed_json_is_debuggable() {
        let err = TestObligationRulesLoadError::MalformedJson(
            DiagnosticMessage::try_new("expected object".to_owned()).unwrap(),
        );
        assert!(format!("{err:?}").contains("MalformedJson"));
    }

    #[test]
    fn test_binding_consistency_error_carries_edge_id() {
        let edge_id = crate::tddd::test_obligation::ids::TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::Money".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-03".to_owned()).unwrap(),
        );
        let error = TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation {
            edge_id: edge_id.clone(),
        };

        assert!(matches!(
            error,
            TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation { edge_id: actual }
                if actual == edge_id
        ));
    }

    use std::path::PathBuf;

    use crate::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderError;
    use crate::tddd::semantic_verify::CatalogueEntryKey;
    use crate::tddd::test_obligation::drift::{
        EdgeResolutionOutcome, EdgeVerdictRecord, TestObligationDrift,
    };
    use crate::tddd::test_obligation::ids::{
        TestObligationAnchorId, TestObligationEdgeId, TestObligationId,
        TestObligationItemIdentifier,
    };
    use crate::tddd::test_obligation::vocab::TestObligationKind;

    fn diag(text: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(text.to_owned()).unwrap()
    }

    fn edge() -> TestObligationEdgeId {
        TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-08".to_owned()).unwrap(),
        )
    }

    fn obligation() -> TestObligationId {
        TestObligationId::new(
            CatalogueEntryKey::try_new("domain::User".to_owned()).unwrap(),
            TestObligationKind::Boundary,
            TestObligationItemIdentifier::try_new("invariant:non_empty".to_owned()).unwrap(),
        )
    }

    fn drift() -> TestObligationDrift {
        TestObligationDrift::orphaned_edge(edge(), diag("no obligation"))
    }

    fn record() -> EdgeVerdictRecord {
        EdgeVerdictRecord::new(None, edge(), None, None, EdgeResolutionOutcome::Pending, None, None)
    }

    fn catalogue_load_error() -> CatalogueDocumentLoaderError {
        CatalogueDocumentLoaderError::NotFound { path: PathBuf::from("missing-types.json") }
    }

    #[test]
    fn test_artifact_codec_error_variants_display() {
        assert!(ArtifactCodecError::Io(diag("read failed")).to_string().contains("read failed"));
        assert!(
            ArtifactCodecError::MalformedJson(diag("bad json")).to_string().contains("bad json")
        );
        assert!(
            ArtifactCodecError::DomainInvariant(diag("bad value"))
                .to_string()
                .contains("bad value")
        );
    }

    #[test]
    fn test_test_source_scan_error_variants_display() {
        assert!(TestSourceScanError::Io(diag("io")).to_string().contains("io"));
        assert!(TestSourceScanError::Parse(diag("syntax")).to_string().contains("syntax"));
    }

    #[test]
    fn test_verify_cache_error_variants_display() {
        assert!(VerifyCacheError::Io(diag("io")).to_string().contains("io"));
        assert!(VerifyCacheError::MalformedJson(diag("bad")).to_string().contains("bad"));
    }

    #[test]
    fn test_semantic_verifier_error_display() {
        assert!(
            SemanticVerifierError::VerifierPort(diag("timeout")).to_string().contains("timeout")
        );
    }

    #[test]
    fn test_obligation_derive_error_all_variants_debuggable() {
        let variants: Vec<ObligationDeriveError> = vec![
            ObligationDeriveError::RulesLoad(TestObligationRulesLoadError::RoleNotCovered {
                role_name: RoleName::try_new("ValueObject".to_owned()).unwrap(),
            }),
            ObligationDeriveError::TrackNotActive { branch: diag("main") },
            ObligationDeriveError::CatalogueLoad(catalogue_load_error()),
            ObligationDeriveError::SpecLoad(SpecDocumentLoadError::NotFound {
                path: PathBuf::from("spec.json"),
            }),
            ObligationDeriveError::InvalidCatalogueState(diag("baseline pending")),
            ObligationDeriveError::ArtifactCodec(ArtifactCodecError::Io(diag("io"))),
            ObligationDeriveError::ArtifactWrite(diag("write failed")),
        ];
        assert_eq!(variants.len(), 7);
        assert!(variants.iter().all(|v| !format!("{v:?}").is_empty()));
    }

    #[test]
    fn test_obligation_check_error_all_variants_debuggable() {
        // `SpecLoad(SpecDocumentLoadError)` is omitted: the error's field is
        // private to its own module and cannot be constructed here.
        let variants: Vec<ObligationCheckError> = vec![
            ObligationCheckError::RulesLoad(TestObligationRulesLoadError::RoleNotCovered {
                role_name: RoleName::try_new("ValueObject".to_owned()).unwrap(),
            }),
            ObligationCheckError::ObligationsAbsent,
            ObligationCheckError::BindingsAbsent,
            ObligationCheckError::StaleObligationsArtifact {
                detail: diag("missing=1, unexpected=0, changed=0"),
            },
            ObligationCheckError::DriftsDetected { drifts: NonEmptyDrifts::new(drift(), vec![]) },
            ObligationCheckError::UnresolvedEdges { edges: NonEmptyEdgeIds::new(edge(), vec![]) },
            ObligationCheckError::StaleVerdicts { edges: NonEmptyEdgeIds::new(edge(), vec![]) },
            ObligationCheckError::CatalogueLoad(catalogue_load_error()),
            ObligationCheckError::InvalidCatalogueState(diag("empty spec ref")),
            ObligationCheckError::ArtifactCodec(ArtifactCodecError::MalformedJson(diag("bad"))),
            ObligationCheckError::SourceScan(TestSourceScanError::Io(diag("io"))),
            ObligationCheckError::CacheIo(VerifyCacheError::Io(diag("io"))),
            ObligationCheckError::TaskAttribution(diag("entry has no task attribution")),
        ];
        assert_eq!(variants.len(), 13);
        assert!(variants.iter().all(|v| !format!("{v:?}").is_empty()));
    }

    #[test]
    fn test_obligation_evaluate_error_all_variants_debuggable() {
        // `SpecLoad(SpecDocumentLoadError)` is omitted: the error's field is
        // private to its own module and cannot be constructed here.
        let variants: Vec<ObligationEvaluateError> = vec![
            ObligationEvaluateError::TrackNotActive { branch: diag("main") },
            ObligationEvaluateError::CatalogueLoad(catalogue_load_error()),
            ObligationEvaluateError::ArtifactLoad(ArtifactCodecError::Io(diag("io"))),
            ObligationEvaluateError::TestSourceScan(TestSourceScanError::Io(diag("io"))),
            ObligationEvaluateError::VerifierPort(SemanticVerifierError::VerifierPort(diag("x"))),
            ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(diag("io"))),
            ObligationEvaluateError::SemanticFailuresConfirmed {
                records: NonEmptyEdgeVerdictRecords::new(record(), vec![]),
            },
            ObligationEvaluateError::HumanEscalationRequired {
                records: NonEmptyEdgeVerdictRecords::new(record(), vec![]),
            },
        ];
        assert_eq!(variants.len(), 8);
        assert!(variants.iter().all(|v| !format!("{v:?}").is_empty()));
    }

    #[test]
    fn test_obligation_results_error_all_variants_debuggable() {
        let io = ObligationResultsError::IoError(diag("read failed"));
        let malformed = ObligationResultsError::MalformedArtifact(diag("bad artifact"));
        assert!(matches!(io, ObligationResultsError::IoError(_)));
        assert!(matches!(malformed, ObligationResultsError::MalformedArtifact(_)));
        assert!(obligation().entry_key().as_str().contains("User"));
    }

    #[test]
    fn test_check_error_carries_fulfillment_cache_reevaluation_reason() {
        let error = ObligationCheckError::FulfillmentCacheRequiresEvaluation(
            FulfillmentCacheReevaluationReason::LegacyRowsMissingBoundTests,
        );

        assert!(matches!(
            error,
            ObligationCheckError::FulfillmentCacheRequiresEvaluation(
                FulfillmentCacheReevaluationReason::LegacyRowsMissingBoundTests
            )
        ));
    }

    #[test]
    fn test_check_error_carries_binding_consistency_error() {
        let edge_id = crate::tddd::test_obligation::ids::TestObligationEdgeId::new(
            CatalogueEntryKey::try_new("domain::Money".to_owned()).unwrap(),
            TestObligationAnchorId::try_new("spec.json".to_owned(), "IN-03".to_owned()).unwrap(),
        );
        let error = ObligationCheckError::BindingConsistency(
            TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation {
                edge_id: edge_id.clone(),
            },
        );

        assert!(matches!(
            error,
            ObligationCheckError::BindingConsistency(
                TestBindingConsistencyError::VoluntaryBindingOwnsDerivedObligation { edge_id: actual }
            ) if actual == edge_id
        ));
    }
}
