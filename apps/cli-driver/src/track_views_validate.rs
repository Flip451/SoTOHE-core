//! Views-validate rendering for [`crate::track::TrackDriver`].

use std::path::PathBuf;

use usecase::track_lifecycle::TrackWorkspaceRoot;
use usecase::track_lifecycle::track_views_validate::{
    TrackViewsValidateCommand, TrackViewsValidateError, TrackViewsValidateResult,
    TrackViewsValidateService,
};

use crate::render::CommandOutcome;

pub(crate) fn render_track_views_validate_outcome(
    service: &dyn TrackViewsValidateService,
    project_root: PathBuf,
) -> CommandOutcome {
    let workspace_root = match TrackWorkspaceRoot::try_new(project_root) {
        Ok(workspace_root) => workspace_root,
        Err(error) => return CommandOutcome::failure(Some(format!("[ERROR] {error}"))),
    };
    service
        .execute(TrackViewsValidateCommand { workspace_root })
        .map(render_track_views_validate_result)
        .unwrap_or_else(track_views_validate_error_to_outcome)
}

fn render_track_views_validate_result(_result: TrackViewsValidateResult) -> CommandOutcome {
    CommandOutcome::success(Some("[OK] Track metadata is valid".to_owned()))
}

fn track_views_validate_error_to_outcome(error: TrackViewsValidateError) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] track metadata validation failed: {error}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use usecase::git_workflow::DiagnosticText;

    use super::*;

    struct RecordingService {
        result: Result<TrackViewsValidateResult, String>,
        calls: Mutex<Vec<PathBuf>>,
    }

    impl TrackViewsValidateService for RecordingService {
        fn execute(
            &self,
            command: TrackViewsValidateCommand,
        ) -> Result<TrackViewsValidateResult, TrackViewsValidateError> {
            self.calls
                .lock()
                .expect("service lock is available")
                .push(command.workspace_root.as_path().to_path_buf());
            match &self.result {
                Ok(TrackViewsValidateResult) => Ok(TrackViewsValidateResult),
                Err(error) => {
                    Err(TrackViewsValidateError::ExecutionFailed(DiagnosticText::new(error)))
                }
            }
        }
    }

    #[test]
    fn test_render_track_views_validate_outcome_success_preserves_cli_contract() {
        let service = RecordingService {
            result: Ok(TrackViewsValidateResult),
            calls: Mutex::new(Vec::new()),
        };
        let project_root = PathBuf::from("workspace");
        let outcome = render_track_views_validate_outcome(&service, project_root.clone());
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("[OK] Track metadata is valid"));
        assert_eq!(outcome.stderr, None);
        let calls = service.calls.lock().expect("service lock is available");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls.first(), Some(&project_root));
    }

    #[test]
    fn test_render_track_views_validate_outcome_service_error_maps_to_prefixed_failure() {
        let service = RecordingService {
            result: Err("invalid metadata".to_owned()),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_views_validate_outcome(&service, PathBuf::from("workspace"));
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] track metadata validation failed: invalid metadata")
        );
        assert_eq!(outcome.stdout, None);
    }

    #[test]
    fn test_render_track_views_validate_outcome_invalid_workspace_preserves_error_prefix() {
        let service = RecordingService {
            result: Ok(TrackViewsValidateResult),
            calls: Mutex::new(Vec::new()),
        };
        let outcome = render_track_views_validate_outcome(&service, PathBuf::from("../escape"));
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(
            outcome.stderr.as_deref(),
            Some("[ERROR] track workspace root must not contain parent traversal: ../escape")
        );
        assert_eq!(outcome.stdout, None);
        assert!(service.calls.lock().expect("service lock is available").is_empty());
    }
}
