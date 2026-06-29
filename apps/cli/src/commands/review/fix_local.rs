//! `sotp review fix-local` — launch the review-fix-lead fixer with provider
//! auto-resolved from `agent-profiles.json`.
//!
//! Resolves the `review-fix-lead` capability for the given round type and
//! dispatches to the infrastructure adapter (currently: `codex` only) via
//! `CliApp.review_run_fix_local` (CN-02 / CN-03 / AC-03 / AC-04).
//! Required flags: `--scope`, `--briefing-file`, `--round-type`.
//! `--track-id` is optional: when omitted, the active track is auto-resolved
//! from the current git branch (`track/<id>`). The reviewer model and scope
//! boundary are self-resolved by the fixer skill.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use cli_composition::ReviewCompositionRoot;
#[cfg(test)]
use cli_driver::review::ReviewInput;

use super::CodexRoundTypeArg;

/// Arguments for `sotp review fix-local`.
#[derive(Debug, Args)]
pub struct FixLocalArgs {
    /// Scope name (e.g., "cli", "infrastructure").
    #[arg(long)]
    pub(super) scope: String,

    /// Path to the briefing file that the fixer should read.
    #[arg(long)]
    pub(super) briefing_file: PathBuf,

    /// Track ID. When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub(super) track_id: Option<String>,

    /// Round type: fast or final.
    #[arg(long, value_enum)]
    pub(super) round_type: CodexRoundTypeArg,

    /// Model for the fixer (Codex) subprocess.
    /// When omitted the model is resolved from `agent-profiles.json`
    /// `review-fix-lead.model` (or `fast_model` for fast round).
    #[arg(long)]
    pub(super) model: Option<String>,

    /// Path to track items directory (used for branch auto-resolve when `--track-id` is omitted).
    #[arg(long, default_value = "track/items")]
    pub(super) items_dir: PathBuf,
}

/// Build the driver input from already-resolved args.
///
/// Test-only helper retained for backwards-compatible parametric tests
/// against the input shape. Production `execute_fix_local` delegates the
/// resolve + dispatch to `cli_composition::ReviewCompositionRoot::review_run_fix_local_resolve`
/// without constructing the input here (thin-bin policy).
#[cfg(test)]
fn build_review_fix_local_input(args: &FixLocalArgs, track_id: String) -> ReviewInput {
    let round_type = match args.round_type {
        CodexRoundTypeArg::Fast => "fast".to_owned(),
        CodexRoundTypeArg::Final => "final".to_owned(),
    };
    ReviewInput::RunFixLocal {
        scope: args.scope.clone(),
        briefing_file: args.briefing_file.clone(),
        track_id,
        round_type,
        model: args.model.clone(),
    }
}

pub(super) fn execute_fix_local(args: &FixLocalArgs) -> ExitCode {
    // Thin-bin: delegate auto-resolve + fail-closed to cli_composition.
    let round_type = match args.round_type {
        CodexRoundTypeArg::Fast => "fast".to_owned(),
        CodexRoundTypeArg::Final => "final".to_owned(),
    };
    let outcome = match ReviewCompositionRoot::new().review_run_fix_local_resolve(
        args.track_id.clone(),
        args.scope.clone(),
        args.briefing_file.clone(),
        round_type,
        args.model.clone(),
        args.items_dir.clone(),
    ) {
        Ok(o) => o,
        Err(e) => {
            eprintln!("{e}");
            return ExitCode::FAILURE;
        }
    };
    match emit_fix_local_outcome(&outcome) {
        Ok(()) => ExitCode::from(outcome.exit_code),
        Err(e) => {
            eprintln!("{e}");
            ExitCode::from(1)
        }
    }
}

