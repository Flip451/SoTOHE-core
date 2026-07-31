//! Record-time verification of a commit hash before a task transition writes
//! it (spec `IN-22`, `AC-28`, `AC-29`).
//!
//! Part of the parent module's use case; a separate file only so that neither
//! module outgrows the workspace module-size limit.

use std::path::Path;

use domain::{CommitHash, ImplPlanReader, TaskId, TrackId};

use super::ports::{CommitRecordVerifierPort, CommitRecordVerifyError};
use super::{TaskOperationError, TaskTransitionCommand};

/// Verifies `commit_hash` before it is recorded, reporting a refusal on the
/// operation's own error channel (`IN-22`, `AC-28`).
///
/// The two rejections become [`TaskOperationError::TransitionFailed`]: the
/// request named a hash this repository will not accept, which is a bad
/// transition rather than a broken store. A repository that could not be read
/// becomes [`TaskOperationError::StoreFailed`], because nothing was judged and
/// the caller must not treat the absence of a refusal as an acceptance.
///
/// The verifier arrives as a parameter rather than through a field so the
/// mapping is testable on its own and the caller stays free to hold the port
/// however it likes.
///
/// # Errors
///
/// Returns [`TaskOperationError::TransitionFailed`] when the hash names no
/// commit object or names one that is not an ancestor of `HEAD`, and
/// [`TaskOperationError::StoreFailed`] when the repository could not be read.
pub(super) fn verify(
    verifier: &dyn CommitRecordVerifierPort,
    items_dir: &Path,
    commit_hash: &CommitHash,
) -> Result<(), TaskOperationError> {
    match verifier.verify_commit_record(items_dir, commit_hash) {
        Ok(()) => Ok(()),
        Err(error @ CommitRecordVerifyError::CommitNotFound { .. })
        | Err(error @ CommitRecordVerifyError::NotAncestorOfHead { .. }) => {
            Err(TaskOperationError::TransitionFailed(error.to_string()))
        }
        Err(error @ CommitRecordVerifyError::RepositoryUnreadable { .. }) => {
            Err(TaskOperationError::StoreFailed(error.to_string()))
        }
    }
}

/// Verifies the hash a requested transition would record, before anything is
/// applied (`IN-22`, `AC-28`, `AC-29`).
///
/// Which transition the request names is the plan's to resolve, exactly as it
/// is for the applier that follows, and which hash that transition would record
/// is the transition's own ([`TaskTransition::recorded_commit_hash`]): the guard
/// verifies exactly the value that would be written, so a hash passed to a
/// transition that ignores it is not made a reason to refuse. An unresolvable
/// transition is left to the applier to report, and a track with no plan has
/// nothing to record into; neither is turned into a verification failure here.
///
/// # Errors
///
/// Returns [`TaskOperationError::StoreFailed`] when the plan cannot be read or
/// the repository cannot be, and [`TaskOperationError::TransitionFailed`] when
/// the repository refuses the hash.
pub(super) fn verify_recorded_hash(
    verifier: &dyn CommitRecordVerifierPort,
    plans: &dyn ImplPlanReader,
    cmd: &TaskTransitionCommand,
    track_id: &TrackId,
    task_id: &TaskId,
    commit_hash: Option<&CommitHash>,
) -> Result<(), TaskOperationError> {
    let Some(commit_hash) = commit_hash else { return Ok(()) };
    let plan = plans
        .load_impl_plan(track_id)
        .map_err(|error| TaskOperationError::StoreFailed(error.to_string()))?;
    let Some(plan) = plan else { return Ok(()) };
    let Ok(transition) =
        plan.transition_for(task_id, &cmd.target_status, Some(commit_hash.clone()))
    else {
        return Ok(());
    };
    let Some(recorded) = transition.recorded_commit_hash() else { return Ok(()) };
    verify(verifier, &cmd.items_dir, recorded)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use domain::FreeText;

    use super::*;

    /// Verifier stub yielding one fixed verdict, so each test observes exactly
    /// the lane its variant is mapped to.
    struct StubVerifier(Box<dyn Fn() -> Result<(), CommitRecordVerifyError> + Send + Sync>);

    impl StubVerifier {
        fn accepting() -> Self {
            Self(Box::new(|| Ok(())))
        }

        fn refusing(error: impl Fn() -> CommitRecordVerifyError + Send + Sync + 'static) -> Self {
            Self(Box::new(move || Err(error())))
        }
    }

    impl CommitRecordVerifierPort for StubVerifier {
        fn verify_commit_record(
            &self,
            _items_dir: &Path,
            _commit_hash: &CommitHash,
        ) -> Result<(), CommitRecordVerifyError> {
            (self.0)()
        }
    }

    fn hash() -> CommitHash {
        CommitHash::try_new("abc1234").unwrap()
    }

    fn items_dir() -> &'static Path {
        Path::new("track/items")
    }

    #[test]
    fn test_accepted_commit_record_passes_verification() {
        let result = verify(&StubVerifier::accepting(), items_dir(), &hash());

        assert!(result.is_ok(), "an accepted hash must not produce an error: {result:?}");
    }

    #[test]
    fn test_absent_commit_is_reported_as_a_transition_failure() {
        let verifier = StubVerifier::refusing(|| CommitRecordVerifyError::CommitNotFound {
            commit_hash: hash(),
        });

        let error = verify(&verifier, items_dir(), &hash()).unwrap_err();

        match error {
            TaskOperationError::TransitionFailed(message) => {
                assert!(
                    message.contains("abc1234") && message.contains("does not exist"),
                    "the refusal must name the rejected hash and its reason: {message}"
                );
            }
            other => panic!("an absent commit must fail the transition lane, got {other:?}"),
        }
    }

    #[test]
    fn test_non_ancestor_commit_is_reported_as_a_transition_failure() {
        let verifier = StubVerifier::refusing(|| CommitRecordVerifyError::NotAncestorOfHead {
            commit_hash: hash(),
        });

        let error = verify(&verifier, items_dir(), &hash()).unwrap_err();

        match error {
            TaskOperationError::TransitionFailed(message) => {
                assert!(
                    message.contains("abc1234") && message.contains("not an ancestor"),
                    "the refusal must name the rejected hash and its reason: {message}"
                );
            }
            other => panic!("a non-ancestor commit must fail the transition lane, got {other:?}"),
        }
    }

    #[test]
    fn test_unreadable_repository_is_reported_as_a_store_failure() {
        let verifier = StubVerifier::refusing(|| CommitRecordVerifyError::RepositoryUnreadable {
            message: FreeText::new("not a git repository"),
        });

        let error = verify(&verifier, items_dir(), &hash()).unwrap_err();

        match error {
            TaskOperationError::StoreFailed(message) => {
                assert!(
                    message.contains("not a git repository"),
                    "an unperformed check must carry the adapter diagnostic: {message}"
                );
            }
            other => panic!("an unreadable repository must fail the store lane, got {other:?}"),
        }
    }
}
