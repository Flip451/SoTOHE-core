//! `task-contract` subcommands for the `sotp` CLI.
//!
//! Provides:
//! - `check`: run the pre-review conformance gate for the given TDDD layer
//!   review group and track, verifying that the contracted catalogue entries
//!   in that layer scope have blue impl_catalog signals.
//!
//! All composition (adapter construction, interactor wiring) lives in
//! `cli_composition`; this module is a thin arg-parsing + dispatch layer
//! (CN-01 / CN-02).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TaskContractCompositionRoot;

use crate::commands::driver_outcome_to_exit;

// ── sotp task-contract ────────────────────────────────────────────────────────

/// Subcommands for `sotp task-contract`.
///
/// `Check`: run the pre-review conformance gate check for the given TDDD layer
/// group and track, verifying the contracted catalogue entries in that layer
/// scope have blue impl_catalog signals.
#[derive(Debug, Clone, Subcommand)]
pub enum TaskContractCommand {
    /// Run the pre-review conformance gate check for a TDDD layer review group.
    ///
    /// Reads `task-contract.json` for the given track and the per-layer
    /// `<group>-type-signals.json` artifact, then verifies that:
    ///
    /// 1. Every scope-relevant signal entry is attributed to a task.
    /// 2. Every attributed entry exists in the signal document.
    /// 3. All attributed entries have a Blue impl_catalog signal.
    ///
    /// Exits 0 on Passed; exits 1 on Blocked (with a violation list printed to
    /// stderr).
    Check(TaskContractCheckArgs),
}

// ── sotp task-contract check ──────────────────────────────────────────────────

/// Arguments for `sotp task-contract check`.
///
/// `group` identifies the TDDD layer review group
/// (`domain`, `usecase`, `infrastructure`, `cli_driver`, `cli`, or
/// `cli_composition`); it is passed as an opaque CLI string and validated as
/// `LayerId` in the primary adapter. `track_id` is the active track identifier.
/// `items_dir` defaults to `"track/items"` (the workspace-wide convention for
/// all track-reading commands).
#[derive(Debug, Clone, Args)]
pub struct TaskContractCheckArgs {
    /// TDDD layer review group (e.g. `domain`, `usecase`, `infrastructure`,
    /// `cli_driver`, `cli`, `cli_composition`).
    #[arg(long)]
    pub group: String,

    /// Active track identifier.
    #[arg(long)]
    pub track_id: String,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Dispatch `sotp task-contract <subcommand>` to the appropriate execute_* handler.
pub fn execute(cmd: TaskContractCommand) -> ExitCode {
    match cmd {
        TaskContractCommand::Check(args) => execute_task_contract_check(args),
    }
}

/// Execute `sotp task-contract check`.
///
/// Constructs a [`TaskContractCompositionRoot`], calls
/// [`task_contract_check(group, track_id, items_dir)`](TaskContractCompositionRoot::task_contract_check),
/// and converts the `CommandOutcome` to a process `ExitCode` (0 on Passed,
/// non-zero on Blocked or error).
pub fn execute_task_contract_check(args: TaskContractCheckArgs) -> ExitCode {
    match TaskContractCompositionRoot::new().task_contract_check(
        args.group,
        args.track_id,
        args.items_dir,
    ) {
        Ok(outcome) => driver_outcome_to_exit(outcome),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use clap::Parser;

    use super::*;

    /// Thin clap wrapper for parsing `sotp task-contract <subcmd>` in tests.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: TaskContractCommand,
    }

    fn parse_task_contract(args: &[&str]) -> TaskContractCommand {
        TestCli::parse_from(args).cmd
    }

    // ── sotp task-contract check: arg parsing ─────────────────────────────────

    #[test]
    fn test_task_contract_check_parses_required_args() {
        let cmd = parse_task_contract(&[
            "task-contract",
            "check",
            "--group",
            "domain",
            "--track-id",
            "my-track",
        ]);
        match cmd {
            TaskContractCommand::Check(args) => {
                assert_eq!(args.group, "domain");
                assert_eq!(args.track_id, "my-track");
                assert_eq!(args.items_dir, PathBuf::from("track/items"));
            }
        }
    }

    #[test]
    fn test_task_contract_check_parses_custom_items_dir() {
        let cmd = parse_task_contract(&[
            "task-contract",
            "check",
            "--group",
            "usecase",
            "--track-id",
            "my-track",
            "--items-dir",
            "custom/track/items",
        ]);
        match cmd {
            TaskContractCommand::Check(args) => {
                assert_eq!(args.items_dir, PathBuf::from("custom/track/items"));
            }
        }
    }

    #[test]
    fn test_task_contract_check_missing_group_is_rejected() {
        let result = TestCli::try_parse_from(["task-contract", "check", "--track-id", "my-track"]);
        assert!(result.is_err(), "--group is required and must be rejected when omitted");
    }

    #[test]
    fn test_task_contract_check_missing_track_id_is_rejected() {
        let result = TestCli::try_parse_from(["task-contract", "check", "--group", "domain"]);
        assert!(result.is_err(), "--track-id is required and must be rejected when omitted");
    }

    #[test]
    fn test_task_contract_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["task-contract", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized task-contract subcommand must be rejected by clap");
    }
}
