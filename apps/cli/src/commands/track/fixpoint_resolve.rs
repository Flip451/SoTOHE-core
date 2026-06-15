//! `sotp track fixpoint-resolve` — resolve the next fixpoint step for the active track.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use cli_composition::{CliApp, FixpointResolveInput};

use crate::CliError;

/// Arguments for `sotp track fixpoint-resolve`.
#[derive(Debug, Args, Clone)]
pub struct FixpointResolveArgs {
    /// Path to the track items root directory (e.g., `track/items`).
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,

    /// Active track ID (directory name under `items_dir`).
    #[arg(long)]
    pub track_id: String,

    /// Current git branch (e.g., `track/my-feature-2026`).
    #[arg(long)]
    pub current_branch: String,
}

/// Execute `sotp track fixpoint-resolve`.
///
/// # Errors
///
/// Returns [`CliError`] when `CliApp::fixpoint_resolve` fails.
pub fn execute_fixpoint_resolve(args: FixpointResolveArgs) -> Result<ExitCode, CliError> {
    let outcome = CliApp::new()
        .fixpoint_resolve(FixpointResolveInput {
            track_id: args.track_id,
            current_branch: args.current_branch,
            items_dir: args.items_dir,
        })
        .map_err(CliError::Message)?;

    if let Some(msg) = outcome.stdout {
        println!("{msg}");
    }
    if let Some(msg) = outcome.stderr {
        eprintln!("{msg}");
    }
    Ok(ExitCode::from(outcome.exit_code))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use std::collections::BTreeSet;

    use clap::Parser as _;
    use cli_composition::{FixpointStep, ReviewScopeSet, format_fixpoint_step};

    // ── Clap argument parsing tests ───────────────────────────────────────────

    /// Minimal CLI wrapper to enable `try_parse_from` in tests.
    #[derive(clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        args: super::FixpointResolveArgs,
    }

    fn try_parse(argv: &[&str]) -> Result<super::FixpointResolveArgs, clap::Error> {
        TestCli::try_parse_from(argv).map(|c| c.args)
    }

    /// Both `--track-id` and `--current-branch` are required; parsing must fail
    /// when both are absent.
    #[test]
    fn test_fixpoint_resolve_args_require_track_id_and_current_branch() {
        let result = try_parse(&["test"]);
        assert!(
            result.is_err(),
            "parse must fail when both --track-id and --current-branch are absent"
        );
    }

    /// Parsing must fail when only `--track-id` is supplied.
    #[test]
    fn test_fixpoint_resolve_args_require_current_branch_when_track_id_supplied() {
        let result = try_parse(&["test", "--track-id", "foo"]);
        assert!(result.is_err(), "parse must fail when --current-branch is absent");
    }

    /// Parsing must fail when only `--current-branch` is supplied.
    #[test]
    fn test_fixpoint_resolve_args_require_track_id_when_current_branch_supplied() {
        let result = try_parse(&["test", "--current-branch", "track/foo"]);
        assert!(result.is_err(), "parse must fail when --track-id is absent");
    }

    /// Parsing must succeed when both required flags are present.
    #[test]
    fn test_fixpoint_resolve_args_parse_with_both_required_flags() {
        let args = try_parse(&["test", "--track-id", "foo", "--current-branch", "track/foo"])
            .expect("parse must succeed with both required flags");
        assert_eq!(args.track_id, "foo");
        assert_eq!(args.current_branch, "track/foo");
        // items_dir defaults to "track/items"
        assert_eq!(args.items_dir.as_os_str(), "track/items");
    }

    /// Clap accepts `--current-branch ""` (empty string), but the domain layer
    /// `FixpointCurrentBranch::try_new` rejects it.  This test documents that
    /// the clap-level validation is permissive and the domain-level validation
    /// is the actual gate.
    #[test]
    fn test_fixpoint_resolve_args_empty_current_branch_is_accepted_by_clap_but_rejected_by_domain()
    {
        use cli_composition::{FixpointCurrentBranch, FixpointResolveError};

        // Clap parse succeeds (empty string is a valid clap string value).
        let args = try_parse(&["test", "--track-id", "foo", "--current-branch", ""])
            .expect("clap must accept an empty --current-branch string");
        assert_eq!(args.current_branch, "");

        // Domain validation rejects it.
        let err = FixpointCurrentBranch::try_new(args.current_branch).unwrap_err();
        assert!(
            matches!(err, FixpointResolveError::InvalidCurrentBranch { .. }),
            "domain must reject empty branch: {err:?}"
        );
    }

    // ── format_fixpoint_step tests ────────────────────────────────────────────

    /// `RunDfp` must format as `"run-dfp"`.
    #[test]
    fn test_format_fixpoint_step_run_dfp() {
        assert_eq!(format_fixpoint_step(FixpointStep::RunDfp), "run-dfp");
    }

    /// `RunRfp` with a single scope must format as `"run-rfp scopes=<scope>"`.
    #[test]
    fn test_format_fixpoint_step_run_rfp_single_scope() {
        let mut set = BTreeSet::new();
        set.insert("plan-artifacts".to_owned());
        let scopes = ReviewScopeSet::try_new(set).unwrap();
        assert_eq!(
            format_fixpoint_step(FixpointStep::RunRfp { scopes }),
            "run-rfp scopes=plan-artifacts"
        );
    }

    /// `RunRfp` with multiple scopes must format in BTreeSet (sorted) order.
    #[test]
    fn test_format_fixpoint_step_run_rfp_multiple_scopes_in_btreeset_order() {
        let mut set = BTreeSet::new();
        set.insert("code".to_owned());
        set.insert("plan-artifacts".to_owned());
        let scopes = ReviewScopeSet::try_new(set).unwrap();
        // "code" < "plan-artifacts" in BTreeSet order.
        assert_eq!(
            format_fixpoint_step(FixpointStep::RunRfp { scopes }),
            "run-rfp scopes=code,plan-artifacts"
        );
    }

    /// `RunRefVerify` must format as `"run-ref-verify"`.
    #[test]
    fn test_format_fixpoint_step_run_ref_verify() {
        assert_eq!(format_fixpoint_step(FixpointStep::RunRefVerify), "run-ref-verify");
    }

    /// `Commit` must format as `"commit"`.
    #[test]
    fn test_format_fixpoint_step_commit() {
        assert_eq!(format_fixpoint_step(FixpointStep::Commit), "commit");
    }

    // ── execute_fixpoint_resolve dispatch tests ───────────────────────────────

    /// `execute_fixpoint_resolve` propagates `CliApp::fixpoint_resolve` errors
    /// as `CliError::Message` (exit code 1).
    ///
    /// A `--track-id` of `""` is rejected by the composition layer before any
    /// git or filesystem access, so this works without a real track directory.
    #[test]
    fn test_execute_fixpoint_resolve_invalid_track_id_returns_cli_error() {
        use crate::CliError;
        let result = super::execute_fixpoint_resolve(super::FixpointResolveArgs {
            track_id: "".to_owned(),
            current_branch: "track/x".to_owned(),
            items_dir: std::path::PathBuf::from("track/items"),
        });

        assert!(
            matches!(result, Err(CliError::Message(_))),
            "empty track_id must yield Err(CliError::Message), got: {result:?}"
        );
    }

    /// `execute_fixpoint_resolve` returns `ExitCode::SUCCESS` when
    /// `CliApp::fixpoint_resolve` returns `CommandOutcome::success`.
    ///
    /// We use a wrong-branch scenario to exercise the stdout-emission path:
    /// the composition layer returns `Err(...)` for a wrong branch, so we
    /// cannot test the success exit code without a full fixture.  Instead, we
    /// document that the stdout is written by verifying the integration via
    /// `cli_composition` tests (`test_fixpoint_resolve_missing_coverage_record_returns_run_dfp`)
    /// and this unit confirms the error-path exit code only.
    #[test]
    fn test_execute_fixpoint_resolve_wrong_branch_returns_cli_error() {
        use crate::CliError;
        let result = super::execute_fixpoint_resolve(super::FixpointResolveArgs {
            track_id: "my-track-2026".to_owned(),
            current_branch: "main".to_owned(),
            items_dir: std::path::PathBuf::from("track/items"),
        });

        assert!(
            matches!(result, Err(CliError::Message(_))),
            "wrong branch must yield Err(CliError::Message), got: {result:?}"
        );
    }

    /// Happy-path: `execute_fixpoint_resolve` returns `ExitCode::SUCCESS` and
    /// writes the step to stdout when `CliApp::fixpoint_resolve` succeeds.
    ///
    /// Uses a temp fixture tree under `target/` (inside the workspace git repo)
    /// with a `.commit_hash` pointing at HEAD and no `dry-check-coverage.json`,
    /// so the dry gate returns Blocked → step = `RunDfp` → stdout `"run-dfp"`.
    /// This confirms that `execute_fixpoint_resolve` correctly emits stdout and
    /// returns `ExitCode::SUCCESS` (exit_code 0).
    #[test]
    fn test_execute_fixpoint_resolve_dry_blocked_returns_success_exit_code() {
        use std::process::ExitCode;

        use cli_composition::CliApp;

        // Locate the workspace root (repo root for tests).
        let mut workspace_root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        workspace_root.pop(); // apps/cli → apps
        workspace_root.pop(); // apps → workspace root

        // Create a temp fixture tree under target/ that satisfies resolve_project_root.
        let base = workspace_root.join("target").join("fixpoint-resolve-cli-tests");
        std::fs::create_dir_all(&base).unwrap();
        let temp_fixture = tempfile::Builder::new()
            .prefix("fixture-")
            .tempdir_in(&base)
            .expect("fixture temp dir must be created");
        let track_id_str = "dfp-cli-track-2026";
        let items_dir = temp_fixture.path().join("track").join("items");
        let track_dir = items_dir.join(track_id_str);
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write the workspace HEAD SHA to .commit_hash.
        let head_sha = std::process::Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&workspace_root)
            .output()
            .expect("git rev-parse HEAD must succeed");
        let head_sha = String::from_utf8_lossy(&head_sha.stdout).trim().to_owned();
        std::fs::write(track_dir.join(".commit_hash"), &head_sha).unwrap();

        // Call the composition directly (same as execute_fixpoint_resolve does).
        let outcome = CliApp::new()
            .fixpoint_resolve(cli_composition::FixpointResolveInput {
                track_id: track_id_str.to_owned(),
                current_branch: format!("track/{track_id_str}"),
                items_dir: items_dir.clone(),
            })
            .expect("fixpoint-resolve with missing coverage must succeed");

        // Verify the outcome that execute_fixpoint_resolve would emit.
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("run-dfp"));

        // Now verify that execute_fixpoint_resolve returns ExitCode::SUCCESS.
        let exit = super::execute_fixpoint_resolve(super::FixpointResolveArgs {
            track_id: track_id_str.to_owned(),
            current_branch: format!("track/{track_id_str}"),
            items_dir,
        })
        .expect("execute_fixpoint_resolve must return Ok(ExitCode)");

        assert_eq!(exit, ExitCode::SUCCESS);
    }
}
