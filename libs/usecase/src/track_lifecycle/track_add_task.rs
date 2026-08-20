use std::path::Path;
use std::sync::Arc;

use domain::{NonEmptyString, TaskId, TaskStatusKind, TrackId, TrackStatus};

use crate::git_workflow::DiagnosticText;
use crate::task_ops::{TaskOperationError, TaskOperationOutput};

use super::{
    TrackItemsDirectory, TrackSelection, TrackSelectionPort, TrackViewSyncOutcome, TrackViewsPort,
    TrackViewsScope, TrackWorkspaceRoot,
};

/// Validated command for adding a task.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackAddTaskCommand {
    /// The track items directory used by the operation.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
    /// The validated task description.
    pub description: NonEmptyString,
    /// Optional validated task section.
    pub section: Option<NonEmptyString>,
    /// Optional validated predecessor task id.
    pub after: Option<TaskId>,
}

impl TrackAddTaskCommand {
    /// Validates primary-adapter strings and creates an add-task command.
    pub fn try_new(
        items_dir: TrackItemsDirectory,
        track: TrackSelection,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> Result<Self, TrackAddTaskError> {
        let description = NonEmptyString::try_new(description)
            .map_err(|error| invalid_input("task description", error))?;
        let section = section
            .map(|value| {
                NonEmptyString::try_new(value).map_err(|error| invalid_input("task section", error))
            })
            .transpose()?;
        let after = after
            .map(|value| {
                if !is_task_id_token(&value) {
                    return Err(TrackAddTaskError::ExecutionFailed(DiagnosticText::new(format!(
                        "invalid --after value {value:?}: expected T<digits> (e.g. T001)"
                    ))));
                }
                TaskId::try_new(value).map_err(|error| invalid_input("predecessor task id", error))
            })
            .transpose()?;

        Ok(Self { items_dir, track, description, section, after })
    }
}

fn is_task_id_token(value: &str) -> bool {
    value.strip_prefix('T').is_some_and(|digits| {
        !digits.is_empty()
            && digits.chars().all(|character| character.is_ascii_digit())
            && digits.parse::<u64>().is_ok()
    })
}

/// Secondary port for adding a task with a validated command.
pub trait TrackTaskAddPort: Send + Sync {
    /// Adds a task using the validated command boundary.
    fn add_task(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
        description: NonEmptyString,
        section: Option<NonEmptyString>,
        after: Option<TaskId>,
    ) -> Result<TaskOperationOutput, TaskOperationError>;
}

/// Presentation-free result of adding a task.
pub struct TrackAddTaskResult {
    /// The track that received the task.
    pub track_id: TrackId,
    /// The newly allocated task id.
    pub task_id: TaskId,
    /// The task description that was persisted.
    pub description: NonEmptyString,
    /// The status of the newly allocated task.
    pub status: TaskStatusKind,
    /// The derived status of the containing track after persistence.
    pub derived_status: TrackStatus,
    /// The rendered-view synchronization outcome.
    pub view_sync: TrackViewSyncOutcome,
}

/// Error returned by the add-task command boundary.
#[derive(Debug)]
pub enum TrackAddTaskError {
    /// The add-task operation or its view synchronization failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackAddTaskError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackAddTaskError {}

/// Application service for adding a task.
pub trait TrackAddTaskService: Send + Sync {
    /// Persists a task and synchronizes the rendered views.
    fn execute(
        &self,
        command: TrackAddTaskCommand,
    ) -> Result<TrackAddTaskResult, TrackAddTaskError>;
}

/// Interactor for the add-task command context.
pub struct TrackAddTaskInteractor {
    operation: Arc<dyn TrackTaskAddPort>,
    resolver: Arc<dyn TrackSelectionPort>,
    views: Arc<dyn TrackViewsPort>,
}

impl TrackAddTaskInteractor {
    /// Creates an interactor from the task operation, selection, and view ports.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackTaskAddPort>,
        resolver: Arc<dyn TrackSelectionPort>,
        views: Arc<dyn TrackViewsPort>,
    ) -> Self {
        Self { operation, resolver, views }
    }
}

impl TrackAddTaskService for TrackAddTaskInteractor {
    fn execute(
        &self,
        command: TrackAddTaskCommand,
    ) -> Result<TrackAddTaskResult, TrackAddTaskError> {
        let TrackAddTaskCommand { items_dir, track, description, section, after } = command;
        let track_id = match &track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => self
                .resolver
                .resolve_required(&items_dir, &track)
                .map_err(|error| execution_failed(error.to_string()))?,
        };
        let workspace_root = workspace_root_for_items(&items_dir)?;
        let output = self
            .operation
            .add_task(track_id, items_dir, description.clone(), section, after)
            .map_err(|error| execution_failed(format!("add-task failed: {error}")))?;

        let result_track_id = TrackId::try_new(output.track_id.clone())
            .map_err(|error| execution_failed(format!("invalid persisted track id: {error}")))?;
        let task_id = output.task_id.ok_or_else(|| {
            execution_failed("add-task operation returned no newly allocated task id")
        })?;
        let task_id = TaskId::try_new(task_id)
            .map_err(|error| execution_failed(format!("invalid persisted task id: {error}")))?;
        let derived_status = parse_track_status(&output.derived_status)?;
        let view_sync = self.sync_views(&workspace_root, &track);

        Ok(TrackAddTaskResult {
            track_id: result_track_id,
            task_id,
            description,
            status: TaskStatusKind::Todo,
            derived_status,
            view_sync,
        })
    }
}

