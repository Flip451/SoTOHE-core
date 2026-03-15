//! Domain layer for the SoTOHE-core track state machine.

mod decision;
mod error;
pub mod guard;
pub mod hook;
mod ids;
pub mod lock;
mod plan;
mod repository;
mod track;
pub mod track_phase;

pub use decision::Decision;
pub use error::{
    DomainError, RepositoryError, TrackReadError, TrackWriteError, TransitionError, ValidationError,
};
pub use ids::{CommitHash, TaskId, TrackBranch, TrackId};
pub use plan::{PlanSection, PlanView};
pub use repository::{TrackReader, TrackWriter, WorktreeReader};
pub use track::{
    StatusOverride, TaskStatus, TaskStatusKind, TaskTransition, TrackMetadata, TrackStatus,
    TrackTask,
};

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn task(id: &str, description: &str) -> TrackTask {
        TrackTask::new(TaskId::new(id).unwrap(), description).unwrap()
    }

    fn section(id: &str, title: &str, task_ids: &[&str]) -> PlanSection {
        PlanSection::new(
            id,
            title,
            Vec::new(),
            task_ids.iter().map(|task_id| TaskId::new(*task_id).unwrap()).collect(),
        )
        .unwrap()
    }

    fn plan(task_ids: &[&str]) -> PlanView {
        PlanView::new(Vec::new(), vec![section("S1", "Build", task_ids)])
    }

    #[test]
    fn track_id_rejects_non_slug_values() {
        let result = TrackId::new("Not A Slug");

        assert!(matches!(
            result,
            Err(ValidationError::InvalidTrackId(value)) if value == "Not A Slug"
        ));
    }

    #[test]
    fn commit_hash_requires_lowercase_hex_between_seven_and_forty_chars() {
        let result = CommitHash::new("abc123");

        assert!(matches!(
            result,
            Err(ValidationError::InvalidCommitHash(value)) if value == "abc123"
        ));
    }

    #[test]
    fn task_transition_accepts_only_reference_state_machine_edges() {
        let mut task = task("T1", "Implement transition logic");

        task.transition(TaskTransition::Start).unwrap();
        task.transition(TaskTransition::Complete { commit_hash: None }).unwrap();

        assert!(matches!(
            task.transition(TaskTransition::Skip),
            Err(TransitionError::InvalidTaskTransition { .. })
        ));
        assert_eq!(task.status().kind(), TaskStatusKind::Done);
    }

    #[test]
    fn track_status_is_derived_from_task_states() {
        let mut track = TrackMetadata::new(
            TrackId::new("track-state-machine").unwrap(),
            "Track state machine",
            vec![task("T1", "Write domain model"), task("T2", "Write tests")],
            plan(&["T1", "T2"]),
            None,
        )
        .unwrap();

        assert_eq!(track.status(), TrackStatus::Planned);

        track.transition_task(&TaskId::new("T1").unwrap(), TaskTransition::Start).unwrap();
        assert_eq!(track.status(), TrackStatus::InProgress);

        track
            .transition_task(
                &TaskId::new("T1").unwrap(),
                TaskTransition::Complete { commit_hash: None },
            )
            .unwrap();
        track.transition_task(&TaskId::new("T2").unwrap(), TaskTransition::Start).unwrap();
        track
            .transition_task(
                &TaskId::new("T2").unwrap(),
                TaskTransition::Complete { commit_hash: None },
            )
            .unwrap();

        assert_eq!(track.status(), TrackStatus::Done);
    }

    #[test]
    fn resolving_every_task_auto_clears_override() {
        let mut track = TrackMetadata::new(
            TrackId::new("track-state-machine").unwrap(),
            "Track state machine",
            vec![task("T1", "Write domain model")],
            plan(&["T1"]),
            Some(StatusOverride::blocked("waiting on review")),
        )
        .unwrap();

        assert_eq!(track.status(), TrackStatus::Blocked);

        track.transition_task(&TaskId::new("T1").unwrap(), TaskTransition::Start).unwrap();
        track
            .transition_task(
                &TaskId::new("T1").unwrap(),
                TaskTransition::Complete { commit_hash: None },
            )
            .unwrap();

        assert_eq!(track.status_override(), None);
        assert_eq!(track.status(), TrackStatus::Done);
    }

    #[rstest]
    #[case::letter_after_prefix_rejected("Ta", false)]
    #[case::hyphen_digit_after_prefix_rejected("T-1", false)]
    #[case::prefix_only_rejected("T", false)]
    #[case::single_digit_accepted("T1", true)]
    #[case::multi_digit_accepted("T123", true)]
    fn task_id_rejects_non_digit_after_prefix(#[case] input: &str, #[case] should_pass: bool) {
        let result = TaskId::new(input);
        if should_pass {
            assert!(result.is_ok(), "expected {input:?} to be valid");
        } else {
            assert!(
                matches!(result, Err(ValidationError::InvalidTaskId(_))),
                "expected {input:?} to be rejected"
            );
        }
    }

    #[test]
    fn plan_must_reference_each_task_exactly_once() {
        let track = TrackMetadata::new(
            TrackId::new("track-state-machine").unwrap(),
            "Track state machine",
            vec![task("T1", "Write domain model"), task("T2", "Write tests")],
            plan(&["T1"]),
            None,
        );

        assert!(matches!(
            track,
            Err(DomainError::Validation(ValidationError::UnreferencedTask(task_id)))
                if task_id == "T2"
        ));
    }

    #[test]
    fn track_read_error_converts_from_repository_error() {
        let repo_err = RepositoryError::TrackNotFound("test-track".to_string());
        let read_err: TrackReadError = repo_err.into();
        assert!(
            matches!(read_err, TrackReadError::Repository(RepositoryError::TrackNotFound(id)) if id == "test-track")
        );
    }

    #[test]
    fn track_write_error_converts_from_repository_error() {
        let repo_err = RepositoryError::Message("disk full".to_string());
        let write_err: TrackWriteError = repo_err.into();
        assert!(
            matches!(write_err, TrackWriteError::Repository(RepositoryError::Message(msg)) if msg == "disk full")
        );
    }

    #[test]
    fn track_write_error_converts_from_domain_error() {
        let domain_err = DomainError::Validation(ValidationError::EmptyTrackTitle);
        let write_err: TrackWriteError = domain_err.into();
        assert!(matches!(
            write_err,
            TrackWriteError::Domain(DomainError::Validation(ValidationError::EmptyTrackTitle))
        ));
    }

    #[test]
    fn track_status_archived_displays_correctly() {
        assert_eq!(TrackStatus::Archived.to_string(), "archived");
    }

    #[test]
    fn track_branch_accepts_valid_format() {
        let branch = TrackBranch::new("track/my-feature").unwrap();
        assert_eq!(branch.as_str(), "track/my-feature");
        assert_eq!(branch.to_string(), "track/my-feature");
    }

    #[rstest]
    #[case::missing_prefix("main")]
    #[case::invalid_slug_after_prefix("track/Not Valid")]
    #[case::empty_slug("track/")]
    fn track_branch_rejects_invalid_input(#[case] input: &str) {
        assert!(
            matches!(TrackBranch::new(input), Err(ValidationError::InvalidTrackBranch(_))),
            "expected {input:?} to be rejected"
        );
    }

    #[test]
    fn track_metadata_with_branch_stores_branch() {
        let track = TrackMetadata::with_branch(
            TrackId::new("my-track").unwrap(),
            Some(TrackBranch::new("track/my-track").unwrap()),
            "My Track",
            vec![task("T1", "Task one")],
            plan(&["T1"]),
            None,
        )
        .unwrap();
        assert_eq!(track.branch().unwrap().as_str(), "track/my-track");
    }

    #[test]
    fn track_metadata_without_branch_returns_none() {
        let track = TrackMetadata::new(
            TrackId::new("my-track").unwrap(),
            "My Track",
            vec![task("T1", "Task one")],
            plan(&["T1"]),
            None,
        )
        .unwrap();
        assert!(track.branch().is_none());
    }
}
