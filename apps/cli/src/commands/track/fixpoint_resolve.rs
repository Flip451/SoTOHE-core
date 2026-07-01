//! `sotp track fixpoint-resolve` — resolve the next fixpoint step for the active track.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use cli_composition::{FixpointResolveInput, TrackCompositionRoot};

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
    let outcome = TrackCompositionRoot::new()
        .fixpoint_resolve(FixpointResolveInput {
            track_id: args.track_id,
            current_branch: args.current_branch,
            items_dir: args.items_dir,
        })
        .map_err(|e| CliError::Message(e.to_string()))?;

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
    use clap::Parser as _;

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

    // Domain-level validation tests for `FixpointCurrentBranch::try_new` and
    // output-shape tests for `format_fixpoint_step` live in
    // `apps/cli-composition/src/track/fixpoint_resolve.rs` so the
    // `cli_composition` public surface does not have to re-export `domain` /
    // `usecase` types across the CN-02 boundary.

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
    /// Uses an isolated temp git repository (not the workspace) with
    /// `enabled: true` in `.harness/config/dry-check.json` and no
    /// `dry-check-coverage.json`, so the dry gate returns Blocked → step = `RunDfp`.
    ///
    /// CWD is temporarily changed to the temp repo root (via `run_in_dir`) so that
    /// `SystemGitRepo::discover()` picks up the isolated repo and reads the fixture
    /// config rather than the workspace config (which may have `enabled: false`).
    #[test]
    fn test_execute_fixpoint_resolve_dry_blocked_returns_success_exit_code() {
        use std::process::{Command, ExitCode};

        use crate::commands::track::test_support::{process_env_lock, run_in_dir};

        let _guard = process_env_lock().lock().unwrap();

        // Create a self-contained git repo with the full structure needed.
        let dir = tempfile::tempdir().expect("tempdir must be created");
        let root = dir.path();

        fn run_git(path: &std::path::Path, args: &[&str]) {
            let status = Command::new("git")
                .args(args)
                .current_dir(path)
                .env("GIT_AUTHOR_NAME", "Test")
                .env("GIT_AUTHOR_EMAIL", "test@test.com")
                .env("GIT_COMMITTER_NAME", "Test")
                .env("GIT_COMMITTER_EMAIL", "test@test.com")
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed with {status}");
        }

        run_git(root, &["init", "-q"]);
        run_git(root, &["config", "commit.gpgsign", "false"]);
        run_git(root, &["checkout", "-B", "main"]);
        std::fs::write(root.join("README.md"), "init\n").unwrap();
        run_git(root, &["add", "."]);
        run_git(root, &["commit", "--no-gpg-sign", "-m", "init"]);

        let track_id_str = "dfp-cli-track-2026";
        run_git(root, &["checkout", "-b", &format!("track/{track_id_str}")]);

        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join(track_id_str);
        std::fs::create_dir_all(&track_dir).unwrap();

        // Write HEAD SHA to .commit_hash so diff-base resolution succeeds.
        let head_sha_output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root)
            .output()
            .expect("git rev-parse HEAD must succeed");
        let head_sha = String::from_utf8_lossy(&head_sha_output.stdout).trim().to_owned();
        std::fs::write(track_dir.join(".commit_hash"), &head_sha).unwrap();

        // Write metadata.json with branch_strategy_snapshot so fixpoint_resolve can read base_branch.
        std::fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{"schema_version":6,"id":"{track_id_str}","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{{"base_branch":"main","merge_target":"main","merge_method":"squash"}}}}"#
            ),
        )
        .unwrap();

        // Write `.harness/config/dry-check.json` with `enabled: true` so the dry gate
        // runs (rather than bypassing via the enabled=false short-circuit).
        let harness_config_dir = root.join(".harness").join("config");
        std::fs::create_dir_all(&harness_config_dir).unwrap();
        std::fs::write(
            harness_config_dir.join("dry-check.json"),
            r#"{
  "schema_version": 4,
  "enabled": true,
  "threshold": 0.85,
  "max_parallelism": 4,
  "known_bad_injection_rate_percent": 10,
  "known_bad_detection_threshold_percent": 90
}"#,
        )
        .unwrap();

        // Switch CWD to the temp repo so `SystemGitRepo::discover()` finds this repo.
        // No dry-check-coverage.json → dry gate Blocked → step = "run-dfp".
        let (outcome, exit) = run_in_dir(root, || {
            let outcome = cli_composition::TrackCompositionRoot::new()
                .fixpoint_resolve(cli_composition::FixpointResolveInput {
                    track_id: track_id_str.to_owned(),
                    current_branch: format!("track/{track_id_str}"),
                    items_dir: items_dir.clone(),
                })
                .expect("fixpoint-resolve with enabled=true + missing coverage must succeed");

            let exit = super::execute_fixpoint_resolve(super::FixpointResolveArgs {
                track_id: track_id_str.to_owned(),
                current_branch: format!("track/{track_id_str}"),
                items_dir: items_dir.clone(),
            })
            .expect("execute_fixpoint_resolve must return Ok(ExitCode)");

            (outcome, exit)
        });

        drop(dir);

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("run-dfp"),
            "enabled=true + missing coverage must yield run-dfp"
        );
        assert_eq!(exit, ExitCode::SUCCESS, "execute_fixpoint_resolve must return SUCCESS");
    }
}
