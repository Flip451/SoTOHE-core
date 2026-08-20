//! Application boundary for linting every active-track catalogue layer.

use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::lint::TrackLintRulesFile;

/// Typed input for the active-track catalogue-lint operation.
pub struct TrackCatalogueLintActiveCommand {
    /// Track selection supplied by the primary adapter.
    pub track: TrackSelection,
    /// Workspace containing the track and its TDDD configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// Optional override for the lint rules file.
    pub rules_file: Option<TrackLintRulesFile>,
}

/// A single catalogue-lint layer result.
pub struct TrackCatalogueLintLayerResult {
    /// The TDDD layer that was evaluated.
    pub layer: domain::tddd::LayerId,
    /// Violations reported for the layer.
    pub violations: Vec<domain::CatalogueLintViolation>,
}

/// Presentation-free result of active-track catalogue linting.
pub enum TrackCatalogueLintActiveResult {
    /// Every enabled layer was evaluated.
    Checked { layers: Vec<TrackCatalogueLintLayerResult> },
    /// Linting was skipped because a layer catalogue is not materialized yet.
    Skipped { layer: domain::tddd::LayerId, path: super::super::TrackCataloguePath },
}

/// Error returned by the active-track catalogue-lint boundary.
#[derive(Debug)]
pub enum TrackCatalogueLintActiveError {
    /// The catalogue-lint operation could not be completed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackCatalogueLintActiveError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackCatalogueLintActiveError {}

/// Secondary port for the blocking active-track catalogue-lint operation.
pub trait TrackCatalogueLintActivePort: Send + Sync {
    /// Runs the existing catalogue-lint workflow for a resolved track.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackCatalogueLintActiveCommand,
    ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError>;
}

/// Application service for active-track catalogue linting.
pub trait TrackCatalogueLintActiveService: Send + Sync {
    /// Resolves the selection and runs the catalogue-lint operation.
    fn execute(
        &self,
        command: TrackCatalogueLintActiveCommand,
    ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError>;
}

/// Interactor for the active-track catalogue-lint command context.
pub struct TrackCatalogueLintActiveInteractor {
    operation: Arc<dyn TrackCatalogueLintActivePort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackCatalogueLintActiveInteractor {
    /// Creates an interactor from the lint operation and selection resolver.
    #[must_use]
    pub fn new(
        operation: Arc<dyn TrackCatalogueLintActivePort>,
        resolver: Arc<dyn TrackSelectionPort>,
    ) -> Self {
        Self { operation, resolver }
    }
}

impl TrackCatalogueLintActiveService for TrackCatalogueLintActiveInteractor {
    fn execute(
        &self,
        command: TrackCatalogueLintActiveCommand,
    ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> {
        let track_id = match &command.track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => {
                self.resolver.resolve_active(&command.workspace_root).map_err(execution_failed)?
            }
        };
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackCatalogueLintActiveError {
    TrackCatalogueLintActiveError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use super::*;
    use crate::track_lifecycle::{TrackItemsDirectory, TrackViewsScope};

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
            self.active.clone()
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            *self.calls.lock().expect("resolver lock is available") += 1;
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

    struct RecordingOperation {
        calls: Mutex<Vec<TrackId>>,
        result: Result<TrackCatalogueLintActiveResult, String>,
    }

    impl TrackCatalogueLintActivePort for RecordingOperation {
        fn execute(
            &self,
            track_id: TrackId,
            _command: TrackCatalogueLintActiveCommand,
        ) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> {
            self.calls.lock().expect("operation lock is available").push(track_id);
            match &self.result {
                Ok(TrackCatalogueLintActiveResult::Checked { layers }) => {
                    Ok(TrackCatalogueLintActiveResult::Checked {
                        layers: layers
                            .iter()
                            .map(|layer| TrackCatalogueLintLayerResult {
                                layer: layer.layer.clone(),
                                violations: layer.violations.clone(),
                            })
                            .collect(),
                    })
                }
                Ok(TrackCatalogueLintActiveResult::Skipped { layer, path }) => {
                    Ok(TrackCatalogueLintActiveResult::Skipped {
                        layer: layer.clone(),
                        path: super::super::TrackCataloguePath::try_new(
                            path.as_path().to_path_buf(),
                        )
                        .expect("catalogue path is valid"),
                    })
                }
                Err(error) => {
                    Err(TrackCatalogueLintActiveError::ExecutionFailed(DiagnosticText::new(error)))
                }
            }
        }
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace is valid")
    }

    fn command(track: TrackSelection) -> TrackCatalogueLintActiveCommand {
        TrackCatalogueLintActiveCommand {
            track,
            workspace_root: workspace_root(),
            rules_file: None,
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    #[test]
    fn test_track_catalogue_lint_active_interactor_explicit_selection_forwards_without_resolution()
    {
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            result: Ok(TrackCatalogueLintActiveResult::Checked { layers: Vec::new() }),
        });
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            calls: Mutex::new(0),
        });
        let interactor =
            TrackCatalogueLintActiveInteractor::new(operation.clone(), resolver.clone());

        interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
            .expect("explicit lint succeeds");

        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls.first().expect("one operation call is recorded").as_ref(),
            "explicit-track"
        );
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 0);
    }

    #[test]
    fn test_track_catalogue_lint_active_interactor_active_selection_resolves_and_forwards() {
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            result: Ok(TrackCatalogueLintActiveResult::Checked { layers: Vec::new() }),
        });
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            calls: Mutex::new(0),
        });
        let interactor =
            TrackCatalogueLintActiveInteractor::new(operation.clone(), resolver.clone());

        interactor.execute(command(TrackSelection::Active)).expect("active lint succeeds");

        let calls = operation.calls.lock().expect("operation lock is available");
        assert_eq!(calls.first().expect("one operation call is recorded").as_ref(), "active-track");
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 1);
    }

    #[test]
    fn test_track_catalogue_lint_active_interactor_resolution_failure_returns_execution_error() {
        let resolver = Arc::new(RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingOperation {
            calls: Mutex::new(Vec::new()),
            result: Ok(TrackCatalogueLintActiveResult::Checked { layers: Vec::new() }),
        });
        let interactor = TrackCatalogueLintActiveInteractor::new(operation.clone(), resolver);

        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };

        assert_eq!(error.to_string(), "active track unavailable");
        assert!(operation.calls.lock().expect("operation lock is available").is_empty());
    }
}
