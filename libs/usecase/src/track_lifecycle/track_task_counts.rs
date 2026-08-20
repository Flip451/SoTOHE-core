use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;
use crate::task_ops::TaskCountsOutput;

use super::{TaskCount, TrackItemsDirectory, TrackSelection, TrackSelectionPort};

/// Validated command for querying task counts.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTaskCountsCommand {
    /// The track items directory used by the query.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
}

/// Presentation-free result of a task-counts query.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackTaskCountsResult {
    /// The total number of tasks.
    pub total: TaskCount,
    /// Tasks in the todo status.
    pub todo: TaskCount,
    /// Tasks in the in_progress status.
    pub in_progress: TaskCount,
    /// Tasks in the done status.
    pub done: TaskCount,
    /// Tasks in the skipped status.
    pub skipped: TaskCount,
}

/// Error returned by the task-counts command boundary.
#[derive(Debug)]
pub enum TrackTaskCountsError {
    /// The storage query or its usecase mapping failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackTaskCountsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackTaskCountsError {}

/// Secondary port for querying per-status task counts.
pub trait TrackTaskCountsQueryPort: Send + Sync {
    /// Returns task counts for the selected track.
    fn task_counts(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
    ) -> Result<TaskCountsOutput, TrackTaskCountsError>;
}

/// Application service for querying per-status task counts.
pub trait TrackTaskCountsService: Send + Sync {
    /// Resolves the selection and returns the task counts.
    fn execute(
        &self,
        command: TrackTaskCountsCommand,
    ) -> Result<TrackTaskCountsResult, TrackTaskCountsError>;
}

/// Interactor for the task-counts command context.
pub struct TrackTaskCountsInteractor {
    query: Arc<dyn TrackTaskCountsQueryPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackTaskCountsInteractor {
    /// Creates an interactor from the query and selection ports.
    #[must_use]
    pub fn new(
        query: Arc<dyn TrackTaskCountsQueryPort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { query, resolver }
    }
}

impl TrackTaskCountsService for TrackTaskCountsInteractor {
    fn execute(
        &self,
        command: TrackTaskCountsCommand,
    ) -> Result<TrackTaskCountsResult, TrackTaskCountsError> {
        let TrackTaskCountsCommand { items_dir, track } = command;
        let track_id = match &track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => self
                .resolver
                .resolve_required(&items_dir, &track)
                .map_err(|error| execution_failed(error.to_string()))?,
        };
        map_counts(self.query.task_counts(track_id, items_dir)?)
    }
}

fn map_counts(output: TaskCountsOutput) -> Result<TrackTaskCountsResult, TrackTaskCountsError> {
    let todo = count(output.todo)?;
    let in_progress = count(output.in_progress)?;
    let done = count(output.done)?;
    let skipped = count(output.skipped)?;
    let total = [todo, in_progress, done, skipped].into_iter().try_fold(0_u64, |acc, value| {
        acc.checked_add(value.value()).ok_or_else(|| execution_failed("task count overflow"))
    })?;
    Ok(TrackTaskCountsResult { total: TaskCount::new(total), todo, in_progress, done, skipped })
}

fn count(value: usize) -> Result<TaskCount, TrackTaskCountsError> {
    u64::try_from(value).map(TaskCount::new).map_err(|_| execution_failed("task count exceeds u64"))
}

fn execution_failed(error: impl Into<String>) -> TrackTaskCountsError {
    TrackTaskCountsError::ExecutionFailed(DiagnosticText::new(error))
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
        result: Result<TaskCountsOutput, String>,
        calls: Mutex<Vec<(TrackId, PathBuf)>>,
    }

