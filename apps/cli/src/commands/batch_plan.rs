//! `batch-plan` subcommands for the `sotp` CLI (IN-06, AC-05, AC-06).
//!
//! Provides `check`: the Phase 3 terminal gate over the track's declared
//! `batch-plan.json`. It compares every batch's per-scope estimate sum with the
//! resolved diff ceiling and checks the plan against the implementation plan's
//! tasks and their declared dependencies.
//!
//! All composition lives in `cli_composition`; this module parses arguments,
//! resolves the active track and dispatches.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::BatchPlanCompositionRoot;
use cli_driver::batch_plan::BatchPlanInput;

use crate::commands::driver_outcome_to_exit;

// ── sotp batch-plan ───────────────────────────────────────────────────────────

/// Subcommands for `sotp batch-plan`.
#[derive(Debug, Clone, Subcommand)]
pub enum BatchPlanCommand {
    /// Run the Phase 3 gate over the track's declared batch plan.
    ///
    /// Reads `batch-plan.json`, `impl-plan.json` and the review scope
    /// configuration, then reports every structural finding: a batch over its
    /// per-scope ceiling, an over-ceiling scope with more than one contributor,
    /// a task the implementation plan does not have, a planned task in no
    /// batch, and a declared dependency delivered by a later batch.
    ///
    /// Exits 0 when the plan conforms; exits 1 with the findings on stderr.
    Check(BatchPlanCheckArgs),
}

