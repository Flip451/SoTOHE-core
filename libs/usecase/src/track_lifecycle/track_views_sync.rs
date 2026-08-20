use std::sync::Arc;

use crate::git_workflow::DiagnosticText;

use super::{
    RenderedViewPath, TrackSelection, TrackSelectionPort, TrackViewsPort, TrackWorkspaceRoot,
};

/// Validated command for synchronizing rendered track views.
#[derive(Debug, PartialEq, Eq)]
pub struct TrackViewsSyncCommand {
    /// Workspace root that owns `track/`.
    pub workspace_root: TrackWorkspaceRoot,
    /// Explicit or active track selection that becomes the view scope.
    pub scope: TrackSelection,
}

/// Presentation-free result of synchronizing rendered views.
#[derive(Debug, PartialEq, Eq)]
pub enum TrackViewsSyncResult {
    /// Every requested view was already current.
    AlreadyCurrent,
    /// One or more views were rewritten.
    Rendered(Vec<RenderedViewPath>),
}

/// Error returned by the views-sync command boundary.
#[derive(Debug)]
pub enum TrackViewsSyncError {
    /// Selection mapping or view synchronization failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackViewsSyncError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackViewsSyncError {}

/// Application service for synchronizing rendered track views.
pub trait TrackViewsSyncService: Send + Sync {
    /// Maps the selection to a view scope and synchronizes those views.
    fn execute(
        &self,
        command: TrackViewsSyncCommand,
    ) -> Result<TrackViewsSyncResult, TrackViewsSyncError>;
}

/// Interactor for the views-sync command context.
pub struct TrackViewsSyncInteractor {
    views: Arc<dyn TrackViewsPort>,
    resolver: Arc<dyn TrackSelectionPort>,
}

impl TrackViewsSyncInteractor {
    /// Creates an interactor from the views and selection ports.
    #[must_use]
    pub fn new(views: Arc<dyn TrackViewsPort>, resolver: Arc<dyn TrackSelectionPort>) -> Self {
        Self { views, resolver }
    }
}

impl TrackViewsSyncService for TrackViewsSyncInteractor {
    fn execute(
        &self,
        command: TrackViewsSyncCommand,
    ) -> Result<TrackViewsSyncResult, TrackViewsSyncError> {
        let TrackViewsSyncCommand { workspace_root, scope } = command;
        let views_scope = self
            .resolver
            .resolve_views_scope(&workspace_root, &scope)
            .map_err(|error| execution_failed(error.to_string()))?;
        let rendered = self
            .views
            .sync(&workspace_root, &views_scope)
            .map_err(|error| execution_failed(error.to_string()))?;
        if rendered.is_empty() {
            Ok(TrackViewsSyncResult::AlreadyCurrent)
        } else {
            Ok(TrackViewsSyncResult::Rendered(rendered))
        }
    }
}

fn execution_failed(error: impl Into<String>) -> TrackViewsSyncError {
    TrackViewsSyncError::ExecutionFailed(DiagnosticText::new(error))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use domain::TrackId;

    use super::*;
    use crate::track_lifecycle::{TrackItemsDirectory, TrackViewsScope};

    struct RecordingResolver {
        views_result: Result<TrackViewsScope, DiagnosticText>,
        views_calls: Mutex<usize>,
    }

    impl TrackSelectionPort for RecordingResolver {
        fn resolve_required(
            &self,
            _items_dir: &TrackItemsDirectory,
            _selection: &TrackSelection,
        ) -> Result<TrackId, DiagnosticText> {
            panic!("views sync must not resolve a required track id")
        }

        fn resolve_active(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
        ) -> Result<TrackId, DiagnosticText> {
            panic!("views sync must map Active through resolve_views_scope")
        }

        fn resolve_views_scope(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            _selection: &TrackSelection,
        ) -> Result<TrackViewsScope, DiagnosticText> {
            *self.views_calls.lock().expect("resolver lock is available") += 1;
            self.views_result.clone()
        }
    }

    struct RecordingViews {
        result: Result<Vec<RenderedViewPath>, DiagnosticText>,
        scopes: Mutex<Vec<TrackViewsScope>>,
    }

    impl TrackViewsPort for RecordingViews {
        fn validate(&self, _workspace_root: &TrackWorkspaceRoot) -> Result<(), DiagnosticText> {
            Ok(())
        }

        fn sync(
            &self,
            _workspace_root: &TrackWorkspaceRoot,
            scope: &TrackViewsScope,
        ) -> Result<Vec<RenderedViewPath>, DiagnosticText> {
            self.scopes.lock().expect("views lock is available").push(scope.clone());
            match &self.result {
                Ok(paths) => Ok(paths
                    .iter()
                    .map(|path| RenderedViewPath::new(path.as_path().to_path_buf()))
                    .collect()),
                Err(error) => Err(DiagnosticText::new(error.as_str())),
            }
        }
    }

    fn track_id(value: &str) -> TrackId {
        TrackId::try_new(value).expect("track id is valid")
    }

    fn workspace_root() -> TrackWorkspaceRoot {
        TrackWorkspaceRoot::try_new(PathBuf::from("workspace")).expect("workspace root is valid")
    }

    fn command(scope: TrackSelection) -> TrackViewsSyncCommand {
        TrackViewsSyncCommand { workspace_root: workspace_root(), scope }
    }

    #[test]
    fn test_track_views_sync_interactor_explicit_selection_syncs_track_scope() {
        let views = Arc::new(RecordingViews {
            result: Ok(vec![RenderedViewPath::new(PathBuf::from("workspace/track/registry.md"))]),
            scopes: Mutex::new(Vec::new()),
        });
        let resolver = Arc::new(RecordingResolver {
            views_result: Ok(TrackViewsScope::Track(track_id("explicit-track"))),
            views_calls: Mutex::new(0),
        });
        let interactor = TrackViewsSyncInteractor::new(views.clone(), resolver.clone());

        let result = interactor
            .execute(command(TrackSelection::Explicit(track_id("explicit-track"))))
            .expect("explicit views sync succeeds");

        assert_eq!(*resolver.views_calls.lock().expect("resolver lock is available"), 1);
        assert_eq!(
            views.scopes.lock().expect("views lock is available").as_slice(),
            &[TrackViewsScope::Track(track_id("explicit-track"))]
        );
        assert!(matches!(result, TrackViewsSyncResult::Rendered(paths) if paths.len() == 1));
    }

    #[test]
    fn test_track_views_sync_interactor_active_selection_uses_resolver_scope() {
        let views = Arc::new(RecordingViews {
            result: Ok(vec![RenderedViewPath::new(PathBuf::from("workspace/track/registry.md"))]),
            scopes: Mutex::new(Vec::new()),
        });
        let resolver = Arc::new(RecordingResolver {
            views_result: Ok(TrackViewsScope::Track(track_id("active-track"))),
            views_calls: Mutex::new(0),
        });
        let interactor = TrackViewsSyncInteractor::new(views.clone(), resolver.clone());

        interactor.execute(command(TrackSelection::Active)).expect("active views sync succeeds");

        assert_eq!(*resolver.views_calls.lock().expect("resolver lock is available"), 1);
        assert_eq!(
            views.scopes.lock().expect("views lock is available").as_slice(),
            &[TrackViewsScope::Track(track_id("active-track"))]
        );
    }

    #[test]
    fn test_track_views_sync_interactor_empty_sync_returns_already_current() {
        let views =
            Arc::new(RecordingViews { result: Ok(Vec::new()), scopes: Mutex::new(Vec::new()) });
        let resolver = Arc::new(RecordingResolver {
            views_result: Ok(TrackViewsScope::RegistryOnly),
            views_calls: Mutex::new(0),
        });
        let interactor = TrackViewsSyncInteractor::new(views, resolver);

        let result =
            interactor.execute(command(TrackSelection::Active)).expect("empty views sync succeeds");

        assert_eq!(result, TrackViewsSyncResult::AlreadyCurrent);
    }

    #[test]
    fn test_track_views_sync_interactor_resolver_failure_returns_error_without_sync() {
        let views =
            Arc::new(RecordingViews { result: Ok(Vec::new()), scopes: Mutex::new(Vec::new()) });
        let resolver = RecordingResolver {
            views_result: Err(DiagnosticText::new("active track unavailable")),
            views_calls: Mutex::new(0),
        };
        let interactor = TrackViewsSyncInteractor::new(views.clone(), Arc::new(resolver));

        let result = interactor.execute(command(TrackSelection::Active));

        assert!(matches!(
            result,
            Err(TrackViewsSyncError::ExecutionFailed(message))
                if message.as_str() == "active track unavailable"
        ));
        assert!(views.scopes.lock().expect("views lock is available").is_empty());
    }

    #[test]
    fn test_track_views_sync_interactor_sync_failure_returns_execution_error() {
        let views = RecordingViews {
            result: Err(DiagnosticText::new("render failed")),
            scopes: Mutex::new(Vec::new()),
        };
        let resolver = RecordingResolver {
            views_result: Ok(TrackViewsScope::RegistryOnly),
            views_calls: Mutex::new(0),
        };
        let interactor = TrackViewsSyncInteractor::new(Arc::new(views), Arc::new(resolver));

        let result = interactor.execute(command(TrackSelection::Active));

        assert!(matches!(
            result,
            Err(TrackViewsSyncError::ExecutionFailed(message)) if message.as_str() == "render failed"
        ));
    }

    #[test]
    fn test_track_views_sync_command_context_colocates_boundary_types() {
        let source = include_str!("track_views_sync.rs");
        assert!(source.contains("pub struct TrackViewsSyncCommand"));
        assert!(source.contains("pub enum TrackViewsSyncError"));
        assert!(source.contains("pub enum TrackViewsSyncResult"));
        assert!(source.contains("pub trait TrackViewsSyncService"));
        assert!(source.contains("impl TrackViewsSyncService for TrackViewsSyncInteractor"));
    }
}
