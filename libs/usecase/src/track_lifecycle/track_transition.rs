use std::path::Path;
use std::sync::Arc;

use domain::{TaskId, TaskStatusKind, TrackId, TrackStatus};

use crate::git_workflow::DiagnosticText;
use crate::task_ops::{TaskOperationError, TaskTransitionOutcome};

use super::{
    TrackItemsDirectory, TrackSelection, TrackSelectionPort, TrackTaskTransition,
    TrackViewSyncOutcome, TrackViewsPort, TrackViewsScope, TrackWorkspaceRoot,
};

/// Validated command for a task transition.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTransitionCommand {
    /// The track items directory used by the operation.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
    /// The task to transition.
    pub task_id: TaskId,
    /// The validated target transition.
    pub transition: TrackTaskTransition,
}

impl TrackTransitionCommand {
    /// Validates the task id string and constructs a transition command.
    pub fn try_new(
        items_dir: TrackItemsDirectory,
        track: TrackSelection,
        task_id: String,
        transition: TrackTaskTransition,
    ) -> Result<Self, TrackTransitionError> {
        let task_id = TaskId::try_new(task_id).map_err(|error| {
            execution_failed(format!("transition failed: invalid task ID: {error}"))
        })?;
        Ok(Self { items_dir, track, task_id, transition })
    }
}

/// Secondary port for applying a validated task transition.
pub trait TrackTaskTransitionPort: Send + Sync {
    /// Applies the requested task transition.
    fn transition_task(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
        task_id: TaskId,
        transition: TrackTaskTransition,
    ) -> Result<TaskTransitionOutcome, TaskOperationError>;
}

/// Presentation-free result of a task transition.
pub enum TrackTransitionResult {
    /// The task was transitioned and the containing track status was derived.
    Transitioned {
        /// The track that received the transition.
        track_id: TrackId,
        /// The task that was transitioned.
        task_id: TaskId,
        /// The requested target status.
        target_status: TaskStatusKind,
        /// The derived status of the containing track.
        derived_status: TrackStatus,
        /// The rendered-view synchronization outcome.
        view_sync: TrackViewSyncOutcome,
    },
    /// Admission refused the transition without changing persisted state.
    Rejected {
        /// The task whose transition was refused.
        task_id: TaskId,
        /// The structured refusal diagnostic.
        reason: DiagnosticText,
    },
}

/// Error returned by the task-transition command boundary.
#[derive(Debug)]
pub enum TrackTransitionError {
    /// The transition operation or its result mapping failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackTransitionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackTransitionError {}

/// Application service for task transitions.
pub trait TrackTransitionService: Send + Sync {
    /// Resolves the selection, applies the transition, and synchronizes views.
    fn execute(
        &self,
        command: TrackTransitionCommand,
    ) -> Result<TrackTransitionResult, TrackTransitionError>;
}

/// Interactor for the task-transition command context.
pub struct TrackTransitionInteractor {
    operation: Arc<dyn TrackTaskTransitionPort>,
    resolver: Arc<dyn TrackSelectionPort>,
    views: Arc<dyn TrackViewsPort>,
}

impl TrackTransitionInteractor {
    /// Creates an interactor from the transition, selection, and view ports.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackTaskTransitionPort>,
        resolver: Arc<dyn TrackSelectionPort>,
        views: Arc<dyn TrackViewsPort>,
    ) -> Self {
        Self { operation, resolver, views }
    }
}

impl TrackTransitionService for TrackTransitionInteractor {
    fn execute(
        &self,
        command: TrackTransitionCommand,
    ) -> Result<TrackTransitionResult, TrackTransitionError> {
        let TrackTransitionCommand { items_dir, track, task_id, transition } = command;
        let track_id = match &track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => self
                .resolver
                .resolve_required(&items_dir, &track)
                .map_err(|error| execution_failed(error.to_string()))?,
        };
        let workspace_root = workspace_root_for_items(&items_dir)?;
        let target_status = target_status_kind(&transition);
        let outcome = self
            .operation
            .transition_task(track_id, items_dir, task_id.clone(), transition)
            .map_err(|error| execution_failed(format!("transition failed: {error}")))?;

        match outcome {
            TaskTransitionOutcome::Transitioned(output) => {
                let derived_status = parse_track_status(&output.derived_status)?;
                let track_id = parse_track_id(output.track_id)?;
                let view_sync = self.sync_views(&workspace_root, &track);
                Ok(TrackTransitionResult::Transitioned {
                    track_id,
                    task_id,
                    target_status,
                    derived_status,
                    view_sync,
                })
            }
            TaskTransitionOutcome::Rejected(rejection) => Ok(TrackTransitionResult::Rejected {
                task_id,
                reason: DiagnosticText::new(rejection.to_string()),
            }),
        }
    }
}

