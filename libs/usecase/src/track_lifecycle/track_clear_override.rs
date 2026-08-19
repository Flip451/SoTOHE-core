use std::path::Path;
use std::sync::Arc;

use crate::git_workflow::DiagnosticText;
use crate::task_ops::{TaskOperationError, TaskOperationOutput};
use domain::{TrackId, TrackStatus};

use super::{
    TrackItemsDirectory, TrackSelection, TrackSelectionPort, TrackViewSyncOutcome, TrackViewsPort,
    TrackViewsScope, TrackWorkspaceRoot,
};

/// Validated command for clearing a status override.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackClearOverrideCommand {
    /// The track items directory used by the operation.
    pub items_dir: TrackItemsDirectory,
    /// The explicit or active track selection.
    pub track: TrackSelection,
}

/// Secondary port for clearing a track status override.
pub trait TrackOverrideClearPort: Send + Sync {
    /// Clears the current status override.
    fn clear_override(
        &self,
        track_id: TrackId,
        items_dir: TrackItemsDirectory,
    ) -> Result<TaskOperationOutput, TaskOperationError>;
}

/// Presentation-free result of clearing a track status override.
pub struct TrackClearOverrideResult {
    /// The track whose override was cleared.
    pub track_id: TrackId,
    /// The derived status after the override was removed.
    pub derived_status: TrackStatus,
    /// The rendered-view synchronization outcome.
    pub view_sync: TrackViewSyncOutcome,
}

/// Error returned by the clear-override command boundary.
#[derive(Debug)]
pub enum TrackClearOverrideError {
    /// The clear-override operation or its result mapping failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackClearOverrideError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackClearOverrideError {}

/// Application service for clearing a track status override.
pub trait TrackClearOverrideService: Send + Sync {
    /// Resolves the selected track, clears its override, and synchronizes views.
    fn execute(
        &self,
        command: TrackClearOverrideCommand,
    ) -> Result<TrackClearOverrideResult, TrackClearOverrideError>;
}

/// Interactor for the clear-override command context.
pub struct TrackClearOverrideInteractor {
    operation: Arc<dyn TrackOverrideClearPort>,
    resolver: Arc<dyn TrackSelectionPort>,
    views: Arc<dyn TrackViewsPort>,
}

impl TrackClearOverrideInteractor {
    /// Creates an interactor from the clear operation, selection, and view ports.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackOverrideClearPort>,
        resolver: Arc<dyn TrackSelectionPort>,
        views: Arc<dyn TrackViewsPort>,
    ) -> Self {
        Self { operation, resolver, views }
    }
}

impl TrackClearOverrideService for TrackClearOverrideInteractor {
    fn execute(
        &self,
        command: TrackClearOverrideCommand,
    ) -> Result<TrackClearOverrideResult, TrackClearOverrideError> {
        let TrackClearOverrideCommand { items_dir, track } = command;
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
            .clear_override(track_id, items_dir)
            .map_err(|error| execution_failed(format!("clear-override failed: {error}")))?;
        let track_id = TrackId::try_new(output.track_id)
            .map_err(|error| execution_failed(format!("invalid persisted track id: {error}")))?;
        let derived_status = parse_track_status(&output.derived_status)?;
        let view_sync = self.sync_views(&workspace_root, &track);

        Ok(TrackClearOverrideResult { track_id, derived_status, view_sync })
    }
}