    impl TrackTaskCountsQueryPort for RecordingQuery {
        fn task_counts(
            &self,
            track_id: TrackId,
            items_dir: TrackItemsDirectory,
        ) -> Result<TaskCountsOutput, TrackTaskCountsError> {
            self.calls
                .lock()
                .expect("query lock is available")
                .push((track_id, items_dir.as_path().to_path_buf()));
            match &self.result {
                Ok(output) => Ok(TaskCountsOutput {
                    todo: output.todo,
                    in_progress: output.in_progress,
                    done: output.done,
                    skipped: output.skipped,
                }),
                Err(error) => Err(execution_failed(error.clone())),
            }
        }
    }

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("track/items"))
            .expect("items directory is valid")
    }

    fn unused_resolver() -> RecordingResolver {
        RecordingResolver {
            active: Ok(TrackId::try_new("ignored").expect("track id is valid")),
            calls: Mutex::new(0),
        }
    }

    #[test]
    fn test_track_task_counts_interactor_explicit_selection_returns_counts() {
        let query = RecordingQuery {
            result: Ok(TaskCountsOutput { todo: 2, in_progress: 1, done: 3, skipped: 4 }),
            calls: Mutex::new(Vec::new()),
        };
        let interactor =
            TrackTaskCountsInteractor::new(Arc::new(query), Arc::new(unused_resolver()));
        let result = interactor
            .execute(TrackTaskCountsCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("counts-track").expect("track id is valid"),
                ),
            })
            .expect("query succeeds");
        assert_eq!(result.total.value(), 10);
        assert_eq!(result.todo.value(), 2);
        assert_eq!(result.in_progress.value(), 1);
        assert_eq!(result.done.value(), 3);
        assert_eq!(result.skipped.value(), 4);
    }

    #[test]
    fn test_track_task_counts_interactor_missing_plan_returns_zero_counts() {
        let query = RecordingQuery {
            result: Ok(TaskCountsOutput { todo: 0, in_progress: 0, done: 0, skipped: 0 }),
            calls: Mutex::new(Vec::new()),
        };
        let interactor =
            TrackTaskCountsInteractor::new(Arc::new(query), Arc::new(unused_resolver()));
        let result = interactor
            .execute(TrackTaskCountsCommand {
                items_dir: items_dir(),
                track: TrackSelection::Explicit(
                    TrackId::try_new("counts-track").expect("track id is valid"),
                ),
            })
            .expect("missing plan is not an error");
        assert_eq!(
            result,
            TrackTaskCountsResult {
                total: TaskCount::new(0),
                todo: TaskCount::new(0),
                in_progress: TaskCount::new(0),
                done: TaskCount::new(0),
                skipped: TaskCount::new(0),
            }
        );
    }

    #[test]
    fn test_track_task_counts_interactor_active_selection_uses_resolver() {
        let query = RecordingQuery {
            result: Ok(TaskCountsOutput { todo: 0, in_progress: 0, done: 0, skipped: 0 }),
            calls: Mutex::new(Vec::new()),
        };
        let resolver = Arc::new(RecordingResolver {
            active: Ok(TrackId::try_new("active-track").expect("track id is valid")),
            calls: Mutex::new(0),
        });
        let interactor = TrackTaskCountsInteractor::new(Arc::new(query), resolver.clone());
        interactor
            .execute(TrackTaskCountsCommand {
                items_dir: items_dir(),
                track: TrackSelection::Active,
            })
            .expect("active query succeeds");
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 1);
    }

    #[test]
    fn test_track_task_counts_interactor_query_failure_maps_to_execution_error() {
        let query = RecordingQuery {
            result: Err("store failed".to_owned()),
            calls: Mutex::new(Vec::new()),
        };
        let interactor =
            TrackTaskCountsInteractor::new(Arc::new(query), Arc::new(unused_resolver()));
        let error = match interactor.execute(TrackTaskCountsCommand {
            items_dir: items_dir(),
            track: TrackSelection::Explicit(
                TrackId::try_new("counts-track").expect("track id is valid"),
            ),
        }) {
            Ok(_) => panic!("query failure must propagate"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), "store failed");
    }

    #[test]
    fn test_track_task_counts_interactor_active_selection_failure_maps_to_execution_error() {
        let query = Arc::new(RecordingQuery {
            result: Ok(TaskCountsOutput { todo: 0, in_progress: 0, done: 0, skipped: 0 }),
            calls: Mutex::new(Vec::new()),
        });
        let resolver = RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            calls: Mutex::new(0),
        };
        let interactor = TrackTaskCountsInteractor::new(query.clone(), Arc::new(resolver));
        let error = match interactor.execute(TrackTaskCountsCommand {
            items_dir: items_dir(),
            track: TrackSelection::Active,
        }) {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), "active track unavailable");
        assert!(query.calls.lock().expect("query lock is available").is_empty());
    }
}