impl TrackTransitionInteractor {
    fn sync_views(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        selection: &TrackSelection,
    ) -> TrackViewSyncOutcome {
        let scope = match selection {
            TrackSelection::Explicit(track_id) => TrackViewsScope::Track(track_id.clone()),
            TrackSelection::Active => {
                match self.resolver.resolve_views_scope(workspace_root, selection) {
                    Ok(scope) => scope,
                    Err(error) => {
                        return TrackViewSyncOutcome::Warning {
                            rendered_views: Vec::new(),
                            diagnostic: error,
                        };
                    }
                }
            }
        };

        match self.views.sync(workspace_root, &scope) {
            Ok(rendered_views) => TrackViewSyncOutcome::Synchronized(rendered_views),
            Err(diagnostic) => {
                TrackViewSyncOutcome::Warning { rendered_views: Vec::new(), diagnostic }
            }
        }
    }
}

fn workspace_root_for_items(
    items_dir: &TrackItemsDirectory,
) -> Result<TrackWorkspaceRoot, TrackTransitionError> {
    let track_dir = items_dir
        .as_path()
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no track parent"))?;
    let root = track_dir
        .parent()
        .ok_or_else(|| execution_failed("track items directory has no workspace root"))?;
    let root = if root.as_os_str().is_empty() { Path::new(".") } else { root };
    TrackWorkspaceRoot::try_new(root.to_path_buf())
        .map_err(|error| execution_failed(error.to_string()))
}

fn parse_track_id(value: String) -> Result<TrackId, TrackTransitionError> {
    TrackId::try_new(value)
        .map_err(|error| execution_failed(format!("invalid persisted track id: {error}")))
}

fn parse_track_status(value: &str) -> Result<TrackStatus, TrackTransitionError> {
    match value {
        "planned" => Ok(TrackStatus::Planned),
        "in_progress" => Ok(TrackStatus::InProgress),
        "done" => Ok(TrackStatus::Done),
        "blocked" => Ok(TrackStatus::Blocked),
        "cancelled" => Ok(TrackStatus::Cancelled),
        "archived" => Ok(TrackStatus::Archived),
        other => Err(execution_failed(format!("invalid derived track status: {other}"))),
    }
}

fn target_status_kind(transition: &TrackTaskTransition) -> TaskStatusKind {
    match transition {
        TrackTaskTransition::Todo => TaskStatusKind::Todo,
        TrackTaskTransition::InProgress => TaskStatusKind::InProgress,
        TrackTaskTransition::Done { .. } => TaskStatusKind::Done,
        TrackTaskTransition::Skipped => TaskStatusKind::Skipped,
    }
}

