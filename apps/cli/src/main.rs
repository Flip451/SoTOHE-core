use std::process::ExitCode;
use std::sync::Arc;

use domain::{
    DomainError, PlanSection, PlanView, TaskId, TaskTransition, TrackId, TrackMetadata, TrackTask,
};
use infrastructure::InMemoryTrackRepository;
use usecase::{SaveTrackUseCase, TransitionTaskUseCase};

fn main() -> ExitCode {
    let repo = Arc::new(InMemoryTrackRepository::new());
    let save = SaveTrackUseCase::new(Arc::clone(&repo));
    let transition = TransitionTaskUseCase::new(Arc::clone(&repo));

    let track = match example_track() {
        Ok(track) => track,
        Err(err) => {
            eprintln!("failed to build example track: {err}");
            return ExitCode::FAILURE;
        }
    };
    let track_id = track.id().clone();

    if let Err(err) = save.execute(&track) {
        eprintln!("failed to save example track: {err}");
        return ExitCode::FAILURE;
    }

    let task_id = match TaskId::new("T1") {
        Ok(task_id) => task_id,
        Err(err) => {
            eprintln!("failed to build example task id: {err}");
            return ExitCode::FAILURE;
        }
    };

    let updated = match transition.execute(&track_id, &task_id, TaskTransition::Start) {
        Ok(track) => track,
        Err(err) => {
            eprintln!("failed to transition example task: {err}");
            return ExitCode::FAILURE;
        }
    };

    println!("SoTOHE-core CLI stub: '{}' is {}", updated.id(), updated.status());
    ExitCode::SUCCESS
}

fn example_track() -> Result<TrackMetadata, DomainError> {
    let task_id = TaskId::new("T1")?;
    let task = TrackTask::new(task_id.clone(), "Implement the track aggregate")?;
    let section = PlanSection::new("S1", "Domain model", Vec::new(), vec![task_id])?;
    let plan =
        PlanView::new(vec!["Track status is derived from task state.".to_owned()], vec![section]);

    TrackMetadata::new(
        TrackId::new("track-state-machine")?,
        "Track state machine",
        vec![task],
        plan,
        None,
    )
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use domain::{TaskId, TaskTransition, TrackStatus};
    use infrastructure::InMemoryTrackRepository;
    use usecase::{SaveTrackUseCase, TransitionTaskUseCase};

    use super::example_track;

    #[test]
    fn example_cli_flow_moves_track_into_in_progress() {
        let repo = Arc::new(InMemoryTrackRepository::new());
        let save = SaveTrackUseCase::new(Arc::clone(&repo));
        let transition = TransitionTaskUseCase::new(Arc::clone(&repo));
        let track = example_track().unwrap();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        let updated = transition.execute(track.id(), &task_id, TaskTransition::Start).unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);
    }
}
