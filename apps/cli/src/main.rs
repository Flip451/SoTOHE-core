#![warn(clippy::too_many_lines)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cli_composition::CliApp;

mod commands;
mod error;

pub use error::CliError;

/// SoTOHE-core CLI: track state machine and workflow management.
#[derive(Parser)]
#[command(name = "sotp", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Domain analysis tools (export-schema, etc.).
    Domain {
        #[command(subcommand)]
        cmd: commands::domain::DomainCommand,
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
    /// Local planner workflow wrappers.
    Plan {
        #[command(subcommand)]
        cmd: commands::plan::PlanCommand,
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
    /// Verification checks for CI validation.
    Verify {
        #[command(subcommand)]
        cmd: commands::verify::VerifyCommand,
    },
    /// Find semantically similar code fragments in the index (information-only).
    FindSimilar(commands::semantic_dup::FindSimilarArgs),
    /// Manage the semantic duplicate detection index (build, measure-quality).
    DupIndex {
        #[command(subcommand)]
        cmd: commands::semantic_dup::DupIndexCommand,
    },
    /// Check diff fragments for semantic near-duplicates (soft gate, exit 0).
    DupCheck(commands::semantic_dup::DupCheckArgs),
    /// DRY violation detection: write, results, check-approved.
    Dry {
        #[command(subcommand)]
        cmd: commands::dry::DryCommand,
    },
    /// Run the example track state machine demo.
    Demo,
}

fn main() -> ExitCode {
    run_cli(Cli::parse(), commands::dry::execute)
}

fn run_cli(cli: Cli, dry_execute: impl FnOnce(commands::dry::DryCommand) -> ExitCode) -> ExitCode {
    match cli.command {
        Some(CliCommand::Domain { cmd }) => commands::domain::execute(cmd),
        Some(CliCommand::Guard { cmd }) => commands::guard::execute(cmd),
        Some(CliCommand::Hook { cmd }) => commands::hook::execute(cmd),
        Some(CliCommand::Track { cmd }) => commands::track::execute(cmd),
        Some(CliCommand::Git { cmd }) => commands::git::execute(cmd),
        Some(CliCommand::Pr { cmd }) => commands::pr::execute(cmd),
        Some(CliCommand::Plan { cmd }) => commands::plan::execute(cmd),
        Some(CliCommand::Review { cmd }) => commands::review::execute(cmd),
        Some(CliCommand::File { cmd }) => commands::file::execute(cmd),
        Some(CliCommand::Verify { cmd }) => commands::verify::execute(cmd),
        Some(CliCommand::FindSimilar(args)) => commands::semantic_dup::execute_find_similar(args),
        Some(CliCommand::DupIndex { cmd }) => commands::semantic_dup::execute_dup_index(cmd),
        Some(CliCommand::DupCheck(args)) => commands::semantic_dup::execute_dup_check(args),
        Some(CliCommand::Dry { cmd }) => dry_execute(cmd),
        Some(CliCommand::Demo) | None => match CliApp::new().demo() {
            Ok(outcome) => {
                if let Some(msg) = outcome.stdout {
                    println!("{msg}");
                }
                ExitCode::from(outcome.exit_code)
            }
            Err(err) => {
                eprintln!("{err}");
                ExitCode::FAILURE
            }
        },
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::process::ExitCode;

    use clap::Parser as _;
    use cli_composition::CliApp;

    use super::{Cli, CliCommand, run_cli};
    use crate::commands::dry::DryCommand;

    #[test]
    fn example_cli_flow_saves_track_successfully() {
        // Delegates to infrastructure::demo::run_example_demo which creates an in-memory
        // track, persists it, derives status "planned", and returns the display string.
        let result = CliApp::new().demo();
        assert!(result.is_ok(), "demo failed: {:?}", result.err());
        let outcome = result.unwrap();
        let msg = outcome.stdout.unwrap_or_default();
        assert!(msg.contains("planned"), "expected 'planned' in output: {msg}");
    }

    // ── CliCommand::Dry entrypoint dispatch routing ───────────────────────────

    /// `sotp dry write --track-id x` must resolve to `CliCommand::Dry { cmd: DryCommand::Write }`.
    /// This also runs the same dispatch helper used by `main()` and checks that
    /// the DRY executor's `ExitCode` is returned to the process entrypoint.
    #[test]
    fn test_dry_dispatch_write_routes_to_dry_write_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "my-track"]).unwrap();
        let exit = run_cli(cli, |cmd| {
            match cmd {
                DryCommand::Write(args) => {
                    assert_eq!(args.track_id, "my-track");
                }
                other => panic!("expected Write, got {other:?}"),
            }
            ExitCode::from(37)
        });
        assert_eq!(exit, ExitCode::from(37));
    }

    /// `sotp dry write --track-id x` must parse into `CliCommand::Dry { cmd: DryCommand::Write }`.
    #[test]
    fn test_dry_dispatch_write_parses_to_dry_write_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "my-track"]).unwrap();
        match cli.command {
            Some(CliCommand::Dry { cmd: DryCommand::Write(args) }) => {
                assert_eq!(args.track_id, "my-track");
            }
            _ => panic!("expected Dry {{ Write }}, got a different variant"),
        }
    }

    /// `sotp dry results --track-id x` must resolve to `CliCommand::Dry { cmd: DryCommand::Results }`.
    #[test]
    fn test_dry_dispatch_results_routes_to_dry_results_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "dry", "results", "--track-id", "my-track"]).unwrap();
        let exit = run_cli(cli, |cmd| {
            match cmd {
                DryCommand::Results(args) => {
                    assert_eq!(args.track_id, "my-track");
                }
                other => panic!("expected Results, got {other:?}"),
            }
            ExitCode::SUCCESS
        });
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    /// `sotp dry results --track-id x` must parse into `CliCommand::Dry { cmd: DryCommand::Results }`.
    #[test]
    fn test_dry_dispatch_results_parses_to_dry_results_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "dry", "results", "--track-id", "my-track"]).unwrap();
        match cli.command {
            Some(CliCommand::Dry { cmd: DryCommand::Results(args) }) => {
                assert_eq!(args.track_id, "my-track");
            }
            _ => panic!("expected Dry {{ Results }}, got a different variant"),
        }
    }

    /// `sotp dry check-approved --track-id x` must resolve to
    /// `CliCommand::Dry { cmd: DryCommand::CheckApproved }`.
    #[test]
    fn test_dry_dispatch_check_approved_routes_to_dry_check_approved_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "check-approved", "--track-id", "my-track"])
            .unwrap();
        let exit = run_cli(cli, |cmd| {
            match cmd {
                DryCommand::CheckApproved(args) => {
                    assert_eq!(args.track_id.as_deref(), Some("my-track"));
                }
                other => panic!("expected CheckApproved, got {other:?}"),
            }
            ExitCode::FAILURE
        });
        assert_eq!(exit, ExitCode::FAILURE);
    }

    /// `sotp dry check-approved --track-id x` must parse into
    /// `CliCommand::Dry { cmd: DryCommand::CheckApproved }`.
    #[test]
    fn test_dry_dispatch_check_approved_parses_to_dry_check_approved_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "check-approved", "--track-id", "my-track"])
            .unwrap();
        match cli.command {
            Some(CliCommand::Dry { cmd: DryCommand::CheckApproved(args) }) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
            }
            _ => panic!("expected Dry {{ CheckApproved }}, got a different variant"),
        }
    }

    /// An unrecognized `sotp dry` subcommand must be rejected by clap (Err),
    /// not silently fall through or panic.
    #[test]
    fn test_dry_dispatch_unknown_subcommand_is_rejected() {
        let result = Cli::try_parse_from(["sotp", "dry", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized dry subcommand must be rejected by clap");
    }
}
