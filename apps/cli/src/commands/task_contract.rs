//! `task-contract` subcommands for the `sotp` CLI.
//!
//! Provides:
//! - `check`: run the pre-review liveness gate for the given optional TDDD
//!   layer and track. When `--layer` is omitted, all 6 canonical TDDD layers
//!   are iterated internally and their outcomes are combined into one result.
//!   Impl-plan task status is used to filter attributions: done/in_progress
//!   tasks require Blue signal; todo tasks tolerate Yellow (Red always blocks).
//! - `coverage`: run the attribution-completeness check for the active track.
//!   Checks that every catalogue entry is attributed to at least one task, and
//!   every attributed entry exists in the catalogue (across all 6 TDDD layers).
//!
//! All composition (adapter construction, interactor wiring) lives in
//! `cli_composition`; this module is a thin arg-parsing + dispatch layer
//! (CN-01 / CN-02).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::{TaskContractCompositionRoot, TrackCompositionRoot};

use crate::commands::driver_outcome_to_exit;

// ── sotp task-contract ────────────────────────────────────────────────────────

/// Subcommands for `sotp task-contract`.
///
/// `Check`: run the pre-review liveness gate check for the given optional
/// TDDD layer and track. Impl-plan task status is used to filter attributions.
///
/// `Coverage`: run the attribution-completeness check for the active track.
#[derive(Debug, Clone, Subcommand)]
pub enum TaskContractCommand {
    /// Run the pre-review liveness gate check for one or all TDDD layers.
    ///
    /// Reads `task-contract.json` and per-layer `<layer>-type-signals.json`,
    /// then verifies that all attributed entries for current/done tasks have
    /// a Blue `impl_catalog` signal. Todo-only attributed entries tolerate
    /// Yellow; Red is always a blocker regardless of task status.
    ///
    /// When `--layer` is omitted, all 6 canonical TDDD layers are iterated.
    /// Layers with no signal document are skipped silently.
    ///
    /// Exits 0 on Passed; exits 1 on Blocked (violation list to stderr).
    Check(TaskContractCheckArgs),

    /// Run the attribution-completeness coverage check for the active track.
    ///
    /// Reads `task-contract.json` and all per-layer `<layer>-type-signals.json`
    /// artifacts (across all 6 canonical TDDD layers), then verifies:
    ///
    /// 1. Every catalogue entry is attributed to at least one task.
    /// 2. Every attributed entry exists in the catalogue.
    ///
    /// Exits 0 on Passed; exits 1 on Blocked (violation list to stderr).
    Coverage(TaskContractCoverageArgs),
}

// ── sotp task-contract check ──────────────────────────────────────────────────

/// Arguments for `sotp task-contract check`.
///
/// `layer` optionally identifies the TDDD layer
/// (`domain`, `usecase`, `infrastructure`, `cli_driver`, `cli`, or
/// `cli_composition`); when omitted, all 6 canonical layers are checked in
/// sequence. It is passed as an opaque CLI string and validated as `LayerId`
/// in the primary adapter. `track_id` is optional; when omitted, the active
/// track is auto-resolved from the current git branch (`track/<id>`), matching
/// the convention of `bin/sotp ref-verify run` and other track-aware commands.
/// `items_dir` defaults to `"track/items"` (the workspace-wide convention for
/// all track-reading commands).
///
/// Impl-plan task status is consulted to filter attributions: done/in_progress
/// attributed entries require Blue signal; todo-only entries tolerate Yellow
/// (Red is always a blocker regardless of status, per D7).
#[derive(Debug, Clone, Args)]
pub struct TaskContractCheckArgs {
    /// Optional TDDD layer (e.g. `domain`, `usecase`, `infrastructure`,
    /// `cli_driver`, `cli`, `cli_composition`). When omitted, all 6 canonical
    /// TDDD layers are iterated and their results combined.
    #[arg(long)]
    pub layer: Option<String>,

