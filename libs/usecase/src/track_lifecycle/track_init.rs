//! Application boundary for initializing a track.

use std::path::Path;
use std::sync::Arc;

use domain::{NonEmptyString, TrackId};

use crate::git_workflow::DiagnosticText;

use super::{
    TrackBranchStrategyPort, TrackItemsDirectory, TrackLifecycleIdInput, TrackMetadataPort,
    TrackViewsPort, TrackViewsScope, TrackWorkspaceRoot,
};

/// Typed input for creating a track's initial metadata.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackInitCommand {
    /// Directory containing track item directories.
    pub items_dir: TrackItemsDirectory,
    /// Validated track identity.
    pub track_id: TrackId,
    /// Validated track title.
    pub title: NonEmptyString,
}

impl TrackInitCommand {
    /// Validates the primary-adapter inputs and creates an initialization command.
    pub fn try_new(
        items_dir: TrackItemsDirectory,
        track_id: TrackLifecycleIdInput,
        title: String,
    ) -> Result<Self, TrackInitError> {
        let title = NonEmptyString::try_new(title).map_err(|error| {
            TrackInitError::ExecutionFailed(DiagnosticText::new(format!(
                "invalid track title: {error}"
            )))
        })?;
        Ok(Self { items_dir, track_id: track_id.into_track_id(), title })
    }
}

/// Presentation-free result of track initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrackInitResult;

/// Error returned by the track-initialization boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrackInitError {
    /// A persistence or rendered-view operation failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackInitError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackInitError {}

/// Application service for initializing a track.
pub trait TrackInitService: Send + Sync {
    /// Persists initial metadata and synchronizes the rendered views.
    fn execute(&self, command: TrackInitCommand) -> Result<TrackInitResult, TrackInitError>;
}

/// Interactor for the track-initialization command context.
pub struct TrackInitInteractor {
    metadata: Arc<dyn TrackMetadataPort>,
    branch_strategy: Arc<dyn TrackBranchStrategyPort>,
    views: Arc<dyn TrackViewsPort>,
}

impl TrackInitInteractor {
    /// Creates an interactor from the metadata, strategy, and view ports.
    #[must_use]
    pub fn new(
        metadata: Arc<dyn TrackMetadataPort>,
        branch_strategy: Arc<dyn TrackBranchStrategyPort>,
        views: Arc<dyn TrackViewsPort>,
    ) -> Self {
        Self { metadata, branch_strategy, views }
    }
}

impl TrackInitService for TrackInitInteractor {
    fn execute(&self, command: TrackInitCommand) -> Result<TrackInitResult, TrackInitError> {
        let snapshot =
            self.branch_strategy.global_for_items(&command.items_dir).map_err(execution_failed)?;
        let workspace_root = workspace_root_for_items(&command.items_dir)?;
        let track_id = command.track_id.clone();
        let metadata =
            domain::TrackMetadata::new(track_id.clone(), command.title.to_string(), None, snapshot)
                .map_err(|error| {
                    execution_failed(DiagnosticText::new(format!(
                        "invalid track metadata: {error}"
                    )))
                })?;

        self.metadata
            .save(&command.items_dir, metadata)
            .map_err(|error| execution_failed(with_context("init failed", error)))?;
        self.views
            .sync(&workspace_root, &TrackViewsScope::Track(track_id))
            .map_err(|error| execution_failed(with_context("sync-views failed", error)))?;

        Ok(TrackInitResult)
    }
}

fn workspace_root_for_items(
    items_dir: &TrackItemsDirectory,
) -> Result<TrackWorkspaceRoot, TrackInitError> {
    let track_dir = items_dir.as_path().parent().ok_or_else(|| {
        execution_failed(DiagnosticText::new("track items directory has no track parent"))
    })?;
    let root = track_dir.parent().ok_or_else(|| {
        execution_failed(DiagnosticText::new("track items directory has no workspace root"))
    })?;
    let root = if root.as_os_str().is_empty() { Path::new(".") } else { root };
    TrackWorkspaceRoot::try_new(root.to_path_buf()).map_err(execution_failed)
}

fn with_context(context: &str, error: DiagnosticText) -> DiagnosticText {
    DiagnosticText::new(format!("{context}: {error}"))
}

