//! Policy vocabulary and loading boundary for scope-aware pre-review gates.
//!
//! This module owns the validated review-scope-to-gate matrix. File decoding
//! belongs to an infrastructure adapter implementing
//! [`PreReviewGateConfigLoaderPort`].

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use domain::review_v2::ScopeName;
use domain::task_contract::PreReviewGateOutcome;
use domain::{FreeText, TrackId};
use thiserror::Error;

use crate::pre_review_gate::{PreReviewGateCommand, PreReviewGateError, PreReviewGateService};

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

/// Command carrying the resolved review scope and track location for
/// pre-review-gate dispatch.
#[derive(Debug, Clone)]
pub struct PreReviewGateDispatchCommand {
    /// Active track whose pre-review gate policy is loaded.
    pub track_id: TrackId,
    /// Scope resolved by local review before dispatch begins.
    pub scope: ScopeName,
    /// Directory containing the active track's items.
    pub items_dir: PathBuf,
}

/// Dispatch result that distinguishes a non-applicable gate from an executed
/// task-contract liveness outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreReviewGateDispatchOutcome {
    /// No pre-review gate applies to the resolved scope.
    NotApplicable,
    /// The existing task-contract liveness gate was run unchanged.
    TaskContract(PreReviewGateOutcome),
}

/// Failure while loading applicability policy, resolving a configured scope,
/// or executing an applicable pre-review gate.
#[derive(Debug, Error)]
pub enum PreReviewGateDispatchError {
    /// The declared pre-review-gate matrix could not be loaded.
    #[error("pre-review gate configuration error: {0}")]
    Config(#[from] PreReviewGateConfigLoadError),
    /// The existing task-contract liveness gate could not execute.
    #[error("task-contract pre-review gate error: {0}")]
    TaskContract(#[from] PreReviewGateError),
    /// The resolved review scope has no matrix entry.
    #[error("pre-review gate scope lookup error: {0}")]
    Lookup(#[from] PreReviewGateLookupError),
}

/// Application service that resolves scope applicability and dispatches only
/// the configured pre-review gates.
pub trait PreReviewGateDispatchService: Send + Sync {
    /// Dispatch applicable pre-review gates for one already-resolved scope.
    ///
    /// # Errors
    ///
    /// Returns a typed loading, lookup, or task-contract execution error.
    fn dispatch(
        &self,
        cmd: PreReviewGateDispatchCommand,
    ) -> Result<PreReviewGateDispatchOutcome, PreReviewGateDispatchError>;
}

/// Interactor that combines matrix loading with the existing task-contract
/// liveness gate.
pub struct PreReviewGateDispatchInteractor {
    config_loader: Arc<dyn PreReviewGateConfigLoaderPort>,
    task_contract_gate: Arc<dyn PreReviewGateService>,
}

impl PreReviewGateDispatchInteractor {
    /// Constructs a dispatcher from its configuration and liveness-gate ports.
    #[must_use]
    pub fn new(
        config_loader: Arc<dyn PreReviewGateConfigLoaderPort>,
        task_contract_gate: Arc<dyn PreReviewGateService>,
    ) -> Self {
        Self { config_loader, task_contract_gate }
    }
}

impl PreReviewGateDispatchService for PreReviewGateDispatchInteractor {
    fn dispatch(
        &self,
        cmd: PreReviewGateDispatchCommand,
    ) -> Result<PreReviewGateDispatchOutcome, PreReviewGateDispatchError> {
        let PreReviewGateDispatchCommand { track_id, scope, items_dir } = cmd;
        let matrix = self.config_loader.load(&items_dir, &track_id)?;
        let applicable_gates = matrix.gates_for(&scope)?;

        let mut task_contract_outcome = None;
        for gate in applicable_gates {
            match gate {
                PreReviewGateKind::TaskContractLiveness => {
                    task_contract_outcome =
                        Some(self.task_contract_gate.check(PreReviewGateCommand {
                            track_id: track_id.clone(),
                            layer: None,
                        })?);
                }
            }
        }

        Ok(task_contract_outcome.map_or(
            PreReviewGateDispatchOutcome::NotApplicable,
            PreReviewGateDispatchOutcome::TaskContract,
        ))
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::HashSet;
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Mutex};

    use domain::review_v2::{MainScopeName, ScopeName};
    use domain::task_contract::{ContractedEntryRef, PreReviewGateViolation};
    use domain::tddd::LayerId;
    use domain::tddd::semantic_verify::CatalogueEntryKey;
    use domain::{ConfidenceSignal, FreeText, TrackId};

    use super::{
        PreReviewGateConfigLoadError, PreReviewGateConfigLoaderPort, PreReviewGateDispatchCommand,
        PreReviewGateDispatchError, PreReviewGateDispatchInteractor, PreReviewGateDispatchOutcome,
        PreReviewGateDispatchService, PreReviewGateKind, PreReviewGateLookupError,
        PreReviewGateMatrix, PreReviewGateMatrixError,
    };
    use crate::pre_review_gate::{PreReviewGateCommand, PreReviewGateError, PreReviewGateService};

    type ConfigCalls = Arc<Mutex<Vec<(PathBuf, TrackId)>>>;
    type TaskContractCalls = Arc<Mutex<Vec<PreReviewGateCommand>>>;
    type DispatchFixture = (PreReviewGateDispatchInteractor, ConfigCalls, TaskContractCalls);

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

    struct RecordingConfigLoader {
        response: Mutex<Option<Result<PreReviewGateMatrix, PreReviewGateConfigLoadError>>>,
        calls: ConfigCalls,
    }

    impl PreReviewGateConfigLoaderPort for RecordingConfigLoader {
        fn load(
            &self,
            items_dir: &Path,
            track_id: &TrackId,
        ) -> Result<PreReviewGateMatrix, PreReviewGateConfigLoadError> {
            self.calls.lock().unwrap().push((items_dir.to_path_buf(), track_id.clone()));
            self.response.lock().unwrap().take().expect("config loader response must be configured")
        }
    }

    struct RecordingTaskContractGate {
        response:
            Mutex<Option<Result<domain::task_contract::PreReviewGateOutcome, PreReviewGateError>>>,
        calls: TaskContractCalls,
    }

    impl PreReviewGateService for RecordingTaskContractGate {
        fn check(
            &self,
            cmd: PreReviewGateCommand,
        ) -> Result<domain::task_contract::PreReviewGateOutcome, PreReviewGateError> {
            self.calls.lock().unwrap().push(cmd);
            self.response
                .lock()
                .unwrap()
                .take()
                .expect("task-contract gate response must be configured")
        }
    }

    fn matrix(entries: Vec<(ScopeName, Vec<PreReviewGateKind>)>) -> PreReviewGateMatrix {
        let known_scopes = entries.iter().map(|(scope, _)| scope.clone()).collect();
        PreReviewGateMatrix::try_new(known_scopes, entries).unwrap()
    }

    fn dispatcher(
        config_result: Result<PreReviewGateMatrix, PreReviewGateConfigLoadError>,
        gate_result: Result<domain::task_contract::PreReviewGateOutcome, PreReviewGateError>,
    ) -> DispatchFixture {
        let config_calls = Arc::new(Mutex::new(Vec::new()));
        let gate_calls = Arc::new(Mutex::new(Vec::new()));
        let config_loader = Arc::new(RecordingConfigLoader {
            response: Mutex::new(Some(config_result)),
            calls: Arc::clone(&config_calls),
        });
        let task_contract_gate = Arc::new(RecordingTaskContractGate {
            response: Mutex::new(Some(gate_result)),
            calls: Arc::clone(&gate_calls),
        });

        (
            PreReviewGateDispatchInteractor::new(config_loader, task_contract_gate),
            config_calls,
            gate_calls,
        )
    }

    fn dispatch_command(scope_name: &str) -> PreReviewGateDispatchCommand {
        PreReviewGateDispatchCommand {
            track_id: TrackId::try_new("scope-policy-test").unwrap(),
            scope: scope(scope_name),
            items_dir: PathBuf::from("track/items"),
        }
    }

    fn non_blue_outcome() -> domain::task_contract::PreReviewGateOutcome {
        let entry = ContractedEntryRef::new(
            LayerId::try_new("usecase").unwrap(),
            CatalogueEntryKey::try_new("PreReviewGateDispatchInteractor".to_owned()).unwrap(),
        );
        domain::task_contract::PreReviewGateOutcome::blocked(vec![
            PreReviewGateViolation::NonBlueSignal { entry, signal: ConfidenceSignal::Yellow },
        ])
        .unwrap()
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

    #[test]
    fn test_pre_review_gate_dispatch_empty_scope_gates_returns_not_applicable_without_task_contract_call()
     {
        let (interactor, config_calls, gate_calls) = dispatcher(
            Ok(matrix(vec![(scope("planning"), vec![])])),
            Ok(domain::task_contract::PreReviewGateOutcome::Passed),
        );

        let result = interactor.dispatch(dispatch_command("planning")).unwrap();

        assert_eq!(result, PreReviewGateDispatchOutcome::NotApplicable);
        assert_eq!(
            config_calls.lock().unwrap().as_slice(),
            &[(PathBuf::from("track/items"), TrackId::try_new("scope-policy-test").unwrap())]
        );
        assert!(gate_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pre_review_gate_dispatch_applicable_task_contract_passed_forwards_all_layers_command() {
        let (interactor, _config_calls, gate_calls) = dispatcher(
            Ok(matrix(vec![(
                scope("implementation"),
                vec![PreReviewGateKind::TaskContractLiveness],
            )])),
            Ok(domain::task_contract::PreReviewGateOutcome::Passed),
        );

        let result = interactor.dispatch(dispatch_command("implementation")).unwrap();

        assert_eq!(
            result,
            PreReviewGateDispatchOutcome::TaskContract(
                domain::task_contract::PreReviewGateOutcome::Passed
            )
        );
        assert_eq!(
            gate_calls.lock().unwrap().as_slice(),
            &[PreReviewGateCommand {
                track_id: TrackId::try_new("scope-policy-test").unwrap(),
                layer: None,
            }]
        );
    }

    #[test]
    fn test_pre_review_gate_dispatch_applicable_task_contract_non_blue_blocked_preserves_outcome() {
        let blocked = non_blue_outcome();
        let (interactor, _config_calls, _gate_calls) = dispatcher(
            Ok(matrix(vec![(
                scope("implementation"),
                vec![PreReviewGateKind::TaskContractLiveness],
            )])),
            Ok(blocked.clone()),
        );

        let result = interactor.dispatch(dispatch_command("implementation")).unwrap();

        assert_eq!(result, PreReviewGateDispatchOutcome::TaskContract(blocked));
    }

    #[test]
    fn test_pre_review_gate_dispatch_config_load_failure_returns_config_error_without_task_contract_call()
     {
        let (interactor, _config_calls, gate_calls) = dispatcher(
            Err(PreReviewGateConfigLoadError::ReadFailed {
                message: FreeText::new("configuration unavailable"),
            }),
            Ok(domain::task_contract::PreReviewGateOutcome::Passed),
        );

        let error = interactor.dispatch(dispatch_command("implementation")).unwrap_err();

        assert!(matches!(error, PreReviewGateDispatchError::Config(_)));
        assert!(gate_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pre_review_gate_dispatch_matrix_lookup_failure_returns_lookup_error_without_task_contract_call()
     {
        let (interactor, _config_calls, gate_calls) = dispatcher(
            Ok(matrix(vec![(scope("planning"), vec![])])),
            Ok(domain::task_contract::PreReviewGateOutcome::Passed),
        );

        let error = interactor.dispatch(dispatch_command("implementation")).unwrap_err();

        assert!(matches!(
            error,
            PreReviewGateDispatchError::Lookup(PreReviewGateLookupError::UnknownScope(
                unknown_scope
            )) if unknown_scope == scope("implementation")
        ));
        assert!(gate_calls.lock().unwrap().is_empty());
    }

    #[test]
    fn test_pre_review_gate_dispatch_task_contract_failure_returns_task_contract_error() {
        let (interactor, _config_calls, gate_calls) = dispatcher(
            Ok(matrix(vec![(
                scope("implementation"),
                vec![PreReviewGateKind::TaskContractLiveness],
            )])),
            Err(PreReviewGateError::TaskContractReadFailed {
                message: FreeText::new("task contract unavailable"),
            }),
        );

        let error = interactor.dispatch(dispatch_command("implementation")).unwrap_err();

        assert!(matches!(error, PreReviewGateDispatchError::TaskContract(_)));
        assert_eq!(gate_calls.lock().unwrap().len(), 1);
    }
}
