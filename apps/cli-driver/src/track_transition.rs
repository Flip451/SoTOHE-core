//! Primary-adapter rendering for the `track transition` command.

use std::path::{Path, PathBuf};

use usecase::track_lifecycle::track_transition::{
    TrackTransitionCommand, TrackTransitionError, TrackTransitionResult, TrackTransitionService,
};
use usecase::track_lifecycle::{
    RenderedViewPath, TrackItemsDirectory, TrackLifecycleIdInput, TrackSelection,
    TrackTaskTransition, TrackViewSyncOutcome,
};

use crate::render::CommandOutcome;

/// Validates transition input, invokes the injected service, and renders the legacy CLI result.
#[allow(clippy::too_many_arguments)]
pub(crate) fn render_track_transition_outcome(
    service: &dyn TrackTransitionService,
    items_dir: PathBuf,
    track_id: Option<String>,
    task_id: String,
    target_status: String,
    commit_hash: Option<String>,
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
    let transition = match TrackTaskTransition::try_new(target_status, commit_hash) {
        Ok(transition) => transition,
        Err(error) => return transition_failure(format!("transition failed: {error}")),
    };
    let project_root = project_root_for_items(items_dir.as_path());
    let command = match TrackTransitionCommand::try_new(items_dir, track, task_id, transition) {
        Ok(command) => command,
        Err(error) => return track_transition_error_to_outcome(error),
    };
    service
        .execute(command)
        .map(|result| render_transition_result(result, &project_root))
        .unwrap_or_else(track_transition_error_to_outcome)
}

fn render_transition_result(result: TrackTransitionResult, project_root: &Path) -> CommandOutcome {
    match result {
        TrackTransitionResult::Transitioned {
            task_id,
            target_status,
            derived_status,
            view_sync,
            ..
        } => {
            let mut lines = vec![format!(
                "[OK] {}: transitioned to {} (track status: {})",
                task_id, target_status, derived_status
            )];
            lines.extend(render_views(project_root, view_sync));
            CommandOutcome::success(Some(lines.join("\n")))
        }
        TrackTransitionResult::Rejected { task_id, reason } => {
            CommandOutcome::failure(Some(format!("[BLOCKED] {task_id}: {reason}")))
        }
    }
}

fn render_views(project_root: &Path, view_sync: TrackViewSyncOutcome) -> Vec<String> {
    match view_sync {
        TrackViewSyncOutcome::Synchronized(rendered_views) => {
            rendered_view_lines(project_root, rendered_views)
        }
        TrackViewSyncOutcome::Warning { rendered_views, diagnostic } => {
            let mut lines = rendered_view_lines(project_root, rendered_views);
            lines.push(format!("warning: operation persisted but sync-views failed: {diagnostic}"));
            lines
        }
    }
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

fn track_transition_error_to_outcome(error: TrackTransitionError) -> CommandOutcome {
    transition_failure(error)
}

fn transition_failure(error: impl std::fmt::Display) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

fn invalid_track_id(error: impl std::fmt::Display) -> CommandOutcome {
    let error = error.to_string();
    let legacy_error = error.strip_prefix("invalid track id: ").unwrap_or(&error);
    transition_failure(legacy_error)
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
    use domain::{TaskStatusKind, TrackStatus};

    struct RecordingService {
        result: Mutex<Option<Result<TrackTransitionResult, TrackTransitionError>>>,
    }

    impl TrackTransitionService for RecordingService {
        fn execute(
            &self,
            _command: TrackTransitionCommand,
        ) -> Result<TrackTransitionResult, TrackTransitionError> {
            self.result.lock().expect("service lock is available").take().expect("one result")
        }
    }

    fn transitioned() -> TrackTransitionResult {
        TrackTransitionResult::Transitioned {
            track_id: domain::TrackId::try_new("transition-track").expect("track id is valid"),
            task_id: domain::TaskId::try_new("T001").expect("task id is valid"),
            target_status: TaskStatusKind::InProgress,
            derived_status: TrackStatus::InProgress,
            view_sync: TrackViewSyncOutcome::Synchronized(vec![RenderedViewPath::new(
                PathBuf::from("workspace/track/registry.md"),
            )]),
        }
    }

    fn service(result: Result<TrackTransitionResult, TrackTransitionError>) -> RecordingService {
        RecordingService { result: Mutex::new(Some(result)) }
    }

    #[test]
    fn test_track_transition_driver_success_preserves_cli_output() {
        let outcome = render_track_transition_outcome(
            &service(Ok(transitioned())),
            PathBuf::from("workspace/track/items"),
            Some("transition-track".to_owned()),
            "T001".to_owned(),
            "in_progress".to_owned(),
            None,
        );

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "[OK] T001: transitioned to in_progress (track status: in_progress)\n[OK] Rendered: track/registry.md"
            )
        );
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_track_transition_driver_invalid_items_dir_preserves_error_prefix() {
        let outcome = render_track_transition_outcome(
            &service(Ok(transitioned())),
            PathBuf::from("wrong/items"),
            Some("transition-track".to_owned()),
            "T001".to_owned(),
            "in_progress".to_owned(),
            None,
        );

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] --items-dir must point to '<project-root>/track/items'; got wrong/items")
        );
    }

    #[test]
    fn test_track_transition_driver_invalid_track_id_preserves_error_prefix() {
        let outcome = render_track_transition_outcome(
            &service(Ok(transitioned())),
            PathBuf::from("workspace/track/items"),
            Some("../escape".to_owned()),
            "T001".to_owned(),
            "in_progress".to_owned(),
            None,
        );

        assert_eq!(outcome.exit_code, 1);
        assert!(outcome.stderr.as_deref().is_some_and(|error| error.starts_with("[ERROR]")));
    }

    #[test]
    fn test_track_transition_driver_rejection_preserves_blocked_output() {
        let result = TrackTransitionResult::Rejected {
            task_id: domain::TaskId::try_new("T001").expect("task id is valid"),
            reason: usecase::git_workflow::DiagnosticText::new("admission refused"),
        };
        let outcome = render_track_transition_outcome(
            &service(Ok(result)),
            PathBuf::from("workspace/track/items"),
            Some("transition-track".to_owned()),
            "T001".to_owned(),
            "in_progress".to_owned(),
            None,
        );

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("[BLOCKED] T001: admission refused"));
    }
}
