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

use crate::tddd::test_obligation::drift::{EdgeVerdictRecord, TestObligationDrift};
use crate::tddd::test_obligation::ids::{DiagnosticMessage, RoleName, TestObligationEdgeId};

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
    /// The catalogue could not be loaded.
    CatalogueLoad(DiagnosticMessage),
    /// The spec could not be loaded.
    SpecLoad(DiagnosticMessage),
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
    /// Only the obligations artifact is present (partial, fail-closed scope).
    ObligationsOnly,
    /// Only the test-bindings artifact is present (partial, fail-closed scope).
    BindingsOnly,
    /// The command ran off a non-active track branch.
    TrackNotActive {
        /// The branch the command was invoked on.
        branch: DiagnosticMessage,
    },
    /// One or more drifts were detected.
    DriftsDetected {
        /// The detected drifts.
        drifts: Vec<TestObligationDrift>,
    },
    /// One or more edges were not resolved by any binding.
    UnresolvedEdges {
        /// The unresolved edges.
        edges: Vec<TestObligationEdgeId>,
    },
    /// One or more edges have a stale (hash-mismatched) verdict.
    StaleVerdicts {
        /// The edges whose verdicts are stale.
        edges: Vec<TestObligationEdgeId>,
    },
    /// Decoding an artifact failed.
    ArtifactCodec(ArtifactCodecError),
    /// Scanning a bound test's source failed.
    SourceScan(TestSourceScanError),
    /// Reading a verdict cache failed.
    CacheIo(VerifyCacheError),
}

/// Failure of `test-obligation evaluate` (IN-09 / IN-12 / AC-06 / AC-07).
#[derive(Debug)]
pub enum ObligationEvaluateError {
    /// The evaluate configuration was invalid.
    InvalidConfig {
        /// Detail describing the invalid configuration.
        message: DiagnosticMessage,
    },
    /// The command ran off a non-active track branch.
    TrackNotActive {
        /// The branch the command was invoked on.
        branch: DiagnosticMessage,
    },
    /// A semantic-verifier port invocation failed.
    VerifierPort(SemanticVerifierError),
    /// Loading a verdict cache failed.
    CachePersistence(VerifyCacheError),
    /// Writing a verdict cache failed.
    CacheWrite(DiagnosticMessage),
    /// Evaluation confirmed one or more semantic failures.
    SemanticFailuresConfirmed {
        /// The per-edge records that failed.
        records: Vec<EdgeVerdictRecord>,
    },
    /// Evaluation could not confirm and requires human escalation.
    HumanEscalationRequired {
        /// The per-edge records requiring escalation.
        records: Vec<EdgeVerdictRecord>,
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

    use crate::tddd::semantic_verify::CatalogueEntryKey;
    use crate::tddd::test_obligation::drift::EdgeResolutionOutcome;
    use crate::tddd::test_obligation::ids::{
        TestObligationAnchorId, TestObligationId, TestObligationItemIdentifier,
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
        EdgeVerdictRecord::new(edge(), EdgeResolutionOutcome::Pending, None)
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
            ObligationDeriveError::CatalogueLoad(diag("no catalogue")),
            ObligationDeriveError::SpecLoad(diag("no spec")),
            ObligationDeriveError::InvalidCatalogueState(diag("baseline pending")),
            ObligationDeriveError::ArtifactCodec(ArtifactCodecError::Io(diag("io"))),
            ObligationDeriveError::ArtifactWrite(diag("write failed")),
        ];
        assert_eq!(variants.len(), 7);
        assert!(variants.iter().all(|v| !format!("{v:?}").is_empty()));
    }

    #[test]
    fn test_obligation_check_error_all_variants_debuggable() {
        let variants: Vec<ObligationCheckError> = vec![
            ObligationCheckError::ObligationsOnly,
            ObligationCheckError::BindingsOnly,
            ObligationCheckError::TrackNotActive { branch: diag("main") },
            ObligationCheckError::DriftsDetected { drifts: vec![drift()] },
            ObligationCheckError::UnresolvedEdges { edges: vec![edge()] },
            ObligationCheckError::StaleVerdicts { edges: vec![edge()] },
            ObligationCheckError::ArtifactCodec(ArtifactCodecError::MalformedJson(diag("bad"))),
            ObligationCheckError::SourceScan(TestSourceScanError::Io(diag("io"))),
            ObligationCheckError::CacheIo(VerifyCacheError::Io(diag("io"))),
        ];
        assert_eq!(variants.len(), 9);
        assert!(variants.iter().all(|v| !format!("{v:?}").is_empty()));
    }

    #[test]
    fn test_obligation_evaluate_error_all_variants_debuggable() {
        let variants: Vec<ObligationEvaluateError> = vec![
            ObligationEvaluateError::InvalidConfig { message: diag("bad tier") },
            ObligationEvaluateError::TrackNotActive { branch: diag("main") },
            ObligationEvaluateError::VerifierPort(SemanticVerifierError::VerifierPort(diag("x"))),
            ObligationEvaluateError::CachePersistence(VerifyCacheError::Io(diag("io"))),
            ObligationEvaluateError::CacheWrite(diag("write failed")),
            ObligationEvaluateError::SemanticFailuresConfirmed { records: vec![record()] },
            ObligationEvaluateError::HumanEscalationRequired { records: vec![record()] },
        ];
        assert_eq!(variants.len(), 7);
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
}
