use std::sync::Arc;

use domain::{NonEmptyString, TaskId, TaskStatusKind, TrackId};

use crate::git_workflow::DiagnosticText;
use crate::task_ops::NextTaskOutput;

use super::{TrackItemsDirectory, TrackSelection, TrackSelectionPort};

/// Validated command for querying the next open task.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackNextTaskCommand {
    /// The track items directory used by the query.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
}

/// Presentation-free result of a next-task query.
pub enum TrackNextTaskResult {
    /// The next open task in plan order.
    Found {
        /// The task identifier.
        task_id: TaskId,
        /// The task description.
        description: NonEmptyString,
        /// The task status used by the existing JSON contract.
        status: TaskStatusKind,
    },
    /// The track has no open task.
    NoOpenTask,
}

/// Error returned by the next-task command boundary.
#[derive(Debug)]
pub enum TrackNextTaskError {
    /// The storage query or its usecase mapping failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackNextTaskError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackNextTaskError {}

/// Secondary port for querying the next open task.
pub trait TrackNextTaskQueryPort: Send + Sync {
    /// Returns the next open task, when one exists.
    fn next_task(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
    ) -> Result<Option<NextTaskOutput>, TrackNextTaskError>;
}

/// Application service for querying the next open task.
pub trait TrackNextTaskService: Send + Sync {
    /// Resolves the selection and returns the next open task.
    fn execute(
        &self,
        command: TrackNextTaskCommand,
    ) -> Result<TrackNextTaskResult, TrackNextTaskError>;
}

/// Interactor for the next-task command context.
pub struct TrackNextTaskInteractor {
    query: Arc<dyn TrackNextTaskQueryPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackNextTaskInteractor {
    /// Creates an interactor from the query and selection ports.
    #[must_use]
    pub fn new(
        query: Arc<dyn TrackNextTaskQueryPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { query, resolver }
    }
}

impl TrackNextTaskService for TrackNextTaskInteractor {
    fn execute(
        &self,
        command: TrackNextTaskCommand,
    ) -> Result<TrackNextTaskResult, TrackNextTaskError> {
        let TrackNextTaskCommand { items_dir, track } = command;
        let track_id = match &track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => self
                .resolver
                .resolve_required(&items_dir, &track)
                .map_err(|error| execution_failed(error.to_string()))?,
        };
        match self.query.next_task(track_id, items_dir)? {
            None => Ok(TrackNextTaskResult::NoOpenTask),
            Some(output) => map_found(output),
        }
    }
}

fn map_found(output: NextTaskOutput) -> Result<TrackNextTaskResult, TrackNextTaskError> {
    let task_id = TaskId::try_new(output.task_id)
        .map_err(|error| execution_failed(format!("invalid persisted task id: {error}")))?;
    let description = NonEmptyString::try_new(output.description).map_err(|error| {
        execution_failed(format!("invalid persisted task description: {error}"))
    })?;
    let status = parse_status(&output.status)?;
    Ok(TrackNextTaskResult::Found { task_id, description, status })
}

fn parse_status(value: &str) -> Result<TaskStatusKind, TrackNextTaskError> {
    match value {
        "todo" => Ok(TaskStatusKind::Todo),
        "in_progress" => Ok(TaskStatusKind::InProgress),
        "done" => Ok(TaskStatusKind::Done),
        "skipped" => Ok(TaskStatusKind::Skipped),
        other => Err(execution_failed(format!("invalid persisted task status: {other}"))),
    }
}

fn execution_failed(error: impl Into<String>) -> TrackNextTaskError {
    TrackNextTaskError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::{TrackViewsScope, TrackWorkspaceRoot};

    struct RecordingResolver {
        active: Result<TrackId, DiagnosticText>,
        calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            *self.calls.lock().expect("resolver lock is available") += 1;
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
            Ok(TrackViewsScope::RegistryOnly)
        }
    }

    struct RecordingQuery {
        result: Result<Option<NextTaskOutput>, String>,
        calls: Mutex<Vec<(TrackId, PathBuf)>>,
    }