/// Writes `outcome.stderr` then `outcome.stdout` to the appropriate streams.
///
/// `stderr` (e.g. the smoke-test failure message placed there by the composition
/// layer) is printed before stdout so the diagnostic always appears even when
/// the caller redirects stdout.
///
/// # Errors
/// Returns `Err` if writing to stdout fails.
fn emit_fix_local_outcome(outcome: &cli_driver::CommandOutcome) -> Result<(), crate::CliError> {
    if let Some(msg) = &outcome.stderr {
        eprintln!("{msg}");
    }
    if let Some(line) = &outcome.stdout {
        writeln!(io::stdout(), "{line}").map_err(crate::CliError::Io)?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        args: FixLocalArgs,
    }

    #[test]
    fn test_fix_local_args_map_to_review_fix_local_input() {
        let cli = <TestCli as clap::Parser>::parse_from([
            "test",
            "--scope",
            "cli",
            "--briefing-file",
            "tmp/reviewer runtime/briefing cli.md",
            "--track-id",
            "review-fix",
            "--round-type",
            "fast",
            "--model",
            "gpt-5.5",
        ]);

        let input = build_review_fix_local_input(&cli.args, "review-fix".to_owned());

        match input {
            ReviewInput::RunFixLocal { scope, briefing_file, track_id, round_type, model } => {
                assert_eq!(scope, "cli");
                assert_eq!(briefing_file, PathBuf::from("tmp/reviewer runtime/briefing cli.md"));
                assert_eq!(track_id, "review-fix");
                assert_eq!(round_type, "fast");
                assert_eq!(model, Some("gpt-5.5".to_owned()));
            }
            _ => panic!("expected RunFixLocal"),
        }
    }

    #[test]
    fn test_fix_local_args_default_model_and_final_round_map_correctly() {
        let cli = <TestCli as clap::Parser>::parse_from([
            "test",
            "--scope",
            "cli",
            "--briefing-file",
            "tmp/reviewer-runtime/briefing.md",
            "--track-id",
            "review-fix",
            "--round-type",
            "final",
        ]);

        let input = build_review_fix_local_input(&cli.args, "review-fix".to_owned());

        match input {
            ReviewInput::RunFixLocal { round_type, model, .. } => {
                assert_eq!(round_type, "final");
                assert_eq!(model, None);
            }
            _ => panic!("expected RunFixLocal"),
        }
    }

    #[test]
    fn test_fix_local_args_missing_required_flag_is_rejected() {
        let err = <TestCli as clap::Parser>::try_parse_from([
            "test",
            "--scope",
            "cli",
            "--track-id",
            "review-fix",
            "--round-type",
            "fast",
            "--model",
            "gpt-5.5",
        ]);

        assert!(err.is_err());
    }

    /// `emit_fix_local_outcome` must return `Ok(())` for any valid outcome and the
    /// caller reads `outcome.exit_code` directly (exit_code 2 for smoke-test, etc.).
    #[test]
    fn test_emit_fix_local_outcome_returns_ok_for_exit_code_2() {
        let outcome = cli_driver::CommandOutcome { stdout: None, stderr: None, exit_code: 2 };
        assert!(emit_fix_local_outcome(&outcome).is_ok());
    }

    /// The CLI propagates whatever exit_code the composition layer placed in the
    /// outcome (0, 1, or 2 — including smoke-test exit 2 from run_fix.rs).
    #[test]
    fn test_emit_fix_local_outcome_returns_ok_for_exit_code_0() {
        let outcome = cli_driver::CommandOutcome { stdout: None, stderr: None, exit_code: 0 };
        assert!(emit_fix_local_outcome(&outcome).is_ok());
    }

    /// When --model is omitted, `model` is `None` (profile model will
    /// be used as the default in run_fix.rs).
    #[test]
    fn test_model_absent_maps_to_none_in_input() {
        let cli = <TestCli as clap::Parser>::parse_from([
            "test",
            "--scope",
            "cli",
            "--briefing-file",
            "tmp/reviewer-runtime/briefing.md",
            "--track-id",
            "review-fix",
            "--round-type",
            "fast",
        ]);

        let input = build_review_fix_local_input(&cli.args, "review-fix".to_owned());

        match input {
            ReviewInput::RunFixLocal { model, .. } => {
                assert_eq!(
                    model, None,
                    "omitted --model must produce None so the profile model is used as default"
                );
            }
            _ => panic!("expected RunFixLocal"),
        }
    }

    /// When --model is explicitly provided, it is forwarded in `input.model`
    /// so run_fix.rs can honor the override over the profile model.
    #[test]
    fn test_explicit_model_is_forwarded_to_input() {
        let cli = <TestCli as clap::Parser>::parse_from([
            "test",
            "--scope",
            "cli",
            "--briefing-file",
            "tmp/reviewer-runtime/briefing.md",
            "--track-id",
            "review-fix",
            "--round-type",
            "fast",
            "--model",
            "my-override-model",
        ]);

        let input = build_review_fix_local_input(&cli.args, "review-fix".to_owned());

        match input {
            ReviewInput::RunFixLocal { model, .. } => {
                assert_eq!(
                    model,
                    Some("my-override-model".to_owned()),
                    "explicit --model must be forwarded as Some(...) to the input DTO"
                );
            }
            _ => panic!("expected RunFixLocal"),
        }
    }

    /// Omitting `--briefing-file` must cause clap to reject the command with
    /// a deterministic validation error (it is now a required argument).
    #[test]
    fn test_fix_local_args_missing_briefing_file_is_rejected() {
        let err = <TestCli as clap::Parser>::try_parse_from([
            "test",
            "--scope",
            "cli",
            "--track-id",
            "review-fix",
            "--round-type",
            "fast",
        ]);

        assert!(err.is_err(), "missing --briefing-file must be rejected by clap");
    }

    // -----------------------------------------------------------------------
    // --track-id optional / branch auto-resolve
    // -----------------------------------------------------------------------

    /// Omitting `--track-id` must parse successfully with `track_id = None`
    /// so that branch auto-resolve can be attempted at runtime.
    #[test]
    fn test_fix_local_args_track_id_optional_parses_as_none_when_omitted() {
        let cli = <TestCli as clap::Parser>::parse_from([
            "test",
            "--scope",
            "cli",
            "--briefing-file",
            "tmp/reviewer-runtime/briefing.md",
            "--round-type",
            "fast",
        ]);

        assert_eq!(
            cli.args.track_id, None,
            "omitted --track-id must produce None so branch auto-resolve can be attempted"
        );
    }

    /// Explicit `--track-id` is forwarded unchanged through `build_review_fix_local_input`.
    #[test]
    fn test_fix_local_args_explicit_track_id_maps_to_run_fix_local_input() {
        let cli = <TestCli as clap::Parser>::parse_from([
            "test",
            "--scope",
            "domain",
            "--briefing-file",
            "tmp/reviewer-runtime/briefing.md",
            "--track-id",
            "my-feature-2026",
            "--round-type",
            "fast",
        ]);

        assert_eq!(cli.args.track_id, Some("my-feature-2026".to_owned()));

        let input = build_review_fix_local_input(&cli.args, "my-feature-2026".to_owned());

        match input {
            ReviewInput::RunFixLocal { track_id, .. } => {
                assert_eq!(track_id, "my-feature-2026");
            }
            _ => panic!("expected RunFixLocal"),
        }
    }

    /// On a non-`track/*` branch, `execute_fix_local` with `track_id = None` must
    /// return a failure exit code (branch auto-resolve fails on non-track branches).
    #[test]
    fn test_execute_fix_local_returns_failure_on_non_track_branch() {
        use std::env;
        use std::fs;
        use std::process::Command;
        use std::sync::{Mutex, OnceLock};

        fn cwd_lock() -> &'static Mutex<()> {
            static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
            LOCK.get_or_init(|| Mutex::new(()))
        }

        let _guard = cwd_lock().lock().unwrap();
        let original_dir = env::current_dir().unwrap();

        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();

        // Initialise a minimal git repo on "main" (a non-track branch).
        Command::new("git").args(["init", "-b", "main"]).current_dir(root).output().unwrap();
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(root)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(root)
            .output()
            .unwrap();

        // Need at least one commit so git branch is set.
        fs::write(root.join(".gitkeep"), "").unwrap();
        Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
        Command::new("git").args(["commit", "-m", "init"]).current_dir(root).output().unwrap();

        // Create track/items so resolve_project_root does not fail.
        fs::create_dir_all(root.join("track/items")).unwrap();

        env::set_current_dir(root).unwrap();

        let args = FixLocalArgs {
            scope: "cli".to_owned(),
            briefing_file: PathBuf::from("/nonexistent/briefing.md"),
            track_id: None, // auto-resolve expected to fail (not a track branch)
            round_type: CodexRoundTypeArg::Fast,
            model: None,
            items_dir: PathBuf::from("track/items"),
        };

        let exit = execute_fix_local(&args);

        env::set_current_dir(&original_dir).unwrap();

        assert_ne!(
            exit,
            std::process::ExitCode::SUCCESS,
            "auto-resolve on a non-track branch must return a failure exit code"
        );
    }
}
