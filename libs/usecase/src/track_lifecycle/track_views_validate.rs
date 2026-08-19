use std::sync::Arc;

use crate::git_workflow::DiagnosticText;

use super::{TrackViewsPort, TrackWorkspaceRoot};

/// Validated command for validating rendered track views.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackViewsValidateCommand {
    /// Workspace root that owns `track/`.
    pub workspace_root: TrackWorkspaceRoot,
}

/// Presentation-free result of validating rendered views.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackViewsValidateResult;

/// Error returned by the views-validate command boundary.
#[derive(Debug)]
pub enum TrackViewsValidateError {
    /// View validation failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackViewsValidateError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackViewsValidateError {}

/// Application service for validating rendered track views.
pub trait TrackViewsValidateService: Send + Sync {
    /// Validates rendered views below the workspace root.
    fn execute(
        &self,
        command: TrackViewsValidateCommand,
    ) -> Result<TrackViewsValidateResult, TrackViewsValidateError>;
}

/// Interactor for the views-validate command context.
pub struct TrackViewsValidateInteractor {
    views: Arc<dyn TrackViewsPort>,
}

impl TrackViewsValidateInteractor {
    /// Creates an interactor from the views port.
    #[must_use]
    pub fn new(views: Arc<dyn TrackViewsPort>) -> Self {
        Self { views }
    }
}

impl TrackViewsValidateService for TrackViewsValidateInteractor {
    fn execute(
        &self,
        command: TrackViewsValidateCommand,
    ) -> Result<TrackViewsValidateResult, TrackViewsValidateError> {
        self.views
            .validate(&command.workspace_root)
            .map(|()| TrackViewsValidateResult)
            .map_err(|error| execution_failed(error.to_string()))
    }
}

fn execution_failed(error: impl Into<String>) -> TrackViewsValidateError {
    TrackViewsValidateError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::{RenderedViewPath, TrackViewsScope};

    struct RecordingViews {
        result: Result<(), DiagnosticText>,
        validated: Mutex<Vec<PathBuf>>,
        synced: Mutex<usize>,
    }

    impl TrackViewsPort for RecordingViews {
        fn validate(&self, workspace_root: &TrackWorkspaceRoot) -> Result<(), DiagnosticText> {
            self.validated
                .lock()
                .expect("views lock is available")
                .push(workspace_root.as_path().to_path_buf());
            match &self.result {
                Ok(()) => Ok(()),
                Err(error) => Err(DiagnosticText::new(error.as_str())),
            }
        }

        fn sync(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _scope: &TrackViewsScope,
        ) -> Result<Vec<RenderedViewPath>, DiagnosticText> {
            *self.synced.lock().expect("sync lock is available") += 1;
            panic!("views validate must not synchronize rendered views")
        }
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace root is valid")
    }

    fn command() -> TrackViewsValidateCommand {
        TrackViewsValidateCommand { workspace_root: workspace_root() }
    }

    #[test]
    fn test_track_views_validate_interactor_success_calls_validate_only() {
        let views = Arc::new(RecordingViews {
            result: Ok(()),
            validated: Mutex::new(Vec::new()),
            synced: Mutex::new(0),
        });
        let interactor = TrackViewsValidateInteractor::new(views.clone());

        let result = interactor.execute(command()).expect("views validate succeeds");

        assert_eq!(result, TrackViewsValidateResult);
        assert_eq!(
            views.validated.lock().expect("views lock is available").as_slice(),
            &[PathBuf::from("workspace")]
        );
        assert_eq!(*views.synced.lock().expect("sync lock is available"), 0);
    }

    #[test]
    fn test_track_views_validate_interactor_port_failure_returns_execution_error() {
        let views = RecordingViews {
            result: Err(DiagnosticText::new("invalid metadata")),
            validated: Mutex::new(Vec::new()),
            synced: Mutex::new(0),
        };
        let interactor = TrackViewsValidateInteractor::new(Arc::new(views));

        let result = interactor.execute(command());

        assert!(matches!(
            result,
            Err(TrackViewsValidateError::ExecutionFailed(message))
                if message.as_str() == "invalid metadata"
        ));
    }

    #[test]
    fn test_track_views_validate_command_context_colocates_boundary_types() {
        let source = include_str!("track_views_validate.rs");
        assert!(source.contains("pub struct TrackViewsValidateCommand"));
        assert!(source.contains("pub enum TrackViewsValidateError"));
        assert!(source.contains("pub struct TrackViewsValidateResult"));
        assert!(source.contains("pub trait TrackViewsValidateService"));
        assert!(source.contains("impl TrackViewsValidateService for TrackViewsValidateInteractor"));
    }
}
