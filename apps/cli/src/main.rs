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
    /// Replaces Makefile.toml shell wrappers with safe Rust dispatch.
    Make(commands::make::MakeArgs),
    /// Run the example track state machine demo.
    Demo,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

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
        Some(CliCommand::Make(args)) => commands::make::execute(args),
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
    use cli_composition::CliApp;

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
}
