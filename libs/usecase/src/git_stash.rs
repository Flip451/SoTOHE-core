//! Guarded worktree stashing.
//!
//! The stash boundary is deliberately a small, synchronous port.  The usecase
//! layer decides which finite operations are legal; the infrastructure adapter
//! is responsible for invoking Git with the repository guard enabled.

use std::sync::Arc;

use thiserror::Error;

use crate::git_workflow::DiagnosticText;

/// Finite application command for saving or restoring the current worktree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GitStashCommand {
    /// Save tracked and untracked worktree changes.
    Push,
    /// Restore the most recent saved worktree.
    Pop,
}

/// Failures from the guarded stash boundary.
#[derive(Debug, Error)]
pub enum GitStashError {
    /// The operation changed a branch ref or branch history, which is outside
    /// the stash contract.  Stash-internal objects and `refs/stash` are the
    /// only permitted ref updates.
    #[error("guarded stash attempted a forbidden branch-ref update")]
    ForbiddenBranchRefUpdate,
    /// Git could not execute the requested stash operation.
    #[error("git stash unavailable: {0}")]
    Unavailable(DiagnosticText),
}

/// Secondary port implemented by the infrastructure Git adapter.
pub trait GitStashPort: Send + Sync {
    /// Execute one guarded stash command.
    fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError>;
}

/// Application service for guarded stash operations.
pub trait GitStashService: Send + Sync {
    /// Execute one guarded stash command.
    fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError>;
}

/// Dependency-injected interactor implementing [`GitStashService`].
pub struct GitStashInteractor {
    port: Arc<dyn GitStashPort>,
}

impl GitStashInteractor {
    /// Construct the interactor over a stash port.
    #[must_use]
    pub fn new(port: Arc<dyn GitStashPort>) -> Self {
        Self { port }
    }
}

impl GitStashService for GitStashInteractor {
    fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError> {
        self.port.execute(command)
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{
        GitStashCommand, GitStashError, GitStashInteractor, GitStashPort, GitStashService,
    };

    #[derive(Default)]
    struct RecordingPort {
        commands: Mutex<Vec<GitStashCommand>>,
    }

    impl GitStashPort for RecordingPort {
        fn execute(&self, command: GitStashCommand) -> Result<(), GitStashError> {
            self.commands.lock().unwrap().push(command);
            Ok(())
        }
    }

    #[test]
    fn test_git_stash_interactor_push_and_pop_delegate_to_port() {
        let port = Arc::new(RecordingPort::default());
        let interactor = GitStashInteractor::new(port.clone());

        interactor.execute(GitStashCommand::Push).unwrap();
        interactor.execute(GitStashCommand::Pop).unwrap();

        assert_eq!(
            *port.commands.lock().unwrap(),
            vec![GitStashCommand::Push, GitStashCommand::Pop]
        );
    }

    struct ForbiddenUpdatePort;

    impl GitStashPort for ForbiddenUpdatePort {
        fn execute(&self, _command: GitStashCommand) -> Result<(), GitStashError> {
            Err(GitStashError::ForbiddenBranchRefUpdate)
        }
    }

    #[test]
    fn test_git_stash_interactor_propagates_forbidden_ref_update() {
        let interactor = GitStashInteractor::new(Arc::new(ForbiddenUpdatePort));

        assert!(matches!(
            interactor.execute(GitStashCommand::Push),
            Err(GitStashError::ForbiddenBranchRefUpdate)
        ));
    }
}
