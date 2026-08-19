//! Views-sync rendering for [`crate::track::TrackDriver`].

use std::path::PathBuf;

use usecase::track_lifecycle::track_views_sync::{
    TrackViewsSyncCommand, TrackViewsSyncError, TrackViewsSyncResult, TrackViewsSyncService,
};
use usecase::track_lifecycle::{TrackLifecycleIdInput, TrackSelection, TrackWorkspaceRoot};

use crate::render::CommandOutcome;

pub(crate) fn render_track_views_sync_outcome(
    service: &dyn TrackViewsSyncService,
    project_root: PathBuf,
    track_id: Option<String>,
) -> CommandOutcome {
    let workspace_root_for_display = project_root.clone();
    let workspace_root = match TrackWorkspaceRoot::try_new(project_root) {
        Ok(workspace_root) => workspace_root,
        Err(error) => return CommandOutcome::failure(Some(format!("[ERROR] {error}"))),
    };
    let scope = match track_id
        .map(TrackLifecycleIdInput::try_new)
        .transpose()
        .map_err(|error| error.to_string())
    {
        Ok(track_id) => TrackSelection::from_input(track_id),
        Err(error) => return invalid_track_id(error),
    };
    service
        .execute(TrackViewsSyncCommand { workspace_root, scope })
        .map(|result| render_track_views_sync_result(result, &workspace_root_for_display))
        .unwrap_or_else(track_views_sync_error_to_outcome)
}

fn render_track_views_sync_result(
    result: TrackViewsSyncResult,
    project_root: &std::path::Path,
) -> CommandOutcome {
    match result {
        TrackViewsSyncResult::AlreadyCurrent => {
            CommandOutcome::success(Some("[OK] All views already up to date".to_owned()))
        }
        TrackViewsSyncResult::Rendered(paths) => {
            let lines = paths
                .into_iter()
                .map(|path| match path.as_path().strip_prefix(project_root) {
                    Ok(relative) => format!("[OK] Rendered: {}", relative.display()),
                    Err(_) => format!("[OK] Rendered: {}", path.as_path().display()),
                })
                .collect::<Vec<_>>();
            CommandOutcome::success(Some(lines.join("\n")))
        }
    }
}

fn track_views_sync_error_to_outcome(error: TrackViewsSyncError) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] sync-views failed: {error}")))
}

fn invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    CommandOutcome::failure(Some(format!("[ERROR] {legacy_error}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::path::PathBuf;
    use std::sync::Mutex;

    use usecase::git_workflow::DiagnosticText;
    use usecase::track_lifecycle::RenderedViewPath;

    use super::*;

    struct RecordingService {
        result: Result<TrackViewsSyncResult, String>,
        calls: Mutex<Vec<(PathBuf, TrackSelection)>>,
    }

    impl TrackViewsSyncService for RecordingService {
        fn execute(
            &self,
            command: TrackViewsSyncCommand,
        ) -> Result<TrackViewsSyncResult, TrackViewsSyncError> {
            self.calls
                .lock()
                .expect("service lock is available")
                .push((command.workspace_root.as_path().to_path_buf(), command.scope.clone()));
            match &self.result {
                Ok(TrackViewsSyncResult::AlreadyCurrent) => {
                    Ok(TrackViewsSyncResult::AlreadyCurrent)
                }
                Ok(TrackViewsSyncResult::Rendered(paths)) => Ok(TrackViewsSyncResult::Rendered(
                    paths
                        .iter()
                        .map(|path| RenderedViewPath::new(path.as_path().to_path_buf()))
                        .collect(),
                )),
                Err(error) => Err(TrackViewsSyncError::ExecutionFailed(DiagnosticText::new(error))),
            }
        }
    }

    #[test]
    fn test_render_track_views_sync_outcome_already_current_preserves_success_contract() {
        let service = RecordingService {
            result: Ok(TrackViewsSyncResult::AlreadyCurrent),
            calls: Mutex::new(Vec::new()),
        };
        let project_root = PathBuf::from("workspace");
        let outcome = render_track_views_sync_outcome(
            &service,
            project_root.clone(),
            Some("views-track".to_owned()),
        );
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("[OK] All views already up to date"));
        assert_eq!(outcome.stderr, None);
        let calls = service.calls.lock().expect("service lock is available");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls.first().map(|(path, _)| path), Some(&project_root));
    }

    #[test]
    fn test_render_track_views_sync_outcome_rendered_strips_workspace_prefix() {
        let service = RecordingService {
            result: Ok(TrackViewsSyncResult::Rendered(vec![
                RenderedViewPath::new(PathBuf::from("workspace/track/registry.md")),
                RenderedViewPath::new(PathBuf::from("workspace/track/items/views-track/plan.md")),
            ])),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_views_sync_outcome(
            &service,
            PathBuf::from("workspace"),
            Some("views-track".to_owned()),
        );
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "[OK] Rendered: track/registry.md\n[OK] Rendered: track/items/views-track/plan.md"
            )
        );
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_render_track_views_sync_outcome_service_error_maps_to_prefixed_failure() {
        let service =
            RecordingService { result: Err("disk full".to_owned()), calls: Mutex::new(Vec::new()) };
        let outcome = render_track_views_sync_outcome(
            &service,
            PathBuf::from("workspace"),
            Some("views-track".to_owned()),
        );
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] sync-views failed: disk full"));
        assert_eq!(outcome.stdout, None);
    }

    #[test]
    fn test_render_track_views_sync_outcome_invalid_track_id_preserves_error_prefix() {
        let service = RecordingService {
            result: Ok(TrackViewsSyncResult::AlreadyCurrent),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_views_sync_outcome(
            &service,
            PathBuf::from("workspace"),
            Some("INVALID".to_owned()),
        );
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] track id 'INVALID' must be a lowercase slug")
        );
        assert_eq!(outcome.stdout, None);
        assert!(service.calls.lock().expect("service lock is available").is_empty());
    }

    #[test]
    fn test_render_track_views_sync_outcome_empty_track_id_preserves_error_prefix() {
        let service = RecordingService {
            result: Ok(TrackViewsSyncResult::AlreadyCurrent),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_views_sync_outcome(
            &service,
            PathBuf::from("workspace"),
            Some("   ".to_owned()),
        );
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] string must not be empty"));
        assert_eq!(outcome.stdout, None);
        assert!(service.calls.lock().expect("service lock is available").is_empty());
    }

    #[test]
    fn test_render_track_views_sync_outcome_explicit_and_active_selection_are_distinguished() {
        let service = RecordingService {
            result: Ok(TrackViewsSyncResult::AlreadyCurrent),
            calls: Mutex::new(Vec::new()),
        };
        let explicit = render_track_views_sync_outcome(
            &service,
            PathBuf::from("workspace"),
            Some("views-track".to_owned()),
        );
        let active = render_track_views_sync_outcome(&service, PathBuf::from("workspace"), None);
        assert_eq!(explicit.exit_code, 0);
        assert_eq!(active.exit_code, 0);
        let calls = service.calls.lock().expect("service lock is available");
        assert_eq!(calls.len(), 2);
        assert!(matches!(
            calls.first().map(|(_, selection)| selection),
            Some(TrackSelection::Explicit(_))
        ));
        assert!(matches!(
            calls.get(1).map(|(_, selection)| selection),
            Some(&TrackSelection::Active)
        ));
    }
}