fn execution_failed(error: DiagnosticText) -> TrackInitError {
    TrackInitError::ExecutionFailed(error)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Mutex;

    use domain::{BranchStrategySnapshot, MergeMethod, TrackMetadata};

    use super::*;

    fn snapshot() -> BranchStrategySnapshot {
        BranchStrategySnapshot::new(
            NonEmptyString::try_new("main").expect("base branch is valid"),
            NonEmptyString::try_new("main").expect("merge target is valid"),
            MergeMethod::Merge,
        )
    }

    fn items_dir() -> TrackItemsDirectory {
        TrackItemsDirectory::try_new(PathBuf::from("workspace/track/items"))
            .expect("items directory is valid")
    }

    fn command(title: &str) -> TrackInitCommand {
        TrackInitCommand::try_new(
            items_dir(),
            TrackLifecycleIdInput::try_new("new-track".to_owned()).expect("track id is valid"),
            title.to_owned(),
        )
        .expect("command is valid")
    }

    struct RecordingBranchStrategy {
        error: Option<DiagnosticText>,
    }

    impl TrackBranchStrategyPort for RecordingBranchStrategy {
        fn global_for_items(
            &self,
            items_dir: &TrackItemsDirectory,
        ) -> Result<BranchStrategySnapshot, DiagnosticText> {
            assert_eq!(items_dir.as_path(), Path::new("workspace/track/items"));
            if let Some(error) = &self.error {
                return Err(DiagnosticText::new(error.to_string()));
            }
            Ok(snapshot())
        }

        fn snapshot_for_track(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _track_id: &TrackId,
        ) -> Result<BranchStrategySnapshot, DiagnosticText> {
            if let Some(error) = &self.error {
                return Err(DiagnosticText::new(error.to_string()));
            }
            Ok(snapshot())
        }
    }

    struct RecordingMetadata {
        saved: Mutex<Vec<TrackMetadata>>,
        error: Option<DiagnosticText>,
    }

    impl TrackMetadataPort for RecordingMetadata {
        fn save(
            &self,
            _items_dir: &TrackItemsDirectory,
            metadata: TrackMetadata,
        ) -> Result<(), DiagnosticText> {
            if let Some(error) = &self.error {
                return Err(DiagnosticText::new(error.to_string()));
            }
            self.saved.lock().expect("metadata lock is available").push(metadata);
            Ok(())
        }

        fn find(
            &self,
            _items_dir: &TrackItemsDirectory,
            _track_id: &TrackId,
        ) -> Result<Option<TrackMetadata>, DiagnosticText> {
            Ok(None)
        }
    }

    struct RecordingViews {
        scopes: Mutex<Vec<TrackViewsScope>>,
        error: Option<DiagnosticText>,
    }

    impl TrackViewsPort for RecordingViews {
        fn validate(&self, _workspace_root: &TrackWorkspaceRoot) -> Result<(), DiagnosticText> {
            Ok(())
        }

        fn sync(
            &self,
            workspace_root: &TrackWorkspaceRoot,
            scope: &TrackViewsScope,
        ) -> Result<Vec<super::super::RenderedViewPath>, DiagnosticText> {
            assert_eq!(workspace_root.as_path(), Path::new("workspace"));
            if let Some(error) = &self.error {
                return Err(DiagnosticText::new(error.to_string()));
            }
            self.scopes.lock().expect("view lock is available").push(scope.clone());
            Ok(Vec::new())
        }
    }

    #[test]
    fn test_track_init_command_try_new_empty_title_returns_error() {
        let result = TrackInitCommand::try_new(
            items_dir(),
            TrackLifecycleIdInput::try_new("new-track".to_owned()).expect("track id is valid"),
            "   ".to_owned(),
        );

        assert!(
            matches!(result, Err(TrackInitError::ExecutionFailed(error)) if error.to_string().contains("track title"))
        );
    }

    #[test]
    fn test_track_init_interactor_valid_command_saves_metadata_and_syncs_track_views() {
        let metadata = Arc::new(RecordingMetadata { saved: Mutex::new(Vec::new()), error: None });
        let views = Arc::new(RecordingViews { scopes: Mutex::new(Vec::new()), error: None });
        let interactor = TrackInitInteractor::new(
            metadata.clone(),
            Arc::new(RecordingBranchStrategy { error: None }),
            views.clone(),
        );

        interactor.execute(command("New Track")).expect("initialization succeeds");

        assert_eq!(metadata.saved.lock().expect("metadata lock is available").len(), 1);
        assert_eq!(
            views.scopes.lock().expect("view lock is available").as_slice(),
            &[TrackViewsScope::Track(TrackId::try_new("new-track").expect("track id is valid"))]
        );
    }

    #[test]
    fn test_track_init_interactor_metadata_failure_maps_to_execution_error() {
        let interactor = TrackInitInteractor::new(
            Arc::new(RecordingMetadata {
                saved: Mutex::new(Vec::new()),
                error: Some(DiagnosticText::new("disk full")),
            }),
            Arc::new(RecordingBranchStrategy { error: None }),
            Arc::new(RecordingViews { scopes: Mutex::new(Vec::new()), error: None }),
        );

        let error = interactor.execute(command("New Track")).expect_err("save must fail");

        assert_eq!(error.to_string(), "init failed: disk full");
    }

    #[test]
    fn test_track_init_interactor_branch_strategy_failure_maps_to_execution_error() {
        let interactor = TrackInitInteractor::new(
            Arc::new(RecordingMetadata { saved: Mutex::new(Vec::new()), error: None }),
            Arc::new(RecordingBranchStrategy {
                error: Some(DiagnosticText::new("config missing")),
            }),
            Arc::new(RecordingViews { scopes: Mutex::new(Vec::new()), error: None }),
        );

        let error = interactor.execute(command("New Track")).expect_err("strategy must fail");

        assert_eq!(error.to_string(), "config missing");
    }

    #[test]
    fn test_track_init_interactor_view_failure_maps_to_execution_error() {
        let interactor = TrackInitInteractor::new(
            Arc::new(RecordingMetadata { saved: Mutex::new(Vec::new()), error: None }),
            Arc::new(RecordingBranchStrategy { error: None }),
            Arc::new(RecordingViews {
                scopes: Mutex::new(Vec::new()),
                error: Some(DiagnosticText::new("view write failed")),
            }),
        );

        let error = interactor.execute(command("New Track")).expect_err("view sync must fail");

        assert_eq!(error.to_string(), "sync-views failed: view write failed");
    }
}
