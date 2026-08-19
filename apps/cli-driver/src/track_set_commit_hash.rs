//! Primary-adapter rendering for the \`track set-commit-hash\` command.

use usecase::track_lifecycle::TrackLifecycleIdInput;
use usecase::track_lifecycle::track_set_commit_hash::{
    TrackSetCommitHashCommand, TrackSetCommitHashResult, TrackSetCommitHashService,
};

use crate::adr_baseline::TrackIdInput;
use crate::render::CommandOutcome;

const RECOVERY_HINT: &str = "[set-commit-hash] Recovery: run \x60bin/sotp track set-commit-hash\x60 to set the v2 diff base manually.";

/// Validates the track input, invokes the commit-hash service, and preserves the
/// legacy stream and exit-code contract.
pub(crate) fn render_track_set_commit_hash_outcome(
    service: &dyn TrackSetCommitHashService,
    input: TrackIdInput,
) -> CommandOutcome {
    let track_id = match TrackLifecycleIdInput::try_new(input.to_string()) {
        Ok(track_id) => track_id,
        Err(error) => return failure_outcome(error.to_string()),
    };
    service
        .execute(TrackSetCommitHashCommand::new(track_id))
        .map(success_outcome)
        .unwrap_or_else(|error| failure_outcome(error.to_string()))
}

fn success_outcome(result: TrackSetCommitHashResult) -> CommandOutcome {
    let message = format!("Recorded .commit_hash: {}", result.commit_hash);
    CommandOutcome {
        stdout: Some(message.clone()),
        stderr: Some(format!("[set-commit-hash] {message}")),
        exit_code: 0,
    }
}

fn failure_outcome(message: String) -> CommandOutcome {
    CommandOutcome::failure(Some(format!("[set-commit-hash] ERROR: {message}\n{RECOVERY_HINT}")))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;
    use domain::CommitHash;
    use usecase::track_lifecycle::track_set_commit_hash::TrackSetCommitHashError;

    struct RecordingService {
        calls: Mutex<Vec<TrackSetCommitHashCommand>>,
        error: Option<String>,
    }

    impl TrackSetCommitHashService for RecordingService {
        fn execute(
            &self,
            command: TrackSetCommitHashCommand,
        ) -> Result<TrackSetCommitHashResult, TrackSetCommitHashError> {
            self.calls.lock().expect("service lock is available").push(command);
            match &self.error {
                Some(error) => Err(TrackSetCommitHashError::ExecutionFailed(
                    usecase::git_workflow::DiagnosticText::new(error),
                )),
                None => Ok(TrackSetCommitHashResult {
                    commit_hash: CommitHash::try_new("a".repeat(40)).expect("hash is valid"),
                }),
            }
        }
    }

    fn track_id() -> TrackIdInput {
        "commit-track".parse().expect("track id is valid")
    }

    #[test]
    fn test_render_track_set_commit_hash_outcome_valid_input_preserves_success_contract() {
        let service = RecordingService { calls: Mutex::new(Vec::new()), error: None };

        let outcome = render_track_set_commit_hash_outcome(&service, track_id());

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("Recorded .commit_hash: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("[set-commit-hash] Recorded .commit_hash"))
        );
        assert_eq!(service.calls.lock().expect("service lock is available").len(), 1);
    }

    #[test]
    fn test_render_track_set_commit_hash_outcome_service_error_preserves_recovery_hint() {
        let service = RecordingService {
            calls: Mutex::new(Vec::new()),
            error: Some("current branch 'main' does not match track branch".to_owned()),
        };

        let outcome = render_track_set_commit_hash_outcome(&service, track_id());

        assert_eq!(outcome.exit_code, 1);
        let stderr = outcome.stderr.expect("failure output is present");
        assert!(stderr.contains("[set-commit-hash] ERROR: current branch 'main'"));
        assert!(stderr.contains(RECOVERY_HINT));
    }
}
