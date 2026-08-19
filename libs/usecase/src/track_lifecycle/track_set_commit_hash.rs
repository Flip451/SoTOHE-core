//! Application boundary for persisting a track's current commit hash.

use std::sync::Arc;

use domain::{CommitHash, TrackId};

use crate::git_workflow::DiagnosticText;

use super::{TrackCommitHashPort, TrackLifecycleIdInput};

/// Typed command for persisting the current commit hash for a track.
pub struct TrackSetCommitHashCommand {
    /// Validated track identity.
    pub track_id: TrackId,
}

impl TrackSetCommitHashCommand {
    /// Creates a commit-hash command from validated primary-adapter input.
    #[must_use]
    pub fn new(track_id: TrackLifecycleIdInput) -> Self {
        Self { track_id: track_id.into_track_id() }
    }
}

/// Error returned by the track commit-hash boundary.
#[derive(Debug)]
pub enum TrackSetCommitHashError {
    /// Git discovery, commit resolution, or persistence failed.
    ExecutionFailed(DiagnosticText),
}

impl std::fmt::Display for TrackSetCommitHashError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ExecutionFailed(error) => formatter.write_str(error.as_str()),
        }
    }
}

impl std::error::Error for TrackSetCommitHashError {}

/// Presentation-free result of persisting a track's current commit hash.
pub struct TrackSetCommitHashResult {
    /// The commit hash written to the track's `.commit_hash` file.
    pub commit_hash: CommitHash,
}

/// Application service for persisting a track's current commit hash.
pub trait TrackSetCommitHashService: Send + Sync {
    /// Resolves and persists the current commit hash for the requested track.
    fn execute(
        &self,
        command: TrackSetCommitHashCommand,
    ) -> Result<TrackSetCommitHashResult, TrackSetCommitHashError>;
}

/// Interactor for the track commit-hash command context.
pub struct TrackSetCommitHashInteractor {
    persistence: Arc<dyn TrackCommitHashPort>,
}

impl TrackSetCommitHashInteractor {
    /// Creates an interactor from the existing commit-hash persistence port.
    #[must_use]
    pub fn new(persistence: Arc<dyn TrackCommitHashPort>) -> Self {
        Self { persistence }
    }
}

impl TrackSetCommitHashService for TrackSetCommitHashInteractor {
    fn execute(
        &self,
        command: TrackSetCommitHashCommand,
    ) -> Result<TrackSetCommitHashResult, TrackSetCommitHashError> {
        let commit_hash = self
            .persistence
            .persist_current_for_track(&command.track_id)
            .map_err(TrackSetCommitHashError::ExecutionFailed)?;
        Ok(TrackSetCommitHashResult { commit_hash })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::sync::Mutex;

    use super::*;

    struct RecordingPersistence {
        commit_hash: Option<CommitHash>,
        error: Option<String>,
        calls: Mutex<Vec<TrackId>>,
    }

    impl TrackCommitHashPort for RecordingPersistence {
        fn persist_current_for_track(
            &self,
            track_id: &TrackId,
        ) -> Result<CommitHash, DiagnosticText> {
            self.calls.lock().expect("persistence lock is available").push(track_id.clone());
            if let Some(error) = &self.error {
                return Err(DiagnosticText::new(error));
            }
            Ok(self.commit_hash.clone().expect("successful persistence has a commit hash"))
        }
    }

    fn command(track_id: &str) -> TrackSetCommitHashCommand {
        TrackSetCommitHashCommand::new(
            TrackLifecycleIdInput::try_new(track_id.to_owned()).expect("track id is valid"),
        )
    }

    #[test]
    fn test_track_set_commit_hash_interactor_persists_current_hash_and_returns_result() {
        let expected = CommitHash::try_new("a".repeat(40)).expect("commit hash is valid");
        let persistence = Arc::new(RecordingPersistence {
            commit_hash: Some(expected.clone()),
            error: None,
            calls: Mutex::new(Vec::new()),
        });
        let interactor = TrackSetCommitHashInteractor::new(persistence.clone());

        let result =
            interactor.execute(command("commit-track")).expect("hash persistence succeeds");

        assert_eq!(result.commit_hash.as_ref(), expected.as_ref());
        let calls = persistence.calls.lock().expect("persistence lock is available");
        assert_eq!(
            calls.as_slice(),
            &[TrackId::try_new("commit-track").expect("track id is valid")]
        );
    }

    #[test]
    fn test_track_set_commit_hash_interactor_persistence_failure_returns_execution_error() {
        let persistence = Arc::new(RecordingPersistence {
            commit_hash: None,
            error: Some("current branch does not match track".to_owned()),
            calls: Mutex::new(Vec::new()),
        });
        let interactor = TrackSetCommitHashInteractor::new(persistence.clone());

        let error = match interactor.execute(command("commit-track")) {
            Ok(_) => panic!("persistence failure must be returned"),
            Err(error) => error,
        };

        assert!(
            matches!(error, TrackSetCommitHashError::ExecutionFailed(message) if message.as_str() == "current branch does not match track")
        );
        assert_eq!(persistence.calls.lock().expect("persistence lock is available").len(), 1);
    }
}