impl TrackClearOverrideInteractor {
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
) -> Result<TrackWorkspaceRoot, TrackClearOverrideError> {
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

fn parse_track_status(value: &str) -> Result<TrackStatus, TrackClearOverrideError> {
    match value {
        "planned" => Ok(TrackStatus::Planned),
        "in_progress" => Ok(TrackStatus::InProgress),
        "done" => Ok(TrackStatus::Done),
        "blocked" => Ok(TrackStatus::Blocked),
        "cancelled" => Ok(TrackStatus::Cancelled),
        other => Err(execution_failed(format!("invalid persisted track status: {other}"))),
    }
}

fn execution_failed(error: impl Into<String>) -> TrackClearOverrideError {
    TrackClearOverrideError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;

    struct RecordingResolver {
        required_result: Result<TrackId, DiagnosticText>,
        views_result: Result<TrackViewsScope, DiagnosticText>,
        required_calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            *self.required_calls.lock().expect("resolver lock is available") += 1;
            self.required_result.clone()
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            self.required_result.clone()
        }

        fn resolve_views_scope(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _selection: &TrackSelection,
        ) -> Result<TrackViewsScope, DiagnosticText> {
            self.views_result.clone()
        }
    }

    struct RecordingOperation {
        calls: Mutex<Vec<TrackId>>,
        result: Result<(), TaskOperationError>,
    }

    impl TrackOverrideClearPort for RecordingOperation {
        fn clear_override(
            &self,
            track_id: TrackId,
            _items_dir: TrackItemsDirectory,
        ) -> Result<TaskOperationOutput, TaskOperationError> {
            self.calls.lock().expect("operation lock is available").push(track_id.clone());
            self.result.as_ref().map_err(|error| match error {
                TaskOperationError::StoreFailed(message) => {
                    TaskOperationError::StoreFailed(message.clone())
                }
                _ => TaskOperationError::StoreFailed("clear failed".to_owned()),
            })?;
            Ok(TaskOperationOutput {
                track_id: track_id.as_ref().to_owned(),
                task_id: None,
                derived_status: "planned".to_owned(),
            })
        }
    }

    struct RecordingViews;

    impl TrackViewsPort for RecordingViews {
        fn validate(&self, _workspace_root: &TrackWorkspaceRoot) -> Result<(), DiagnosticText> {
            Ok(())
        }

        fn sync(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _scope: &TrackViewsScope,
        ) -> Result<Vec<super::super::RenderedViewPath>, DiagnosticText> {
            Ok(vec![super::super::RenderedViewPath::new(PathBuf::from(
                "workspace/track/registry.md",
            ))])
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    fn command(track: TrackSelection) -> TrackClearOverrideCommand {
        TrackClearOverrideCommand {
            items_dir: TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
                .expect("items directory is valid"),
            track,
        }
    }

    fn resolver(result: Result<TrackId, DiagnosticText>) -> Arc<RecordingResolver> {
        Arc::new(RecordingResolver {
            required_result: result,
            views_result: Ok(TrackViewsScope::Track(track_id("active-track"))),
            required_calls: Mutex::new(0),
        })
    }

    fn operation(result: Result<(), TaskOperationError>) -> Arc<RecordingOperation> {
        Arc::new(RecordingOperation { calls: Mutex::new(Vec::new()), result })
    }

    #[test]
    fn test_track_clear_override_interactor_explicit_selection_clears_and_syncs_views() {
        let operation = operation(Ok(()));
        let resolver = resolver(Ok(track_id("active-track")));
        let interactor = TrackClearOverrideInteractor::new(
            operation.clone(),
            resolver.clone(),
            Arc::new(RecordingViews),
        );

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
            .expect("clear override succeeds");

        assert_eq!(result.track_id.as_ref(), "explicit-track");
        assert_eq!(*resolver.required_calls.lock().expect("resolver lock is available"), 0);
        assert_eq!(
            operation
                .calls
                .lock()
                .expect("operation lock is available")
                .first()
                .expect("operation was called")
                .as_ref(),
            "explicit-track"
        );
        assert!(matches!(result.view_sync, TrackViewSyncOutcome::Synchronized(_)));
    }

    #[test]
    fn test_track_clear_override_interactor_active_selection_resolves_before_clearing() {
        let operation = operation(Ok(()));
        let resolver = resolver(Ok(track_id("active-track")));
        let interactor = TrackClearOverrideInteractor::new(
            operation.clone(),
            resolver.clone(),
            Arc::new(RecordingViews),
        );

        interactor
            .execute(command(TrackSelection::Active))
            .expect("active clear override succeeds");

        assert_eq!(*resolver.required_calls.lock().expect("resolver lock is available"), 1);
        assert_eq!(
            operation
                .calls
                .lock()
                .expect("operation lock is available")
                .first()
                .expect("operation was called")
                .as_ref(),
            "active-track"
        );
    }

    #[test]
    fn test_track_clear_override_interactor_resolver_failure_returns_error_without_operation() {
        let operation = operation(Ok(()));
        let resolver = resolver(Err(DiagnosticText::new("active track unavailable")));
        let interactor = TrackClearOverrideInteractor::new(
            operation.clone(),
            resolver,
            Arc::new(RecordingViews),
        );

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("resolver failure must stop the operation"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "active track unavailable");
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }
}
