//! Policy vocabulary and loading boundary for scope-aware pre-review gates.
//!
//! This module owns the validated review-scope-to-gate matrix. File decoding
//! belongs to an infrastructure adapter implementing
//! [`PreReviewGateConfigLoaderPort`].

use std::collections::{HashMap, HashSet};
use std::path::Path;

use domain::review_v2::ScopeName;
use domain::{FreeText, TrackId};
use thiserror::Error;

/// Application-workflow vocabulary of pre-review gates that may be assigned
/// to a resolved review scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PreReviewGateKind {
    /// Verifies task-contract liveness against implementation-catalogue signals.
    TaskContractLiveness,
}

/// Construction failure while validating a total, duplicate-free review-scope
/// applicability matrix.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PreReviewGateMatrixError {
    /// A configured review scope has no matrix entry.
    #[error("pre-review gate matrix is missing scope '{0}'")]
    MissingScope(ScopeName),
    /// A matrix entry refers to a scope not configured for review.
    #[error("pre-review gate matrix contains unknown scope '{0}'")]
    UnknownScope(ScopeName),
    /// A matrix has more than one entry for the same scope.
    #[error("pre-review gate matrix contains duplicate scope '{0}'")]
    DuplicateScope(ScopeName),
    /// A scope assigns the same gate more than once.
    #[error("pre-review gate matrix contains duplicate gate '{gate:?}' for scope '{scope}'")]
    DuplicateGate {
        /// Scope with the repeated gate assignment.
        scope: ScopeName,
        /// Gate assigned more than once to `scope`.
        gate: PreReviewGateKind,
    },
}

/// Lookup failure when a requested review scope is absent from the validated
/// applicability matrix.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PreReviewGateLookupError {
    /// The requested scope is not configured in the matrix.
    #[error("pre-review gate matrix contains no entry for scope '{0}'")]
    UnknownScope(ScopeName),
}

/// Application-workflow value that validates a total mapping from every
/// configured review scope to its applicable pre-review gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreReviewGateMatrix {
    gates_by_scope: HashMap<ScopeName, Vec<PreReviewGateKind>>,
}

impl PreReviewGateMatrix {
    /// Validates and constructs a total, duplicate-free applicability matrix.
    ///
    /// Every member of `known_scopes` must have exactly one entry in `entries`.
    /// Entries may intentionally contain an empty gate list.
    ///
    /// # Errors
    ///
    /// Returns [`PreReviewGateMatrixError`] if an entry names an unknown scope,
    /// repeats a scope or gate, or omits a known scope.
    pub fn try_new(
        known_scopes: HashSet<ScopeName>,
        entries: Vec<(ScopeName, Vec<PreReviewGateKind>)>,
    ) -> Result<Self, PreReviewGateMatrixError> {
        let mut gates_by_scope = HashMap::with_capacity(entries.len());

        for (scope, gates) in entries {
            if !known_scopes.contains(&scope) {
                return Err(PreReviewGateMatrixError::UnknownScope(scope));
            }

            if gates_by_scope.contains_key(&scope) {
                return Err(PreReviewGateMatrixError::DuplicateScope(scope));
            }

            let mut assigned_gates = HashSet::with_capacity(gates.len());
            for gate in &gates {
                if !assigned_gates.insert(*gate) {
                    return Err(PreReviewGateMatrixError::DuplicateGate {
                        scope: scope.clone(),
                        gate: *gate,
                    });
                }
            }

            gates_by_scope.insert(scope, gates);
        }

        if let Some(missing_scope) =
            known_scopes.into_iter().find(|scope| !gates_by_scope.contains_key(scope))
        {
            return Err(PreReviewGateMatrixError::MissingScope(missing_scope));
        }

        Ok(Self { gates_by_scope })
    }

    /// Returns the gates for a configured scope.
    ///
    /// # Errors
    ///
    /// Returns [`PreReviewGateLookupError::UnknownScope`] when `scope` has no
    /// matrix entry.
    pub fn gates_for(
        &self,
        scope: &ScopeName,
    ) -> Result<&[PreReviewGateKind], PreReviewGateLookupError> {
        self.gates_by_scope
            .get(scope)
            .map(Vec::as_slice)
            .ok_or_else(|| PreReviewGateLookupError::UnknownScope(scope.clone()))
    }
}

