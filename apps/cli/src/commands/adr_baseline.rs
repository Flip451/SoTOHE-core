//! `sotp adr-baseline` command boundary.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::AdrBaselineCompositionRoot;
use cli_driver::adr_baseline::{
    AdrBaselineKindInput, AdrBaselineReasonInput, AdrBaselineRequest, AdrSourceFileNameInput,
    TrackIdInput,
};

use crate::CliError;

/// ADR baseline CLI subcommand family.
#[derive(Debug, Clone, Subcommand)]
pub enum AdrBaselineCommand {
    /// Atomically record an ADR baseline snapshot and ledger entry.
    Snapshot(AdrBaselineSnapshotArgs),
    /// Restore an ADR source from its latest recorded baseline.
    Restore(AdrBaselineRestoreArgs),
    /// Verify the primary ADR's init snapshot before review starts.
    CheckReview(AdrBaselineCheckReviewArgs),
    /// Verify recorded and required ADR baselines before commit.
    CheckCommit(AdrBaselineCheckCommitArgs),
}

/// ADR baseline snapshot CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineSnapshotArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
    /// Direct Markdown filename under `knowledge/adr`.
    #[arg(long)]
    pub source: AdrSourceFileNameInput,
    /// Snapshot kind: init, cite, new-adr, non-semantic-fix, or escalation.
    #[arg(long)]
    pub kind: AdrBaselineKindInput,
    /// Required for new-adr and escalation; forbidden for all other kinds.
    #[arg(long)]
    pub reason: Option<AdrBaselineReasonInput>,
}

/// ADR baseline restore CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineRestoreArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
    /// Direct Markdown filename under `knowledge/adr`.
    #[arg(long)]
    pub source: AdrSourceFileNameInput,
}

/// ADR baseline review-check CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineCheckReviewArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
    /// Primary ADR filename whose init snapshot is required.
    #[arg(long)]
    pub primary_source: AdrSourceFileNameInput,
}

/// ADR baseline commit-check CLI arguments.
#[derive(Debug, Clone, Args)]
pub struct AdrBaselineCheckCommitArgs {
    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
    /// Track ID; defaults to the active `track/<id>` branch.
    #[arg(long)]
    pub track_id: Option<TrackIdInput>,
}

/// Executes the ADR baseline command family.
pub fn execute(cmd: AdrBaselineCommand) -> ExitCode {
    execute_with_error_chain(cmd).0
}

/// Executes the ADR baseline command while retaining a diagnostic for callers.
pub fn execute_with_error_chain(cmd: AdrBaselineCommand) -> (ExitCode, Option<String>) {
    match dispatch(cmd) {
        Ok(outcome) => {
            if let Some(stdout) = outcome.stdout {
                println!("{stdout}");
            }
            if let Some(stderr) = outcome.stderr {
                eprintln!("{stderr}");
                (ExitCode::from(outcome.exit_code), Some(stderr))
            } else {
                (ExitCode::from(outcome.exit_code), None)
            }
        }
        Err(error) => {
            eprintln!("{error}");
            (ExitCode::FAILURE, Some(error.to_string()))
        }
    }
}

fn dispatch(cmd: AdrBaselineCommand) -> Result<cli_driver::CommandOutcome, CliError> {
    let input = match cmd {
        AdrBaselineCommand::Snapshot(args) => AdrBaselineRequest::Snapshot {
            items_dir: args.items_dir,
            track_id: args.track_id,
            source: args.source,
            kind: args.kind,
            reason: args.reason,
        },
        AdrBaselineCommand::Restore(args) => AdrBaselineRequest::Restore {
            items_dir: args.items_dir,
            track_id: args.track_id,
            source: args.source,
        },
        AdrBaselineCommand::CheckReview(args) => AdrBaselineRequest::CheckReview {
            items_dir: args.items_dir,
            track_id: args.track_id,
            primary_source: args.primary_source,
        },
        AdrBaselineCommand::CheckCommit(args) => {
            AdrBaselineRequest::CheckCommit { items_dir: args.items_dir, track_id: args.track_id }
        }
    };
    AdrBaselineCompositionRoot::new()
        .execute(input)
        .map_err(|error| CliError::Message(error.to_string()))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use clap::Parser;

    use super::*;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: AdrBaselineCommand,
    }

    #[test]
    fn test_adr_baseline_snapshot_parses_typed_arguments() {
        let cli = TestCli::parse_from([
            "adr-baseline",
            "snapshot",
            "--source",
            "decision.md",
            "--kind",
            "new-adr",
            "--reason",
            "user approved this ADR",
        ]);
        assert!(matches!(
            cli.command,
            AdrBaselineCommand::Snapshot(AdrBaselineSnapshotArgs { track_id: None, .. })
        ));
    }

    #[test]
    fn test_adr_baseline_arguments_reject_invalid_filename_and_kind() {
        assert!(
            TestCli::try_parse_from([
                "adr-baseline",
                "snapshot",
                "--source",
                "../decision.md",
                "--kind",
                "init"
            ])
            .is_err()
        );
        assert!(
            TestCli::try_parse_from([
                "adr-baseline",
                "snapshot",
                "--source",
                "decision.md",
                "--kind",
                "unknown"
            ])
            .is_err()
        );
    }

    #[test]
    fn test_adr_baseline_dispatch_maps_resolution_error_to_cli_error() {
        let result = dispatch(AdrBaselineCommand::CheckCommit(AdrBaselineCheckCommitArgs {
            items_dir: PathBuf::from("fixture/items"),
            track_id: Some("fixture-track".parse().unwrap()),
        }));

        assert!(matches!(
            result,
            Err(CliError::Message(message))
                if message == "--items-dir must point to '<project-root>/track/items'; got fixture/items"
        ));
    }
}
