//! Guarded worktree stashing.
//!
//! The stash boundary is deliberately a small, synchronous port. The usecase
//! layer owns the finite operation contract; the infrastructure adapter owns
//! Git invocation and the repository-local pairing record.

use std::sync::Arc;

use domain::CommitHash;
use thiserror::Error;

use crate::git_workflow::DiagnosticText;

/// The result of a guarded stash push.
///
/// The created variant carries the immutable stash commit identity. A clean
/// worktree is represented explicitly so a paired pop cannot fall through to
/// an unrelated entry in the stash stack.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GitStashPushOutcome {
    /// A stash entry was created for this push.
    Created(CommitHash),
    /// Git reported that there were no local changes to save.
    NothingToStash,
}

/// Failures that can occur while creating a guarded stash.
#[derive(Debug, Error)]
pub enum GitStashPushError {
    /// The operation changed a branch ref or branch history, which is outside
    /// the stash contract. Stash-internal objects and `refs/stash` are the
    /// only permitted ref updates.
    #[error("guarded stash attempted a forbidden branch-ref update")]
    ForbiddenBranchRefUpdate,
    /// A guarded stash pairing record already exists.
    #[error("a guarded stash push is already pending; resolve it with pop first")]
    PendingGuardedStashExists,
    /// Git or the repository-local pairing record was unavailable.
    #[error("git stash unavailable: {0}")]
    Unavailable(DiagnosticText),
}

/// Failures that can occur while restoring a guarded stash.
#[derive(Debug, Error)]
pub enum GitStashPopError {
    /// The operation changed a branch ref or branch history, which is outside
    /// the stash contract. Stash-internal objects and `refs/stash` are the
    /// only permitted ref updates.
    #[error("guarded stash attempted a forbidden branch-ref update")]
    ForbiddenBranchRefUpdate,
    /// No repository-local pairing record is available for this pop.
    #[error(
        "no pending guarded stash record; inspect `git stash list` for the expected entry or OID, then run the guarded push (`bin/sotp git stash push`) before retrying pop or recover the orphaned stash"
    )]
    NoPendingGuardedStash,
    /// Git or the repository-local pairing record was unavailable.
    #[error("git stash unavailable: {0}")]
    Unavailable(DiagnosticText),
    /// The recorded stash commit is no longer present in the stash list.
    #[error("recorded stash identity is missing: {0}")]
    StashIdentityMissing(CommitHash),
    /// The recorded identity does not match the stash entry being inspected.
    #[error("recorded stash identity mismatch: expected {expected}, found {actual}")]
    StashIdentityMismatch {
        /// Identity persisted by the guarded push.
        expected: CommitHash,
        /// Different identity found at the inspected stash entry.
        actual: CommitHash,
    },
}

/// Secondary port implemented by the infrastructure Git adapter.
pub trait GitStashPort: Send + Sync {
    /// Create a guarded stash and return the typed pairing outcome.
    ///
    /// # Errors
    /// Returns [`GitStashPushError`] when Git, the safety guard, or the pairing
    /// record cannot complete the operation.
    fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError>;

    /// Restore the pending guarded stash.
    ///
    /// The infrastructure adapter reads the repository-local record as the
    /// cross-process source of truth.
    ///
    /// # Errors
    /// Returns [`GitStashPopError`] when the pending record is absent, the
    /// recorded stash cannot be verified, Git fails, or the record cannot be
    /// cleared after a successful restoration.
    fn pop(&self) -> Result<(), GitStashPopError>;
}

/// Application service for guarded stash operations.
pub trait GitStashService: Send + Sync {
    /// Create a guarded stash and persist its pairing outcome.
    ///
    /// # Errors
    /// Returns [`GitStashPushError`] when the operation cannot be completed.
    fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError>;

    /// Restore the pending guarded stash.
    ///
    /// # Errors
    /// Returns [`GitStashPopError`] when the operation cannot be completed.
    fn pop(&self) -> Result<(), GitStashPopError>;
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
    fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError> {
        self.port.push()
    }

