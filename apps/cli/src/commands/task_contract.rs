//! `task-contract` subcommands for the `sotp` CLI.
//!
//! Provides:
//! - `check`: run the pre-review conformance gate for the given optional TDDD
//!   layer and track. When `--layer` is omitted, all 6 canonical TDDD layers
//!   are iterated internally and their outcomes are combined into one result.
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
/// `Check`: run the pre-review conformance gate check for the given optional
/// TDDD layer and track. When `--layer` is omitted, all 6 canonical TDDD
/// layers are checked in sequence and their violations are combined.
#[derive(Debug, Clone, Subcommand)]
pub enum TaskContractCommand {
    /// Run the pre-review conformance gate check for one or all TDDD layers.
    ///
    /// Reads `task-contract.json` for the given track and the per-layer
    /// `<layer>-type-signals.json` artifact(s), then verifies that:
    ///
    /// 1. Every scope-relevant signal entry is attributed to a task.
    /// 2. Every attributed entry exists in the signal document.
    /// 3. All attributed entries have a Blue impl_catalog signal.
    ///
    /// When `--layer` is omitted, all 6 canonical TDDD layers are iterated.
    /// Layers with no signal document are skipped silently.
    ///
    /// Exits 0 on Passed; exits 1 on Blocked (with a violation list printed to
    /// stderr).
    Check(TaskContractCheckArgs),
}

// ── sotp task-contract check ──────────────────────────────────────────────────

/// Arguments for `sotp task-contract check`.
///
/// `layer` optionally identifies the TDDD layer
/// (`domain`, `usecase`, `infrastructure`, `cli_driver`, `cli`, or
/// `cli_composition`); when omitted, all 6 canonical layers are checked in
/// sequence. It is passed as an opaque CLI string and validated as `LayerId`
/// in the primary adapter. `track_id` is the active track identifier.
/// `items_dir` defaults to `"track/items"` (the workspace-wide convention for
/// all track-reading commands).
#[derive(Debug, Clone, Args)]
pub struct TaskContractCheckArgs {
    /// Optional TDDD layer (e.g. `domain`, `usecase`, `infrastructure`,
    /// `cli_driver`, `cli`, `cli_composition`). When omitted, all 6 canonical
    /// TDDD layers are iterated and their results combined.
    #[arg(long)]
    pub layer: Option<String>,

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
/// [`task_contract_check(layer, track_id, items_dir)`](TaskContractCompositionRoot::task_contract_check),
/// and converts the `CommandOutcome` to a process `ExitCode` (0 on Passed,
/// non-zero on Blocked or error).
pub fn execute_task_contract_check(args: TaskContractCheckArgs) -> ExitCode {
    match TaskContractCompositionRoot::new().task_contract_check(
        args.layer,
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
    fn test_task_contract_check_parses_layer_arg() {
        let cmd = parse_task_contract(&[
            "task-contract",
            "check",
            "--layer",
            "domain",
            "--track-id",
            "my-track",
        ]);
        match cmd {
            TaskContractCommand::Check(args) => {
                assert_eq!(args.layer, Some("domain".to_owned()));
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
            "--layer",
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
    fn test_task_contract_check_omitting_layer_is_accepted() {
        // --layer is now optional; omitting it selects all-layers mode.
        let result = TestCli::try_parse_from(["task-contract", "check", "--track-id", "my-track"]);
        assert!(result.is_ok(), "--layer is optional; omitting it should be accepted");
        match result.unwrap().cmd {
            TaskContractCommand::Check(args) => {
                assert_eq!(args.layer, None, "omitting --layer must yield None");
            }
        }
    }

    #[test]
    fn test_task_contract_check_missing_track_id_is_rejected() {
        let result = TestCli::try_parse_from(["task-contract", "check", "--layer", "domain"]);
        assert!(result.is_err(), "--track-id is required and must be rejected when omitted");
    }

    #[test]
    fn test_task_contract_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["task-contract", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized task-contract subcommand must be rejected by clap");
    }
}