    impl TrackNextTaskQueryPort for RecordingQuery {
        fn next_task(
            &self,
            track_id: TrackId,
            items_dir: TrackItemsDirectory,
        ) -> Result<Option<NextTaskOutput>, TrackNextTaskError> {
            self.calls
                .lock()
                .expect("query lock is available")
                .push((track_id, items_dir.as_path().to_path_buf()));
            match &self.result {
                Ok(None) => Ok(None),
                Ok(Some(output)) => Ok(Some(NextTaskOutput {
                    task_id: output.task_id.clone(),
                    description: output.description.clone(),
                    status: output.status.clone(),
                })),
                Err(error) => Err(execution_failed(error.clone())),
            }
        }
    }

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("track/items"))
            .expect("items directory is valid")
    }

    #[test]
    fn test_track_next_task_interactor_explicit_selection_returns_found_task() {
        let query = RecordingQuery {
            result: Ok(Some(NextTaskOutput {
                task_id: "T002".to_owned(),
                description: "next work".to_owned(),
                status: "todo".to_owned(),
            })),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        };
        let interactor = TrackNextTaskInteractor::new(Arc::new(query), Arc::new(resolver));
        let result = interactor
            .execute(TrackNextTaskCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("next-track").expect("track id is valid"),
                ),
            })
            .expect("query succeeds");
        assert!(matches!(
            result,
            TrackNextTaskResult::Found { task_id, description, status: TaskStatusKind::Todo }
                if task_id.as_ref() == "T002" && description.as_ref() == "next work"
        ));
    }

    #[test]
    fn test_track_next_task_interactor_missing_plan_returns_no_open_task() {
        let query = RecordingQuery { result: Ok(None), calls: Mutex::new(Vec::new()) };
        let resolver = RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        };
        let interactor = TrackNextTaskInteractor::new(Arc::new(query), Arc::new(resolver));
        let result = interactor
            .execute(TrackNextTaskCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("next-track").expect("track id is valid"),
                ),
            })
            .expect("missing plan is not an error");
        assert!(matches!(result, TrackNextTaskResult::NoOpenTask));
    }

    #[test]
    fn test_track_next_task_interactor_active_selection_uses_resolver() {
        let query = RecordingQuery { result: Ok(None), calls: Mutex::new(Vec::new()) };
        let resolver = Arc::new(RecordingResolver {
            active: Ok(TrackId::try_new("active-track").expect("track id is valid")),
            calls: Mutex::new(0),
        });
        let interactor = TrackNextTaskInteractor::new(Arc::new(query), resolver.clone());
        interactor
            .execute(TrackNextTaskCommand { items_dir: items_dir(), track: TrackSelection::Active })
            .expect("active query succeeds");
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 1);
    }

    #[test]
    fn test_track_next_task_interactor_in_progress_status_maps_to_found_status() {
        let query = RecordingQuery {
            result: Ok(Some(NextTaskOutput {
                task_id: "T003".to_owned(),
                description: "current work".to_owned(),
                status: "in_progress".to_owned(),
            })),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        };
        let interactor = TrackNextTaskInteractor::new(Arc::new(query), Arc::new(resolver));
        let result = interactor
            .execute(TrackNextTaskCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("next-track").expect("track id is valid"),
                ),
            })
            .expect("query succeeds");
        assert!(matches!(
            result,
            TrackNextTaskResult::Found { task_id, description, status: TaskStatusKind::InProgress }
                if task_id.as_ref() == "T003" && description.as_ref() == "current work"
        ));
    }

    #[test]
    fn test_track_next_task_interactor_active_selection_failure_maps_to_execution_error() {
        let query = Arc::new(RecordingQuery { result: Ok(None), calls: Mutex::new(Vec::new()) });
        let resolver = RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            calls: Mutex::new(0),
        };
        let interactor = TrackNextTaskInteractor::new(query.clone(), Arc::new(resolver));
        let error = match interactor
            .execute(TrackNextTaskCommand { items_dir: items_dir(), track: TrackSelection::Active })
        {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), "active track unavailable");
        assert!(query.calls.lock().expect("query lock is available").is_empty());
    }
}