    fn pop(&self) -> Result<(), GitStashPopError> {
        self.port.pop()
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::{Arc, Mutex};

    use domain::CommitHash;

    use super::{
        GitStashInteractor, GitStashPopError, GitStashPort, GitStashPushError, GitStashPushOutcome,
        GitStashService,
    };

    #[derive(Default)]
    struct RecordingPort {
        pushes: Mutex<u32>,
        pops: Mutex<u32>,
    }

    impl GitStashPort for RecordingPort {
        fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError> {
            *self.pushes.lock().unwrap() += 1;
            Ok(GitStashPushOutcome::Created(
                CommitHash::try_new("0123456789abcdef0123456789abcdef01234567").unwrap(),
            ))
        }

        fn pop(&self) -> Result<(), GitStashPopError> {
            *self.pops.lock().unwrap() += 1;
            Ok(())
        }
    }

    #[test]
    fn test_git_stash_interactor_push_and_parameterless_pop_delegate_to_port() {
        let port = Arc::new(RecordingPort::default());
        let interactor = GitStashInteractor::new(port.clone());

        interactor.push().unwrap();
        interactor.pop().unwrap();

        assert_eq!(*port.pushes.lock().unwrap(), 1);
        assert_eq!(*port.pops.lock().unwrap(), 1);
    }

    struct ForbiddenUpdatePort;

    impl GitStashPort for ForbiddenUpdatePort {
        fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError> {
            Err(GitStashPushError::ForbiddenBranchRefUpdate)
        }

        fn pop(&self) -> Result<(), GitStashPopError> {
            Err(GitStashPopError::ForbiddenBranchRefUpdate)
        }
    }

    #[test]
    fn test_git_stash_interactor_propagates_forbidden_ref_update() {
        let interactor = GitStashInteractor::new(Arc::new(ForbiddenUpdatePort));

        assert!(matches!(interactor.push(), Err(GitStashPushError::ForbiddenBranchRefUpdate)));
        assert!(matches!(interactor.pop(), Err(GitStashPopError::ForbiddenBranchRefUpdate)));
    }

    #[derive(Default)]
    struct StatefulRecordPort {
        pending: Mutex<Option<GitStashPushOutcome>>,
    }

    impl GitStashPort for StatefulRecordPort {
        fn push(&self) -> Result<GitStashPushOutcome, GitStashPushError> {
            let mut pending = self.pending.lock().unwrap();
            if pending.is_some() {
                return Err(GitStashPushError::PendingGuardedStashExists);
            }
            let outcome = GitStashPushOutcome::NothingToStash;
            *pending = Some(outcome.clone());
            Ok(outcome)
        }

        fn pop(&self) -> Result<(), GitStashPopError> {
            let mut pending = self.pending.lock().unwrap();
            if pending.take().is_none() {
                return Err(GitStashPopError::NoPendingGuardedStash);
            }
            Ok(())
        }
    }

    #[test]
    fn test_git_stash_interactor_rejects_second_push_and_propagates_absent_record_failure() {
        let interactor = GitStashInteractor::new(Arc::new(StatefulRecordPort::default()));

        let error = interactor.pop().expect_err("pop without a pending record must fail closed");
        assert!(matches!(&error, GitStashPopError::NoPendingGuardedStash));
        let guidance = error.to_string();
        assert!(guidance.contains("no pending guarded stash record"));
        assert!(guidance.contains("git stash list"));
        assert!(guidance.contains("expected entry or OID"));
        assert!(guidance.contains("bin/sotp git stash push"));
        assert!(guidance.contains("recover the orphaned stash"));
        interactor.push().unwrap();
        assert!(matches!(interactor.push(), Err(GitStashPushError::PendingGuardedStashExists)));
        interactor.pop().unwrap();
    }
}