/// Arguments for `sotp batch-plan check`.
#[derive(Debug, Clone, Args)]
pub struct BatchPlanCheckArgs {
    /// Active track identifier. When omitted, auto-resolved from the current
    /// git branch (only `track/<id>` branches are accepted).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Dispatches `sotp batch-plan <subcommand>`.
///
/// Thin: it reads no working directory, resolves no track and renders no
/// failure of its own. The optional track id passes through unresolved — the
/// driver resolves it, validates it and renders every outcome.
pub fn execute(cmd: BatchPlanCommand) -> ExitCode {
    match cmd {
        BatchPlanCommand::Check(args) => {
            let driver = BatchPlanCompositionRoot::new().batch_plan_driver();
            driver_outcome_to_exit(driver.handle(BatchPlanInput::Check {
                track_id: args.track_id,
                items_dir: args.items_dir,
            }))
        }
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use clap::Parser;

    use super::*;

    /// Thin clap wrapper for parsing `sotp batch-plan <subcmd>` in tests.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: BatchPlanCommand,
    }

    #[test]
    fn test_check_defaults_the_items_directory_and_leaves_the_track_unresolved() {
        let BatchPlanCommand::Check(args) = TestCli::parse_from(["sotp", "check"]).cmd;

        assert_eq!(args.items_dir, PathBuf::from("track/items"));
        assert_eq!(args.track_id, None, "an omitted track is resolved from the branch");
    }

    /// Writes a track's two planning artifacts into a fresh items directory.
    fn items_dir_with(batch_plan: &str, impl_plan: &str) -> tempfile::TempDir {
        let dir = tempfile::Builder::new()
            .prefix("batch-plan-cli-")
            .tempdir_in(std::env::current_dir().unwrap())
            .unwrap();
        let track_dir = dir.path().join("some-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("batch-plan.json"), batch_plan).unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), impl_plan).unwrap();
        dir
    }

    #[test]
    fn test_execute_returns_the_terminal_check_result_for_a_batch_over_its_scope_ceiling() {
        // The shipped configuration gives `domain` a 500-line ceiling; B1
        // declares 300 + 250 = 550 across two decomposable tasks.
        let dir = items_dir_with(
            r#"{
              "schema_version": 1,
              "track_id": "some-track",
              "task_estimates": [
                {"task_id": "T001", "scope_estimates": [
                  {"scope": "domain", "production_lines": 300, "test_lines": 0}]},
                {"task_id": "T002", "scope_estimates": [
                  {"scope": "domain", "production_lines": 250, "test_lines": 0}]}
              ],
              "batches": [{"id": "B1", "task_ids": ["T001", "T002"]}]
            }"#,
            r#"{
              "schema_version": 2,
              "tasks": [
                {"id": "T001", "description": "First", "status": "todo"},
                {"id": "T002", "description": "Second", "status": "todo"}
              ],
              "plan": {"summary": [], "sections": [
                {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001", "T002"]}
              ]}
            }"#,
        );

        let exit = execute(BatchPlanCommand::Check(BatchPlanCheckArgs {
            track_id: Some("some-track".to_owned()),
            items_dir: dir.path().to_path_buf(),
        }));

        assert_eq!(exit, ExitCode::FAILURE, "an excess is fail-closed");
    }

    #[test]
    fn test_execute_passes_a_conforming_plan_and_the_single_justified_exception() {
        // 1000 lines against the same 500-line ceiling, declared by the one
        // task that states why it cannot be split.
        let exempt = items_dir_with(
            r#"{
              "schema_version": 1,
              "track_id": "some-track",
              "task_estimates": [
                {"task_id": "T001",
                 "scope_estimates": [
                   {"scope": "domain", "production_lines": 700, "test_lines": 300}],
                 "oversize_justification": "the transition table cannot be split"}
              ],
              "batches": [{"id": "B1", "task_ids": ["T001"]}]
            }"#,
            r#"{
              "schema_version": 2,
              "tasks": [{"id": "T001", "description": "First", "status": "todo"}],
              "plan": {"summary": [], "sections": [
                {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001"]}
              ]}
            }"#,
        );

        let exit = execute(BatchPlanCommand::Check(BatchPlanCheckArgs {
            track_id: Some("some-track".to_owned()),
            items_dir: exempt.path().to_path_buf(),
        }));
        assert_eq!(exit, ExitCode::SUCCESS, "the sole-contributor exemption applies");

        // The same shape without the reason is refused.
        let unjustified = items_dir_with(
            r#"{
              "schema_version": 1,
              "track_id": "some-track",
              "task_estimates": [
                {"task_id": "T001", "scope_estimates": [
                  {"scope": "domain", "production_lines": 700, "test_lines": 300}]}
              ],
              "batches": [{"id": "B1", "task_ids": ["T001"]}]
            }"#,
            r#"{
              "schema_version": 2,
              "tasks": [{"id": "T001", "description": "First", "status": "todo"}],
              "plan": {"summary": [], "sections": [
                {"id": "S1", "title": "Impl", "description": [], "task_ids": ["T001"]}
              ]}
            }"#,
        );

        let exit = execute(BatchPlanCommand::Check(BatchPlanCheckArgs {
            track_id: Some("some-track".to_owned()),
            items_dir: unjustified.path().to_path_buf(),
        }));
        assert_eq!(exit, ExitCode::FAILURE, "the same excess without a reason is refused");
    }

    #[test]
    fn test_execute_runs_the_wired_gate_and_fails_closed_on_an_absent_batch_plan() {
        // The named track has no batch-plan.json, so the wired path reports the
        // absence instead of passing.
        let exit = execute(BatchPlanCommand::Check(BatchPlanCheckArgs {
            track_id: Some("no-such-track".to_owned()),
            items_dir: PathBuf::from("track/items"),
        }));

        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_execute_refuses_an_identifier_that_is_not_a_track_id() {
        let exit = execute(BatchPlanCommand::Check(BatchPlanCheckArgs {
            track_id: Some("Not A Track".to_owned()),
            items_dir: PathBuf::from("track/items"),
        }));

        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_check_accepts_an_explicit_track_and_items_directory() {
        let BatchPlanCommand::Check(args) = TestCli::parse_from([
            "sotp",
            "check",
            "--track-id",
            "some-track",
            "--items-dir",
            "other/items",
        ])
        .cmd;

        assert_eq!(args.track_id.as_deref(), Some("some-track"));
        assert_eq!(args.items_dir, PathBuf::from("other/items"));
    }
}
