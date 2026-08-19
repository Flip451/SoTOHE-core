//! Primary-adapter rendering for the `track set-override` command.

use std::path::{Path, PathBuf};

use usecase::track_lifecycle::track_set_override::{
    TrackSetOverrideCommand, TrackSetOverrideError, TrackSetOverrideResult, TrackSetOverrideService,
};
use usecase::track_lifecycle::{
    RenderedViewPath, TrackItemsDirectory, TrackLifecycleIdInput, TrackSelection,
    TrackViewSyncOutcome,
};

use crate::render::CommandOutcome;

/// Validates set-override input, invokes the service, and renders the legacy CLI result.
pub(crate) fn render_track_set_override_outcome(
    service: &dyn TrackSetOverrideService,
    items_dir: PathBuf,
    track_id: Option<String>,
    status: String,
    reason: String,
) -> CommandOutcome {
    let items_dir_for_error = items_dir.clone();
    let items_dir = match TrackItemsDirectory::try_new(items_dir) {
        Ok(items_dir) => items_dir,
        Err(_) => return invalid_items_dir(&items_dir_for_error),
    };
    let track = match track_id
        .map(TrackLifecycleIdInput::try_new)
        .transpose()
        .map_err(|error| error.to_string())
    {
        Ok(track_id) => TrackSelection::from_input(track_id),
        Err(error) => return invalid_track_id(error),
    };
    let project_root = project_root_for_items(items_dir.as_path());
    let command = match TrackSetOverrideCommand::try_new(items_dir, track, status, reason) {
        Ok(command) => command,
        Err(error) => return error_to_outcome(error),
    };
    service
        .execute(command)
        .map(|result| render_result(result, &project_root))
        .unwrap_or_else(error_to_outcome)
}

fn render_result(result: TrackSetOverrideResult, project_root: &Path) -> CommandOutcome {
    let mut lines = vec![format!(
        "[OK] Override set to '{}' (track status: {})",
        result.status, result.derived_status
    )];
    match result.view_sync {
        TrackViewSyncOutcome::Synchronized(rendered_views) => {
            lines.extend(rendered_view_lines(project_root, rendered_views));
        }
        TrackViewSyncOutcome::Warning { rendered_views, diagnostic } => {
            lines.extend(rendered_view_lines(project_root, rendered_views));
            lines.push(format!("warning: operation persisted but sync-views failed: {diagnostic}"));
        }
    }
    CommandOutcome::success(Some(lines.join("\n")))
}

fn rendered_view_lines(project_root: &Path, rendered_views: Vec<RenderedViewPath>) -> Vec<String> {
    rendered_views
        .into_iter()
        .map(|path| match path.as_path().strip_prefix(project_root) {
            Ok(relative) => format!("[OK] Rendered: {}", relative.display()),
            Err(_) => format!("[OK] Rendered: {}", path.as_path().display()),
        })
        .collect()
}

fn project_root_for_items(items_dir: &Path) -> PathBuf {
    items_dir
        .parent()
        .and_then(Path::parent)
        .filter(|root| !root.as_os_str().is_empty())
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."))
}

fn error_to_outcome(error: TrackSetOverrideError) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

fn invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    error_to_outcome(TrackSetOverrideError::ExecutionFailed(
        usecase::git_workflow::DiagnosticText::new(legacy_error),
    ))
}

fn invalid_items_dir(items_dir: &Path) -> CommandOutcome {
    CommandOutcome::failure(Some(format!(
        "[ERROR] --items-dir must point to '<project-root>/track/items'; got {}",
        items_dir.display()
    )))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use domain::{StatusOverrideKind, TrackId, TrackStatus};

    struct RecordingService {
        calls: Mutex<Vec<TrackSetOverrideCommand>>,
        result: Mutex<Option<Result<TrackSetOverrideResult, TrackSetOverrideError>>>,
    }

    impl TrackSetOverrideService for RecordingService {
        fn execute(
            &self,
            command: TrackSetOverrideCommand,
        ) -> Result<TrackSetOverrideResult, TrackSetOverrideError> {
            self.calls.lock().expect("service lock is available").push(command);
            self.result
                .lock()
                .expect("result lock is available")
                .take()
                .expect("one service result")
        }
    }

    fn result() -> TrackSetOverrideResult {
        TrackSetOverrideResult {
            track_id: TrackId::try_new("set-track").expect("track id is valid"),
            status: StatusOverrideKind::Blocked,
            derived_status: TrackStatus::Blocked,
            view_sync: TrackViewSyncOutcome::Synchronized(Vec::new()),
        }
    }

    fn service(result: Result<TrackSetOverrideResult, TrackSetOverrideError>) -> RecordingService {
        RecordingService { calls: Mutex::new(Vec::new()), result: Mutex::new(Some(result)) }
    }

    #[test]
    fn test_render_track_set_override_outcome_valid_input_preserves_success_contract() {
        let service = service(Ok(result()));

        let outcome = render_track_set_override_outcome(
            &service,
            PathBuf::from("workspace/track/items"),
            Some("set-track".to_owned()),
            "blocked".to_owned(),
            "waiting".to_owned(),
        );

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("[OK] Override set to 'blocked' (track status: blocked)")
        );
        assert_eq!(outcome.stderr, None);
        assert_eq!(service.calls.lock().expect("service lock is available").len(), 1);
    }

    #[test]
    fn test_render_track_set_override_outcome_invalid_items_dir_preserves_error_prefix() {
        let service = service(Ok(result()));

        let outcome = render_track_set_override_outcome(
            &service,
            PathBuf::from("workspace/items"),
            Some("set-track".to_owned()),
            "blocked".to_owned(),
            "waiting".to_owned(),
        );

        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|error| error.starts_with("[ERROR] --items-dir must point to"))
        );
        assert!(service.calls.lock().expect("service lock is available").is_empty());
    }

    #[test]
    fn test_render_track_set_override_outcome_invalid_track_id_preserves_error_prefix() {
        let service = service(Ok(result()));

        let outcome = render_track_set_override_outcome(
            &service,
            PathBuf::from("workspace/track/items"),
            Some("../escape".to_owned()),
            "blocked".to_owned(),
            "waiting".to_owned(),
        );

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.as_deref().is_some_and(|error| error.starts_with("[ERROR] ")));
        assert!(service.calls.lock().expect("service lock is available").is_empty());
    }

    #[test]
    fn test_render_track_set_override_outcome_invalid_status_returns_failure_without_service_call()
    {
        let service = service(Ok(result()));

        let outcome = render_track_set_override_outcome(
            &service,
            PathBuf::from("workspace/track/items"),
            Some("set-track".to_owned()),
            "planned".to_owned(),
            "waiting".to_owned(),
        );

        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|error| { error.contains("unknown status override kind") })
        );
        assert!(service.calls.lock().expect("service lock is available").is_empty());
    }
}