/// Failure to read or validate the declarative pre-review-gate applicability
/// matrix.
#[derive(Debug, Error)]
pub enum PreReviewGateConfigLoadError {
    /// Loading or decoding the configuration failed.
    #[error("pre-review gate configuration load failed: {message}")]
    ReadFailed {
        /// Opaque adapter diagnostic text.
        message: FreeText,
    },
    /// The decoded configuration violates matrix invariants.
    #[error("pre-review gate configuration contains an invalid matrix: {0}")]
    InvalidMatrix(PreReviewGateMatrixError),
}

/// Synchronous secondary port that loads and validates the declared review-scope
/// to pre-review-gate applicability matrix before local-review dispatch.
///
/// Synchronous I/O is deliberate: local review reads one small local config file
/// and does not require an async runtime.
pub trait PreReviewGateConfigLoaderPort: Send + Sync {
    /// Loads the applicability matrix for `track_id` from the track items area.
    ///
    /// # Errors
    ///
    /// Returns [`PreReviewGateConfigLoadError`] on read, decode, or matrix
    /// validation failure.
    fn load(
        &self,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> Result<PreReviewGateMatrix, PreReviewGateConfigLoadError>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::HashSet;
    use std::path::Path;

    use domain::TrackId;
    use domain::review_v2::{MainScopeName, ScopeName};

    use super::{
        PreReviewGateConfigLoaderPort, PreReviewGateKind, PreReviewGateLookupError,
        PreReviewGateMatrix, PreReviewGateMatrixError,
    };

    fn scope(name: &str) -> ScopeName {
        ScopeName::Main(MainScopeName::new(name).unwrap())
    }

    fn known_scopes(names: &[&str]) -> HashSet<ScopeName> {
        names.iter().map(|name| scope(name)).collect()
    }

    struct ValidatingConfigLoader;

    impl PreReviewGateConfigLoaderPort for ValidatingConfigLoader {
        fn load(
            &self,
            _items_dir: &Path,
            _track_id: &TrackId,
        ) -> Result<PreReviewGateMatrix, super::PreReviewGateConfigLoadError> {
            PreReviewGateMatrix::try_new(
                known_scopes(&["spec", "types", "implementation"]),
                vec![
                    (scope("spec"), vec![]),
                    (scope("types"), vec![]),
                    (scope("implementation"), vec![PreReviewGateKind::TaskContractLiveness]),
                ],
            )
            .map_err(super::PreReviewGateConfigLoadError::InvalidMatrix)
        }
    }

    #[test]
    fn test_pre_review_gate_config_loader_port_validated_matrix_returns_scope_policy() {
        let loader = ValidatingConfigLoader;
        let track_id = TrackId::try_new("scope-policy-test").unwrap();

        let matrix = loader.load(Path::new("track/items"), &track_id).unwrap();

        assert_eq!(matrix.gates_for(&scope("spec")).unwrap(), []);
        assert_eq!(matrix.gates_for(&scope("types")).unwrap(), []);
        assert_eq!(
            matrix.gates_for(&scope("implementation")).unwrap(),
            [PreReviewGateKind::TaskContractLiveness]
        );
    }

    #[test]
    fn test_pre_review_gate_matrix_planning_scopes_return_no_liveness_gate() {
        let matrix = PreReviewGateMatrix::try_new(
            known_scopes(&["spec", "types", "implementation"]),
            vec![
                (scope("spec"), vec![]),
                (scope("types"), vec![]),
                (scope("implementation"), vec![PreReviewGateKind::TaskContractLiveness]),
            ],
        )
        .unwrap();

        assert_eq!(matrix.gates_for(&scope("spec")).unwrap(), []);
        assert_eq!(matrix.gates_for(&scope("types")).unwrap(), []);
        assert_eq!(
            matrix.gates_for(&scope("implementation")).unwrap(),
            [PreReviewGateKind::TaskContractLiveness]
        );
    }

    #[test]
    fn test_pre_review_gate_matrix_arbitrary_scopes_returns_declared_gate_vectors() {
        let matrix = PreReviewGateMatrix::try_new(
            known_scopes(&["alpha", "beta", "gamma"]),
            vec![
                (scope("alpha"), vec![]),
                (scope("beta"), vec![PreReviewGateKind::TaskContractLiveness]),
                (scope("gamma"), vec![]),
            ],
        )
        .unwrap();

        assert_eq!(matrix.gates_for(&scope("alpha")).unwrap(), []);
        assert_eq!(
            matrix.gates_for(&scope("beta")).unwrap(),
            [PreReviewGateKind::TaskContractLiveness]
        );
        assert_eq!(matrix.gates_for(&scope("gamma")).unwrap(), []);
    }

    #[test]
    fn test_pre_review_gate_matrix_missing_types_scope_rejects_incomplete_declaration() {
        let error = PreReviewGateMatrix::try_new(
            known_scopes(&["spec", "types", "implementation"]),
            vec![
                (scope("spec"), vec![]),
                (scope("implementation"), vec![PreReviewGateKind::TaskContractLiveness]),
            ],
        )
        .unwrap_err();

        assert_eq!(error, PreReviewGateMatrixError::MissingScope(scope("types")));
    }

    #[test]
    fn test_pre_review_gate_matrix_valid_entries_returns_configured_gates() {
        let matrix = PreReviewGateMatrix::try_new(
            known_scopes(&["spec", "implementation"]),
            vec![
                (scope("spec"), vec![]),
                (scope("implementation"), vec![PreReviewGateKind::TaskContractLiveness]),
            ],
        )
        .unwrap();

        assert_eq!(matrix.gates_for(&scope("spec")).unwrap(), []);
        assert_eq!(
            matrix.gates_for(&scope("implementation")).unwrap(),
            [PreReviewGateKind::TaskContractLiveness]
        );
    }

    #[test]
    fn test_pre_review_gate_matrix_missing_scope_returns_missing_scope_error() {
        let error = PreReviewGateMatrix::try_new(known_scopes(&["spec"]), Vec::new()).unwrap_err();

        assert_eq!(error, PreReviewGateMatrixError::MissingScope(scope("spec")));
    }

    #[test]
    fn test_pre_review_gate_matrix_unknown_scope_returns_unknown_scope_error() {
        let error = PreReviewGateMatrix::try_new(
            known_scopes(&["spec"]),
            vec![(scope("implementation"), vec![])],
        )
        .unwrap_err();

        assert_eq!(error, PreReviewGateMatrixError::UnknownScope(scope("implementation")));
    }

    #[test]
    fn test_pre_review_gate_matrix_arbitrary_unknown_scope_returns_matrix_error() {
        let unknown_scope = scope("gamma");
        let error = PreReviewGateMatrix::try_new(
            known_scopes(&["alpha", "beta"]),
            vec![(unknown_scope.clone(), vec![])],
        )
        .unwrap_err();

        assert_eq!(error, PreReviewGateMatrixError::UnknownScope(unknown_scope));
    }

    #[test]
    fn test_pre_review_gate_matrix_repeated_scope_returns_duplicate_scope_error() {
        let error = PreReviewGateMatrix::try_new(
            known_scopes(&["spec"]),
            vec![(scope("spec"), vec![]), (scope("spec"), vec![])],
        )
        .unwrap_err();

        assert_eq!(error, PreReviewGateMatrixError::DuplicateScope(scope("spec")));
    }

    #[test]
    fn test_pre_review_gate_matrix_repeated_gate_returns_duplicate_gate_error() {
        let error = PreReviewGateMatrix::try_new(
            known_scopes(&["implementation"]),
            vec![(
                scope("implementation"),
                vec![
                    PreReviewGateKind::TaskContractLiveness,
                    PreReviewGateKind::TaskContractLiveness,
                ],
            )],
        )
        .unwrap_err();

        assert_eq!(
            error,
            PreReviewGateMatrixError::DuplicateGate {
                scope: scope("implementation"),
                gate: PreReviewGateKind::TaskContractLiveness,
            }
        );
    }

    #[test]
    fn test_pre_review_gate_matrix_unconfigured_scope_returns_lookup_error() {
        let matrix =
            PreReviewGateMatrix::try_new(known_scopes(&["spec"]), vec![(scope("spec"), vec![])])
                .unwrap();

        assert_eq!(
            matrix.gates_for(&scope("implementation")).unwrap_err(),
            PreReviewGateLookupError::UnknownScope(scope("implementation"))
        );
    }
}
