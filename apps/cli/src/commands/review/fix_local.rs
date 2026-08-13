//! `sotp review fix-local` — launch the review-fix-lead fixer with provider
//! auto-resolved from `agent-profiles.json`.
//!
//! Resolves the `review-fix-lead` capability for the given round type and
//! dispatches to the infrastructure adapter (currently: `codex` only) via
//! the dedicated review-fix driver factory (CN-02 / CN-03 /
//! AC-03 / AC-04).
//! Required flags: `--scope`, `--briefing-file`, `--round-type`.
//! `--track-id` is optional: when omitted, the active track is auto-resolved
//! from the current git branch (`track/<id>`). The reviewer model and scope
//! boundary are self-resolved by the fixer skill.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use cli_driver::review::ReviewFixInput;

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
    pub(crate) items_dir: PathBuf,
}

pub(super) fn execute_fix_local(args: &FixLocalArgs) -> ExitCode {
    let input = ReviewFixInput::new(
        args.scope.clone(),
        args.briefing_file.clone(),
        args.track_id.clone(),
        args.items_dir.clone(),
        match args.round_type {
            CodexRoundTypeArg::Fast => "fast".to_owned(),
            CodexRoundTypeArg::Final => "final".to_owned(),
        },
        args.model.clone(),
    );
    let outcome = cli_composition::ReviewCompositionRoot::new().review_fix_driver().handle(input);
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

    fn review_fix_input(args: &FixLocalArgs, track_id: String) -> ReviewFixInput {
        ReviewFixInput::new(
            args.scope.clone(),
            args.briefing_file.clone(),
            Some(track_id),
            args.items_dir.clone(),
            match args.round_type {
                CodexRoundTypeArg::Fast => "fast".to_owned(),
                CodexRoundTypeArg::Final => "final".to_owned(),
            },
            args.model.clone(),
        )
    }

    #[test]
    fn test_fix_local_args_map_to_raw_review_fix_input() {
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

        let (scope, briefing_file, track_id, _items_dir, round_type, model) =
            review_fix_input(&cli.args, "review-fix".to_owned()).into_parts();

        assert_eq!(scope, "cli");
        assert_eq!(briefing_file, PathBuf::from("tmp/reviewer runtime/briefing cli.md"));
        assert_eq!(track_id.as_deref(), Some("review-fix"));
        assert_eq!(round_type, "fast");
        assert_eq!(model.as_deref(), Some("gpt-5.5"));
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

        let (_, _, _, _, round_type, model) =
            review_fix_input(&cli.args, "review-fix".to_owned()).into_parts();

        assert_eq!(round_type, "final");
        assert_eq!(model, None);
    }

    #[test]
    fn test_fix_local_args_preserve_invalid_scope_for_driver_validation() {
        let cli = <TestCli as clap::Parser>::parse_from([
            "test",
            "--scope",
            "   ",
            "--briefing-file",
            "tmp/reviewer-runtime/briefing.md",
            "--track-id",
            "review-fix",
            "--round-type",
            "fast",
        ]);

        assert_eq!(review_fix_input(&cli.args, "review-fix".to_owned()).into_parts().0, "   ");
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

    /// When --model is omitted, `model` stays absent for profile resolution.
    #[test]
    fn test_model_absent_maps_to_none_in_command() {
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

        let (_, _, _, _, _, model) =
            review_fix_input(&cli.args, "review-fix".to_owned()).into_parts();

        assert_eq!(
            model, None,
            "omitted --model must produce None so the profile model is used as default"
        );
    }

    /// When --model is explicitly provided, it becomes a validated model override.
    #[test]
    fn test_explicit_model_is_forwarded_to_command() {
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

        let (_, _, _, _, _, model) =
            review_fix_input(&cli.args, "review-fix".to_owned()).into_parts();

        assert_eq!(
            model.as_deref(),
            Some("my-override-model"),
            "explicit --model must become a validated command override"
        );
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

    /// Explicit `--track-id` is forwarded unchanged through the validated command.
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

        let (_, _, track_id, _, _, _) =
            review_fix_input(&cli.args, "my-feature-2026".to_owned()).into_parts();

        assert_eq!(track_id.as_deref(), Some("my-feature-2026"));
    }

    /// On a non-`track/*` branch, `execute_fix_local` with `track_id = None` must
    /// return a failure exit code (branch auto-resolve fails on non-track branches).
    #[test]
    fn test_execute_fix_local_returns_failure_on_non_track_branch() {
        use crate::commands::track::test_support::process_env_lock;
        use std::env;
        use std::fs;
        use std::process::Command;

        let _guard = process_env_lock().lock().unwrap();
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
