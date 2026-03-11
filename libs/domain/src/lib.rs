//! Domain layer for the SoTOHE-core track state machine.
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]

mod error;
pub mod guard;
mod ids;
pub mod lock;
mod plan;
mod repository;
mod track;

pub use error::{DomainError, RepositoryError, TransitionError, ValidationError};
pub use ids::{CommitHash, TaskId, TrackId};
pub use plan::{PlanSection, PlanView};
pub use repository::TrackRepository;
pub use track::{
    StatusOverride, TaskStatus, TaskStatusKind, TaskTransition, TrackMetadata, TrackStatus,
    TrackTask,
};

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
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

    #[test]
    fn task_id_rejects_non_digit_after_prefix() {
        assert!(matches!(TaskId::new("Ta"), Err(ValidationError::InvalidTaskId(_))));
        assert!(matches!(TaskId::new("T-1"), Err(ValidationError::InvalidTaskId(_))));
        assert!(matches!(TaskId::new("T"), Err(ValidationError::InvalidTaskId(_))));
        assert!(TaskId::new("T1").is_ok());
        assert!(TaskId::new("T123").is_ok());
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
}