fn execution_failed(error: impl Into<String>) -> TrackTransitionError {
    TrackTransitionError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::task_admission::AdmissionRejectionOutput;
    use crate::task_ops::TaskOperationOutput;
    use crate::track_lifecycle::{RenderedViewPath, TrackViewsScope};

    struct RecordingResolver {
        active: Result<TrackId, DiagnosticText>,
        required_calls: Mutex<usize>,
        view_scope_calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            *self.required_calls.lock().expect("resolver lock is available") += 1;
            self.active.clone()
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            self.active.clone()
        }

        fn resolve_views_scope(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _selection: &TrackSelection,
        ) -> Result<TrackViewsScope, DiagnosticText> {
            *self.view_scope_calls.lock().expect("resolver lock is available") += 1;
            Ok(TrackViewsScope::RegistryOnly)
        }
    }

    struct RecordingOperation {
        calls: Mutex<Vec<(TrackId, PathBuf, TaskId, TrackTaskTransition)>>,
        result: Mutex<Option<Result<TaskTransitionOutcome, TaskOperationError>>>,
    }

    impl TrackTaskTransitionPort for RecordingOperation {
        fn transition_task(
            &self,
            track_id: TrackId,
            items_dir: TrackItemsDirectory,
            task_id: TaskId,
            transition: TrackTaskTransition,
        ) -> Result<TaskTransitionOutcome, TaskOperationError> {
            self.calls.lock().expect("operation lock is available").push((
                track_id,
                items_dir.as_path().to_path_buf(),
                task_id,
                transition,
            ));
            self.result.lock().expect("operation lock is available").take().expect("one result")
        }
    }

    struct RecordingViews {
        error: Option<DiagnosticText>,
    }

    impl TrackViewsPort for RecordingViews {
        fn validate(&self, _workspace_root: &TrackWorkspaceRoot) -> Result<(), DiagnosticText> {
            Ok(())
        }

        fn sync(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _scope: &TrackViewsScope,
        ) -> Result<Vec<RenderedViewPath>, DiagnosticText> {
            match &self.error {
                Some(error) => Err(DiagnosticText::new(error.as_str())),
                None => {
                    Ok(vec![RenderedViewPath::new(PathBuf::from("workspace/track/registry.md"))])
                }
            }
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
            .expect("items directory is valid")
    }

    fn command(track: TrackSelection) -> TrackTransitionCommand {
        TrackTransitionCommand {
            items_dir: items_dir(),
            track,
            task_id: TaskId::try_new("T001").expect("task id is valid"),
            transition: TrackTaskTransition::InProgress,
        }
    }

    fn transitioned() -> TaskTransitionOutcome {
        TaskTransitionOutcome::Transitioned(TaskOperationOutput {
            track_id: "transition-track".to_owned(),
            task_id: None,
            derived_status: "in_progress".to_owned(),
        })
    }

    fn resolver(active: Result<TrackId, DiagnosticText>) -> Arc<RecordingResolver> {
        Arc::new(RecordingResolver {
            active,
            required_calls: Mutex::new(0),
            view_scope_calls: Mutex::new(0),
        })
    }

    fn operation(
        result: Result<TaskTransitionOutcome, TaskOperationError>,
    ) -> Arc<RecordingOperation> {
        Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            result: Mutex::new(Some(result)),
        })
    }

    #[test]
    fn test_track_transition_interactor_explicit_selection_forwards_and_syncs_views() {
        let resolver = resolver(Ok(track_id("active-track")));
        let operation = operation(Ok(transitioned()));
        let interactor = TrackTransitionInteractor::new(
            operation.clone(),
            resolver.clone(),
            Arc::new(RecordingViews { error: None }),
        );

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("transition-track"))))
            .expect("transition succeeds");

        assert!(matches!(
            result,
            TrackTransitionResult::Transitioned {
                target_status: TaskStatusKind::InProgress,
                derived_status: TrackStatus::InProgress,
                view_sync: TrackViewSyncOutcome::Synchronized(_),
                ..
            }
        ));
        assert_eq!(*resolver.required_calls.lock().expect("resolver lock is available"), 0);
        assert_eq!(operation.calls.lock().expect("operation lock is available").len(), 1);
    }

    #[test]
    fn test_track_transition_interactor_active_selection_uses_resolver() {
        let resolver = resolver(Ok(track_id("active-track")));
        let operation = operation(Ok(transitioned()));
        let interactor = TrackTransitionInteractor::new(
            operation.clone(),
            resolver.clone(),
            Arc::new(RecordingViews { error: None }),
        );

        let result = interactor
            .execute(command(TrackSelection::Active))
            .expect("active transition succeeds");

        assert!(matches!(result, TrackTransitionResult::Transitioned { .. }));
        assert_eq!(*resolver.required_calls.lock().expect("resolver lock is available"), 1);
        assert_eq!(
            operation
                .calls
                .lock()
                .expect("operation lock is available")
                .first()
                .expect("one operation call")
                .0
                .as_ref(),
            "active-track"
        );
    }

    #[test]
    fn test_track_transition_interactor_resolver_failure_returns_execution_error() {
        let resolver = resolver(Err(DiagnosticText::new("active track unavailable")));
        let operation = operation(Ok(transitioned()));
        let interactor = TrackTransitionInteractor::new(
            operation.clone(),
            resolver,
            Arc::new(RecordingViews { error: None }),
        );

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "active track unavailable");
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }

    #[test]
    fn test_track_transition_interactor_rejection_returns_typed_result() {
        let resolver = resolver(Ok(track_id("active-track")));
        let operation = operation(Ok(TaskTransitionOutcome::Rejected(
            AdmissionRejectionOutput::NoCurrentBatch {
                task_id: "T001".to_owned(),
                task_batch: "batch-a".to_owned(),
            },
        )));
        let interactor = TrackTransitionInteractor::new(
            operation,
            resolver,
            Arc::new(RecordingViews { error: None }),
        );

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("transition-track"))))
            .expect("rejection is a result");

        assert!(matches!(
            result,
            TrackTransitionResult::Rejected { task_id, reason }
                if task_id.as_ref() == "T001" && reason.as_str().contains("every declared batch")
        ));
    }

    #[test]
    fn test_track_transition_interactor_view_failure_returns_warning_after_persistence() {
        let interactor = TrackTransitionInteractor::new(
            operation(Ok(transitioned())),
            resolver(Ok(track_id("active-track"))),
            Arc::new(RecordingViews { error: Some(DiagnosticText::new("render failed")) }),
        );

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("transition-track"))))
            .expect("view failure is a warning");

        assert!(matches!(
            result,
            TrackTransitionResult::Transitioned {
                view_sync: TrackViewSyncOutcome::Warning { diagnostic, .. },
                ..
            } if diagnostic.as_str() == "render failed"
        ));
    }

    #[test]
    fn test_track_transition_interactor_operation_failure_has_transition_prefix() {
        let interactor = TrackTransitionInteractor::new(
            operation(Err(TaskOperationError::StoreFailed("storage unavailable".to_owned()))),
            resolver(Ok(track_id("active-track"))),
            Arc::new(RecordingViews { error: None }),
        );

        let error = match interactor
            .execute(command(TrackSelection::Explicit(track_id("transition-track"))))
        {
            Ok(_) => panic!("operation failure must propagate"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("transition failed: store failed"));
    }
}
