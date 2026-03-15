use std::process::ExitCode;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use domain::{
    DomainError, PlanSection, PlanView, TaskId, TaskTransition, TrackId, TrackMetadata, TrackTask,
};
use infrastructure::InMemoryTrackStore;
use usecase::{SaveTrackUseCase, TransitionTaskUseCase};

mod commands;
mod error;

pub use error::CliError;

/// SoTOHE-core CLI: track state machine and file lock management.
#[derive(Parser)]
#[command(name = "sotp", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand)]
enum CliCommand {
    /// File lock management for agent concurrent access.
    Lock {
        #[command(subcommand)]
        cmd: commands::lock::LockCommand,
        /// Directory for lock registry files.
        #[arg(long, default_value = ".locks")]
        locks_dir: String,
    },
    /// Shell command guard for git operation blocking.
    Guard {
        #[command(subcommand)]
        cmd: commands::guard::GuardCommand,
    },
    /// Security-critical hook dispatch (Rust fail-closed).
    Hook {
        #[command(subcommand)]
        cmd: commands::hook::HookCommand,
    },
    /// Track operations (transition, etc.) with file-system persistence.
    Track {
        #[command(subcommand)]
        cmd: commands::track::TrackCommand,
    },
    /// Guarded local git workflow wrappers.
    Git {
        #[command(subcommand)]
        cmd: commands::git::GitCommand,
    },
    /// Pull-request workflow wrappers.
    Pr {
        #[command(subcommand)]
        cmd: commands::pr::PrCommand,
    },
    /// Local review workflow wrappers.
    Review {
        #[command(subcommand)]
        cmd: commands::review::ReviewCommand,
    },
    /// File utility operations (atomic write, etc.).
    File {
        #[command(subcommand)]
        cmd: commands::file::FileCommand,
    },
    /// Run the example track state machine demo.
    Demo,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    match cli.command {
        Some(CliCommand::Lock { cmd, locks_dir }) => commands::lock::execute(cmd, &locks_dir),
        Some(CliCommand::Guard { cmd }) => commands::guard::execute(cmd),
        Some(CliCommand::Hook { cmd }) => commands::hook::execute(cmd),
        Some(CliCommand::Track { cmd }) => commands::track::execute(cmd),
        Some(CliCommand::Git { cmd }) => commands::git::execute(cmd),
        Some(CliCommand::Pr { cmd }) => commands::pr::execute(cmd),
        Some(CliCommand::Review { cmd }) => commands::review::execute(cmd),
        Some(CliCommand::File { cmd }) => commands::file::execute(cmd),
        Some(CliCommand::Demo) | None => match run_demo() {
            Ok(code) => code,
            Err(err) => {
                eprintln!("{err}");
                err.exit_code()
            }
        },
    }
}

fn run_demo() -> Result<ExitCode, CliError> {
    let store = Arc::new(InMemoryTrackStore::new());
    let save = SaveTrackUseCase::new(Arc::clone(&store));
    let transition = TransitionTaskUseCase::new(Arc::clone(&store));

    let track = example_track()
        .map_err(|e| CliError::Message(format!("failed to build example track: {e}")))?;
    let track_id = track.id().clone();

    save.execute(&track)
        .map_err(|e| CliError::Message(format!("failed to save example track: {e}")))?;

    let task_id = TaskId::new("T1")
        .map_err(|e| CliError::Message(format!("failed to build example task id: {e}")))?;

    let updated = transition
        .execute(&track_id, &task_id, TaskTransition::Start)
        .map_err(|e| CliError::Message(format!("failed to transition example task: {e}")))?;

    println!("SoTOHE-core CLI stub: '{}' is {}", updated.id(), updated.status());
    Ok(ExitCode::SUCCESS)
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
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::Arc;

    use domain::{TaskId, TaskTransition, TrackStatus};
    use infrastructure::InMemoryTrackStore;
    use usecase::{SaveTrackUseCase, TransitionTaskUseCase};

    use super::example_track;

    #[test]
    fn example_cli_flow_moves_track_into_in_progress() {
        let store = Arc::new(InMemoryTrackStore::new());
        let save = SaveTrackUseCase::new(Arc::clone(&store));
        let transition = TransitionTaskUseCase::new(Arc::clone(&store));
        let track = example_track().unwrap();
        let task_id = TaskId::new("T1").unwrap();

        save.execute(&track).unwrap();
        let updated = transition.execute(track.id(), &task_id, TaskTransition::Start).unwrap();

        assert_eq!(updated.status(), TrackStatus::InProgress);
    }
}