    /// Active track identifier. When omitted, auto-resolved from the current
    /// git branch (only `track/<id>` branches are accepted).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

// ── sotp task-contract coverage ───────────────────────────────────────────────

/// Arguments for `sotp task-contract coverage`.
///
/// Checks attribution completeness across all 6 canonical TDDD layers.
/// `track_id` is optional; when omitted, the active track is auto-resolved
/// from the current git branch (`track/<id>`).
/// `items_dir` defaults to `"track/items"`.
#[derive(Debug, Clone, Args)]
pub struct TaskContractCoverageArgs {
    /// Active track identifier. When omitted, auto-resolved from the current
    /// git branch (only `track/<id>` branches are accepted).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Dispatch `sotp task-contract <subcommand>` to the appropriate execute_* handler.
pub fn execute(cmd: TaskContractCommand) -> ExitCode {
    match cmd {
        TaskContractCommand::Check(args) => execute_task_contract_check(args),
        TaskContractCommand::Coverage(args) => execute_task_contract_coverage(args),
    }
}

/// Execute `sotp task-contract check`.
///
/// Resolves `track_id` (auto from `track/<id>` branch when not explicit).
/// Fail-closed when no active track can be resolved (PR #175 round 20 P1):
/// non-track context (e.g. `main`, detached HEAD, git read failure) returns
/// `ExitCode::FAILURE` instead of silently passing. CI routes only invoke
/// this from `ci-track-container` / `ci-track-local` after the workflow has
/// recreated the `track/<id>` branch, so the fail-closed path never fires
/// in normal CI; non-track invocations are caller bugs.
pub fn execute_task_contract_check(args: TaskContractCheckArgs) -> ExitCode {
    let track_id_opt = match args.track_id {
        Some(id) => Some(id),
        None => detect_active_track_from_branch_cwd(),
    };
    task_contract_check_core(track_id_opt, args.layer, args.items_dir)
}

/// Core logic for `sotp task-contract check`, separated for testability.
///
/// Returns `ExitCode::FAILURE` with a diagnostic on stderr when `track_id_opt`
/// is `None`. CI invokes this only from `ci-track-container` / `ci-track-local`
/// after the workflow recreates the `track/<id>` branch, so a `None` here
/// indicates a caller bug (called on `main` / detached HEAD / git failure).
fn task_contract_check_core(
    track_id_opt: Option<String>,
    layer: Option<String>,
    items_dir: PathBuf,
) -> ExitCode {
    let resolved_track_id = match track_id_opt {
        Some(id) => id,
        None => {
            eprintln!(
                "[BLOCKED] sotp task-contract check requires an active track \
                 (resolved from a `track/<id>` git branch); \
                 pass --track-id or run on a track branch"
            );
            return ExitCode::FAILURE;
        }
    };
    match TaskContractCompositionRoot::new().task_contract_check(
        layer,
        resolved_track_id,
        items_dir,
    ) {
        Ok(outcome) => driver_outcome_to_exit(outcome),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Execute `sotp task-contract coverage`.
///
/// Resolves `track_id` (auto from `track/<id>` branch when not explicit).
/// Fail-closed when no active track can be resolved (PR #175 round 20 P1):
/// same policy as `task-contract check`.
pub fn execute_task_contract_coverage(args: TaskContractCoverageArgs) -> ExitCode {
    let track_id_opt = match args.track_id {
        Some(id) => Some(id),
        None => detect_active_track_from_branch_cwd(),
    };
    task_contract_coverage_core(track_id_opt, args.items_dir)
}

/// Core logic for `sotp task-contract coverage`, separated for testability.
///
/// Returns `ExitCode::FAILURE` with a diagnostic on stderr when `track_id_opt`
/// is `None`. Same policy as `task_contract_check_core`.
fn task_contract_coverage_core(track_id_opt: Option<String>, items_dir: PathBuf) -> ExitCode {
    let resolved_track_id = match track_id_opt {
        Some(id) => id,
        None => {
            eprintln!(
                "[BLOCKED] sotp task-contract coverage requires an active track \
                 (resolved from a `track/<id>` git branch); \
                 pass --track-id or run on a track branch"
            );
            return ExitCode::FAILURE;
        }
    };
    match TaskContractCompositionRoot::new().task_contract_coverage(resolved_track_id, items_dir) {
        Ok(outcome) => driver_outcome_to_exit(outcome),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

/// Auto-resolve the active track id from the current git branch.
///
/// Returns `Some("<id>")` when on a `track/<id>` branch, `None` otherwise
/// (`main`, detached HEAD, git failure, etc.). Mirrors the convention used by
/// `sotp ref-verify run`, `sotp track views sync`, and other track-aware
/// commands.
fn detect_active_track_from_branch_cwd() -> Option<String> {
    let project_root = std::env::current_dir().ok()?;
    TrackCompositionRoot::new().detect_active_track_from_branch(&project_root)
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
                assert_eq!(args.track_id, Some("my-track".to_owned()));
                assert_eq!(args.items_dir, PathBuf::from("track/items"));
            }
            other => panic!("expected Check, got {other:?}"),
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
            other => panic!("expected Check, got {other:?}"),
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
            other => panic!("expected Check, got {other:?}"),
        }
    }

    #[test]
    fn test_task_contract_check_omitting_track_id_is_accepted() {
        // --track-id is now optional; omitting it triggers auto-resolution from
        // the current git branch (`track/<id>`) at runtime. Clap-level parse
        // must accept this; resolution itself is exercised by integration tests
        // / shell invocations on real track branches.
        let result = TestCli::try_parse_from(["task-contract", "check", "--layer", "domain"]);
        assert!(result.is_ok(), "--track-id is optional; omitting it should be accepted");
        match result.unwrap().cmd {
            TaskContractCommand::Check(args) => {
                assert_eq!(args.track_id, None, "omitting --track-id must yield None");
                assert_eq!(args.layer, Some("domain".to_owned()));
            }
            other => panic!("expected Check, got {other:?}"),
        }
    }

    #[test]
    fn test_task_contract_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["task-contract", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized task-contract subcommand must be rejected by clap");
    }

    // ── sotp task-contract coverage: arg parsing ──────────────────────────────

    #[test]
    fn test_task_contract_coverage_parses_track_id_arg() {
        let cmd = parse_task_contract(&["task-contract", "coverage", "--track-id", "my-track"]);
        match cmd {
            TaskContractCommand::Coverage(args) => {
                assert_eq!(args.track_id, Some("my-track".to_owned()));
                assert_eq!(args.items_dir, PathBuf::from("track/items"));
            }
            other => panic!("expected Coverage, got {other:?}"),
        }
    }

    #[test]
    fn test_task_contract_coverage_parses_custom_items_dir() {
        let cmd = parse_task_contract(&[
            "task-contract",
            "coverage",
            "--track-id",
            "my-track",
            "--items-dir",
            "custom/track/items",
        ]);
        match cmd {
            TaskContractCommand::Coverage(args) => {
                assert_eq!(args.items_dir, PathBuf::from("custom/track/items"));
            }
            other => panic!("expected Coverage, got {other:?}"),
        }
    }

    #[test]
    fn test_task_contract_coverage_omitting_track_id_is_accepted() {
        let result = TestCli::try_parse_from(["task-contract", "coverage"]);
        assert!(result.is_ok(), "--track-id is optional; omitting it should be accepted");
        match result.unwrap().cmd {
            TaskContractCommand::Coverage(args) => {
                assert_eq!(args.track_id, None, "omitting --track-id must yield None");
            }
            other => panic!("expected Coverage, got {other:?}"),
        }
    }

    // ── Fail-closed on non-track branch (PR #175 round 20 P1) ─────────────────

    #[test]
    fn coverage_non_track_branch_fails_closed() {
        // Simulate a non-track branch: track_id_opt = None (auto-resolve returned None).
        // Expected: ExitCode::FAILURE — feedback_no_graceful_skip_for_active_track
        // requires that "active track unknown → error" so misrouted invocations
        // (main / detached HEAD / git read failure) cannot silently pass the gate.
        let code = task_contract_coverage_core(None, PathBuf::from("track/items"));
        assert_eq!(code, ExitCode::FAILURE, "non-track branch must fail closed, not silently pass");
    }

    #[test]
    fn check_non_track_branch_fails_closed() {
        let code = task_contract_check_core(None, None, PathBuf::from("track/items"));
        assert_eq!(code, ExitCode::FAILURE, "non-track branch must fail closed, not silently pass");
    }
}
