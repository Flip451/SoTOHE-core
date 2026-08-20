//! Primary-adapter rendering for [`crate::track::TrackDriver`]'s switch-base path.

use std::path::PathBuf;

use usecase::track_lifecycle::TrackWorkspaceRoot;
use usecase::track_lifecycle::track_switch_base::{
    TrackSwitchBaseCommand, TrackSwitchBaseError, TrackSwitchBaseResult, TrackSwitchBaseService,
};

use crate::render::CommandOutcome;

/// Validates the workspace root, invokes the switch-base service, and preserves
/// the legacy stream and exit-code contract.
pub(crate) fn render_track_switch_base_outcome(
    service: &dyn TrackSwitchBaseService,
    project_root: PathBuf,
) -> CommandOutcome {
    let workspace_root = match TrackWorkspaceRoot::try_new(project_root) {
        Ok(workspace_root) => workspace_root,
        Err(error) => return CommandOutcome::failure(Some(format!("[ERROR] {error}"))),
    };
    service
        .execute(TrackSwitchBaseCommand::new(workspace_root))
        .map(success_outcome)
        .unwrap_or_else(error_outcome)
}

fn success_outcome(result: TrackSwitchBaseResult) -> CommandOutcome {
    match result {
        TrackSwitchBaseResult::Synced { branch } => {
            let branch = branch.as_str();
            CommandOutcome::success(Some(format!(
                "Switching to {branch}...\nPulling latest from origin/{branch}...\n[OK] On {branch}, up to date."
            )))
        }
        TrackSwitchBaseResult::SyncWarning { branch } => {
            let branch = branch.as_str();
            CommandOutcome::success(Some(format!(
                "Switching to {branch}...\nPulling latest from origin/{branch}...\n[WARN] Pull failed (may not have remote tracking branch)"
            )))
        }
        TrackSwitchBaseResult::CheckoutFailed { branch, exit_code } => CommandOutcome {
            stdout: Some(format!("Failed to checkout {}", branch.as_str())),
            stderr: None,
            exit_code: exit_code.value(),
        },
    }
}

fn error_outcome(error: TrackSwitchBaseError) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[ERROR] {error}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use domain::BaseBranchName;
    use usecase::track_lifecycle::ProcessExitCode;

    use super::*;

    struct RecordingService {
        result: Result<TrackSwitchBaseResult, String>,
        calls: Mutex<Vec<PathBuf>>,
    }

    impl TrackSwitchBaseService for RecordingService {
        fn execute(
            &self,
            command: TrackSwitchBaseCommand,
        ) -> Result<TrackSwitchBaseResult, TrackSwitchBaseError> {
            self.calls
                .lock()
                .expect("service lock is available")
                .push(command.workspace_root.as_path().to_path_buf());
            match &self.result {
                Ok(TrackSwitchBaseResult::Synced { branch }) => {
                    Ok(TrackSwitchBaseResult::Synced { branch: branch.clone() })
                }
                Ok(TrackSwitchBaseResult::SyncWarning { branch }) => {
                    Ok(TrackSwitchBaseResult::SyncWarning { branch: branch.clone() })
                }
                Ok(TrackSwitchBaseResult::CheckoutFailed { branch, exit_code }) => {
                    Ok(TrackSwitchBaseResult::CheckoutFailed {
                        branch: branch.clone(),
                        exit_code: *exit_code,
                    })
                }
                Err(error) => Err(TrackSwitchBaseError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
            }
        }
    }

    fn base_branch() -> BaseBranchName {
        BaseBranchName::try_new("main".to_owned()).expect("base branch is valid")
    }

    #[test]
    fn test_render_track_switch_base_outcome_synced_preserves_success_contract() {
        let service = RecordingService {
            result: Ok(TrackSwitchBaseResult::Synced { branch: base_branch() }),
            calls: Mutex::new(Vec::new()),
        };

        let outcome = render_track_switch_base_outcome(&service, PathBuf::from("workspace"));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "Switching to main...\nPulling latest from origin/main...\n[OK] On main, up to date."
            )
        );
        assert_eq!(outcome.stderr, None);
        assert_eq!(
            service.calls.lock().expect("service lock is available").as_slice(),
            &[PathBuf::from("workspace")]
        );
    }

    #[test]
    fn test_render_track_switch_base_outcome_sync_warning_preserves_success_contract() {
        let service = RecordingService {
            result: Ok(TrackSwitchBaseResult::SyncWarning { branch: base_branch() }),
            calls: Mutex::new(Vec::new()),
        };

        let outcome = render_track_switch_base_outcome(&service, PathBuf::from("workspace"));

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "Switching to main...\nPulling latest from origin/main...\n[WARN] Pull failed (may not have remote tracking branch)"
            )
        );
        assert_eq!(outcome.stderr, None);
    }

    #[test]
    fn test_render_track_switch_base_outcome_checkout_failure_preserves_legacy_result() {
        let service = RecordingService {
            result: Ok(TrackSwitchBaseResult::CheckoutFailed {
                branch: base_branch(),
                exit_code: ProcessExitCode::new(7),
            }),
            calls: Mutex::new(Vec::new()),
        };

        let outcome = render_track_switch_base_outcome(&service, PathBuf::from("workspace"));

        assert_eq!(outcome.stdout.as_deref(), Some("Failed to checkout main"));
        assert_eq!(outcome.stderr, None);
        assert_eq!(outcome.exit_code, 7);
    }

    #[test]
    fn test_render_track_switch_base_outcome_service_error_maps_to_prefixed_failure() {
        let service = RecordingService {
            result: Err("not a track branch".to_owned()),
            calls: Mutex::new(Vec::new()),
        };

        let outcome = render_track_switch_base_outcome(&service, PathBuf::from("workspace"));

        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stderr.as_deref(), Some("[ERROR] not a track branch"));
        assert_eq!(outcome.stdout, None);
    }
}
