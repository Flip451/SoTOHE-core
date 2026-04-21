#![forbid(unsafe_code)]
//! Domain layer for the SoTOHE-core track state machine.

pub mod auto_phase;
mod decision;
mod error;
pub mod git_ref;
pub mod guard;
pub mod hook;
mod ids;
pub mod impl_plan;
mod plan;
pub mod plan_ref;
mod repository;
pub mod review_v2;
pub mod schema;
mod signal;
pub mod skill_compliance;
pub mod spec;
pub mod task_coverage;
pub mod tddd;
mod timestamp;
mod track;
pub mod track_phase;
pub mod verify;

pub use decision::Decision;
pub use error::{
    DomainError, RepositoryError, TrackReadError, TrackWriteError, TransitionError,
    ValidationError, WorktreeError,
};
pub use git_ref::{RefValidationError, validate_branch_ref};
pub use ids::{CommitHash, NonEmptyString, ReviewGroupName, TaskId, TrackBranch, TrackId};
pub use impl_plan::{IMPL_PLAN_SCHEMA_VERSION, ImplPlanDocument};
pub use plan::{PlanSection, PlanView};
pub use plan_ref::{
    AdrAnchor, AdrRef, ContentHash, ConventionAnchor, ConventionRef, InformalGroundKind,
    InformalGroundRef, InformalGroundSummary, SpecElementId, SpecRef,
};
pub use repository::{ImplPlanReader, ImplPlanWriter, TrackReader, TrackWriter, WorktreeReader};
pub use review_v2::RoundType;
pub use schema::{TraitImplEntry, TraitNode, TypeGraph, TypeNode};
pub use signal::{
    ConfidenceSignal, SignalBasis, SignalCounts, classify_source_tag, evaluate_source_tag,
};
pub use spec::{
    HearingMode, HearingRecord, HearingSignalDelta, HearingSignalSnapshot, SpecDocument,
    SpecRequirement, SpecScope, SpecSection, SpecValidationError, check_spec_doc_signals,
    evaluate_requirement_signal,
};
pub use task_coverage::{TASK_COVERAGE_SCHEMA_VERSION, TaskCoverageDocument};
pub use tddd::baseline::{TraitBaselineEntry, TypeBaseline, TypeBaselineEntry};
pub use tddd::catalogue::{
    MemberDeclaration, MethodDeclaration, ParamDeclaration, TypeAction, TypeCatalogueDocument,
    TypeCatalogueEntry, TypeDefinitionKind, TypeSignal, TypestateTransitions,
};
pub use tddd::consistency::{
    ActionContradiction, ActionContradictionKind, ConsistencyReport, check_consistency,
    check_type_signals,
};
pub use tddd::signals::{evaluate_type_signals, undeclared_to_signals};
pub use tddd::type_signals_doc::{
    TYPE_SIGNALS_SCHEMA_VERSION, TypeSignalsDocument, TypeSignalsLoadResult,
};
pub use timestamp::Timestamp;
pub use track::{
    StatusOverride, StatusOverrideKind, TaskStatus, TaskStatusKind, TaskTransition, TrackMetadata,
    TrackStatus, TrackTask, derive_track_status,
};

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use rstest::rstest;

    use super::*;

    fn task(id: &str, description: &str) -> TrackTask {
        TrackTask::new(TaskId::try_new(id).unwrap(), description).unwrap()
    }

    #[test]
    fn track_id_rejects_non_slug_values() {
        let result = TrackId::try_new("Not A Slug");

        assert!(matches!(
            result,
            Err(ValidationError::InvalidTrackId(value)) if value == "Not A Slug"
        ));
    }

    #[test]
    fn commit_hash_requires_lowercase_hex_between_seven_and_forty_chars() {
        let result = CommitHash::try_new("abc123");

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

    // TrackMetadata is identity-only; status is derived on demand via
    // derive_track_status(impl_plan, status_override). Task-level state machine tests
    // stay in track.rs; the lib integration tests verify the public API surface.

    #[test]
    fn track_status_derives_to_planned_with_no_plan_no_override() {
        let track = TrackMetadata::new(
            TrackId::try_new("track-state-machine").unwrap(),
            "Track state machine",
            None,
        )
        .unwrap();
        // No impl-plan, no override → Planned
        assert_eq!(derive_track_status(None, track.status_override()), TrackStatus::Planned);
    }

    #[test]
    fn track_status_override_sets_blocked() {
        let mut track = TrackMetadata::new(
            TrackId::try_new("track-state-machine").unwrap(),
            "Track state machine",
            None,
        )
        .unwrap();

        track.set_status_override(Some(StatusOverride::blocked("waiting on review").unwrap()));
        assert_eq!(derive_track_status(None, track.status_override()), TrackStatus::Blocked);
        assert!(track.status_override().is_some());

        // Clearing override → Planned
        track.set_status_override(None);
        assert_eq!(track.status_override(), None);
        assert_eq!(derive_track_status(None, track.status_override()), TrackStatus::Planned);
    }

    #[rstest]
    #[case::letter_after_prefix_rejected("Ta", false)]
    #[case::hyphen_digit_after_prefix_rejected("T-1", false)]
    #[case::prefix_only_rejected("T", false)]
    #[case::single_digit_accepted("T1", true)]
    #[case::multi_digit_accepted("T123", true)]
    fn task_id_rejects_non_digit_after_prefix(#[case] input: &str, #[case] should_pass: bool) {
        let result = TaskId::try_new(input);
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
        let branch = TrackBranch::try_new("track/my-feature").unwrap();
        assert_eq!(branch.as_ref(), "track/my-feature");
        assert_eq!(branch.to_string(), "track/my-feature");
    }

    #[rstest]
    #[case::missing_prefix("main")]
    #[case::invalid_slug_after_prefix("track/Not Valid")]
    #[case::empty_slug("track/")]
    fn track_branch_rejects_invalid_input(#[case] input: &str) {
        assert!(
            matches!(TrackBranch::try_new(input), Err(ValidationError::InvalidTrackBranch(_))),
            "expected {input:?} to be rejected"
        );
    }

    #[test]
    fn track_metadata_with_branch_stores_branch() {
        let track = TrackMetadata::with_branch(
            TrackId::try_new("my-track").unwrap(),
            Some(TrackBranch::try_new("track/my-track").unwrap()),
            "My Track",
            None,
        )
        .unwrap();
        assert_eq!(track.branch().unwrap().as_ref(), "track/my-track");
    }

    #[test]
    fn track_metadata_without_branch_returns_none() {
        let track =
            TrackMetadata::new(TrackId::try_new("my-track").unwrap(), "My Track", None).unwrap();
        assert!(track.branch().is_none());
    }
}
