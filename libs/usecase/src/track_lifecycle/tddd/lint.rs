//! Shared value objects and the single-layer Track lint command context.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::TrackId;

use crate::git_workflow::DiagnosticText;

use super::super::{TrackSelection, TrackSelectionPort, TrackWorkspaceRoot};
use super::validate_non_traversing_path;

/// A validated path to a catalogue-lint rules file.
#[derive(PartialEq, Eq)]
pub struct TrackLintRulesFile(PathBuf);

impl TrackLintRulesFile {
    /// Validates and wraps a lint-rules-file path.
    pub fn try_new(value: PathBuf) -> Result<Self, DiagnosticText> {
        validate_non_traversing_path(&value, "track lint rules file")?;
        Ok(Self(value))
    }

    /// Returns the wrapped path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

/// Typed input for linting one catalogue layer.
pub struct TrackLintCommand {
    /// Track selection supplied by the primary adapter.
    pub track: TrackSelection,
    /// Workspace containing the track and its TDDD configuration.
    pub workspace_root: TrackWorkspaceRoot,
    /// The single layer to lint.
    pub layer: domain::tddd::LayerId,
    /// Optional override for the lint rules file.
    pub rules_file: Option<TrackLintRulesFile>,
}

/// Presentation-free result of a single-layer catalogue lint.
pub struct TrackLintResult {
    /// Violations reported for the layer.
    pub violations: Vec<domain::CatalogueLintViolation>,
}

/// Error returned by the single-layer catalogue-lint boundary.
#[derive(Debug)]
pub enum TrackLintError {
    /// The catalogue-lint operation could not be completed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackLintError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackLintError {}

/// Secondary port for the blocking single-layer catalogue-lint operation.
pub trait TrackLintPort: Send + Sync {
    /// Runs the existing catalogue-lint workflow for a resolved track and layer.
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackLintCommand,
    ) -> Result<TrackLintResult, TrackLintError>;
}

/// Application service for single-layer catalogue linting.
pub trait TrackLintService: Send + Sync {
    /// Resolves the selection and runs the catalogue-lint operation.
    fn execute(&self, command: TrackLintCommand) -> Result<TrackLintResult, TrackLintError>;
}

/// Interactor for the single-layer catalogue-lint command context.
pub struct TrackLintInteractor {
    operation: Arc<dyn TrackLintPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackLintInteractor {
    /// Creates an interactor from the lint operation and selection resolver.
    #[must_use]
    pub fn new(operation: Arc<dyn TrackLintPort>, resolver: Arc<dyn TrackSelectionPort>) -> Self {
        Self { operation, resolver }
    }
}

impl TrackLintService for TrackLintInteractor {
    fn execute(&self, command: TrackLintCommand) -> Result<TrackLintResult, TrackLintError> {
        let track_id = match &command.track {
            TrackSelection::Explicit(track_id) => track_id.clone(),
            TrackSelection::Active => {
                self.resolver.resolve_active(&command.workspace_root).map_err(execution_failed)?
            }
        };
        self.operation.execute(track_id, command)
    }
}

fn execution_failed(error: DiagnosticText) -> TrackLintError {
    TrackLintError::ExecutionFailed(error)
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

    struct RecordingLint {
        calls: Mutex<Vec<TrackId>>,
    }

    impl TrackLintPort for RecordingLint {
        fn execute(
            &self,
            track_id: TrackId,
            _command: TrackLintCommand,
        ) -> Result<TrackLintResult, TrackLintError> {
            self.calls.lock().expect("lint lock is available").push(track_id);
            Ok(TrackLintResult { violations: Vec::new() })
        }
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace is valid")
    }

    fn command(track: TrackSelection) -> TrackLintCommand {
        TrackLintCommand {
            track,
            workspace_root: workspace_root(),
            layer: domain::tddd::LayerId::try_new("domain".to_owned()).expect("layer is valid"),
            rules_file: None,
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    #[test]
    fn test_track_lint_rules_file_try_new_rejects_parent_relative_path() {
        let error = match TrackLintRulesFile::try_new(PathBuf::from("../rules.json")) {
            Ok(_) => panic!("parent-relative rules path must fail"),
            Err(error) => error,
        };
        assert!(error.to_string().contains("track lint rules file"));
    }

    #[test]
    fn test_track_lint_interactor_explicit_selection_forwards_without_resolution() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("active-track")),
            calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingLint { calls: Mutex::new(Vec::new()) });
        let interactor = TrackLintInteractor::new(operation.clone(), resolver.clone());
        interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
            .expect("explicit lint succeeds");
        let calls = operation.calls.lock().expect("lint lock is available");
        assert_eq!(
            calls.first().expect("one operation call is recorded").as_ref(),
            "explicit-track"
        );
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 0);
    }

    #[test]
    fn test_track_lint_interactor_active_selection_uses_resolver() {
        let resolver = Arc::new(RecordingResolver {
            active: Ok(track_id("lint-track")),
            calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingLint { calls: Mutex::new(Vec::new()) });
        let interactor = TrackLintInteractor::new(operation.clone(), resolver.clone());
        let result = interactor.execute(command(TrackSelection::Active)).expect("lint succeeds");
        assert!(result.violations.is_empty());
        assert_eq!(
            operation
                .calls
                .lock()
                .expect("lint lock is available")
                .first()
                .expect("one call")
                .as_ref(),
            "lint-track"
        );
        assert_eq!(*resolver.calls.lock().expect("resolver lock is available"), 1);
    }

    #[test]
    fn test_track_lint_interactor_resolution_failure_returns_execution_error() {
        let resolver = Arc::new(RecordingResolver {
            active: Err(DiagnosticText::new("active track unavailable")),
            calls: Mutex::new(0),
        });
        let operation = Arc::new(RecordingLint { calls: Mutex::new(Vec::new()) });
        let interactor = TrackLintInteractor::new(operation.clone(), resolver);
        let error = match interactor.execute(command(TrackSelection::Active)) {
            Ok(_) => panic!("selection failure must propagate"),
            Err(error) => error,
        };
        assert_eq!(error.to_string(), "active track unavailable");
        assert!(operation.calls.lock().expect("lint lock is available").is_empty());
    }
}