impl TrackAddTaskInteractor {
    fn sync_views(
        &self,
        workspace_root: &TrackWorkspaceRoot,
        selection: &TrackSelection,
    ) -> TrackViewSyncOutcome {
        let scope = match selection {
            // An explicit selection has already been validated by the primary adapter and the
            // task-operation branch guard. Keeping the scope local also preserves branchless
            // track behavior used by planning fixtures.
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
) -> Result<TrackWorkspaceRoot, TrackAddTaskError> {
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

fn parse_track_status(value: &str) -> Result<TrackStatus, TrackAddTaskError> {
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

fn invalid_input(label: &str, error: impl std::fmt::Display) -> TrackAddTaskError {
    execution_failed(format!("invalid {label}: {error}"))
}

fn execution_failed(error: impl Into<String>) -> TrackAddTaskError {
    TrackAddTaskError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::{RenderedViewPath, TrackViewsScope};

    struct RecordingResolver {
        active: Result<TrackId, DiagnosticText>,
        active_calls: Mutex<usize>,
        view_scope_calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            *self.active_calls.lock().expect("resolver lock is available") += 1;
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
        calls: Mutex<Vec<(TrackId, PathBuf, NonEmptyString)>>,
        result: Result<TaskOperationOutput, String>,
    }

    impl TrackTaskAddPort for RecordingOperation {
        fn add_task(
            &self,
            track_id: TrackId,
            items_dir: TrackItemsDirectory,
            description: NonEmptyString,
            _section: Option<NonEmptyString>,
            _after: Option<TaskId>,
        ) -> Result<TaskOperationOutput, TaskOperationError> {
            self.calls.lock().expect("operation lock is available").push((
                track_id,
                items_dir.as_path().to_path_buf(),
                description,
            ));
            match &self.result {
                Ok(output) => Ok(TaskOperationOutput {
                    track_id: output.track_id.clone(),
                    task_id: output.task_id.clone(),
                    derived_status: output.derived_status.clone(),
                }),
                Err(error) => Err(TaskOperationError::StoreFailed(error.clone())),
            }
        }
    }

    struct RecordingViews {
        error: Option<DiagnosticText>,
        calls: Mutex<usize>,
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
            *self.calls.lock().expect("views lock is available") += 1;
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

    fn command(track: TrackSelection) -> TrackAddTaskCommand {
        TrackAddTaskCommand::try_new(
            items_dir(),
            track,
            "new task".to_owned(),
            Some("work".to_owned()),
            Some("T001".to_owned()),
        )
        .expect("command is valid")
    }

    fn output() -> TaskOperationOutput {
        TaskOperationOutput {
            track_id: "add-task-track".to_owned(),
            task_id: Some("T002".to_owned()),
            derived_status: "in_progress".to_owned(),
        }
    }

    #[test]
    fn test_track_add_task_command_try_new_rejects_invalid_after_with_legacy_message() {
        let error = TrackAddTaskCommand::try_new(
            items_dir(),
            TrackSelection::Explicit(track_id("add-task-track")),
            "new task".to_owned(),
            None,
            Some("not-a-task".to_owned()),
        )
        .expect_err("invalid after token must fail");

        assert!(error.to_string().contains("invalid --after value \"not-a-task\""));
    }

    #[test]
    fn test_track_add_task_interactor_explicit_selection_persists_and_syncs_views() {
        let operation =
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok(output()) });
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            active_calls: Mutex::new(0),
            view_scope_calls: Mutex::new(0),
        });
        let views = Arc::new(RecordingViews { error: None, calls: Mutex::new(0) });
        let interactor = TrackAddTaskInteractor::new(operation.clone(), resolver.clone(), views);

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("add-task-track"))))
            .expect("add-task succeeds");

        assert_eq!(result.track_id.as_ref(), "add-task-track");
        assert_eq!(result.task_id.as_ref(), "T002");
        assert_eq!(result.description.as_ref(), "new task");
        assert_eq!(result.status, TaskStatusKind::Todo);
        assert_eq!(result.derived_status, TrackStatus::InProgress);
        assert!(matches!(result.view_sync, TrackViewSyncOutcome::Synchronized(_)));
        assert_eq!(*resolver.active_calls.lock().expect("resolver lock is available"), 0);
        assert_eq!(*resolver.view_scope_calls.lock().expect("resolver lock is available"), 0);
        assert_eq!(operation.calls.lock().expect("operation lock is available").len(), 1);
    }

    #[test]
    fn test_track_add_task_interactor_active_selection_resolves_and_propagates_operation_error() {
        let interactor = TrackAddTaskInteractor::new(
            Arc::new(RecordingOperation {
                calls: Mutex::new(Vec::new()),
                result: Err("storage unavailable".to_owned()),
            }),
            Arc::new(RecordingResolver {
                active: Ok(track_id("active-track")),
                active_calls: Mutex::new(0),
                view_scope_calls: Mutex::new(0),
            }),
            Arc::new(RecordingViews { error: None, calls: Mutex::new(0) }),
        );

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("operation failure must propagate"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("add-task failed"));
        assert!(error.to_string().contains("storage unavailable"));
    }

    #[test]
    fn test_track_add_task_interactor_view_failure_returns_warning_after_persistence() {
        let interactor = TrackAddTaskInteractor::new(
            Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result: Ok(output()) }),
            Arc::new(RecordingResolver {
                active: Ok(track_id("active-track")),
                active_calls: Mutex::new(0),
                view_scope_calls: Mutex::new(0),
            }),
            Arc::new(RecordingViews {
                error: Some(DiagnosticText::new("render failed")),
                calls: Mutex::new(0),
            }),
        );

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("add-task-track"))))
            .expect("view failure is a warning");

        assert!(matches!(
            result.view_sync,
            TrackViewSyncOutcome::Warning { diagnostic, .. }
                if diagnostic.as_str() == "render failed"
        ));
    }
}
