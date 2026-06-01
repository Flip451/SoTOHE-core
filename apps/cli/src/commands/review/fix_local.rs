//! `sotp review fix-local` — launch the review-fix-lead fixer with provider
//! auto-resolved from `agent-profiles.json`.
//!
//! Resolves the `review-fix-lead` capability for the given round type and
//! dispatches to the infrastructure adapter (currently: `codex` only) via
//! `CliApp.review_run_fix_local` (CN-02 / CN-03 / AC-03 / AC-04).
//! The 7-flag interface mirrors the current `track-local-review-fix-codex`
//! Makefile bash arguments (CN-04).

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::Args;
use cli_composition::RunReviewFixLocalInput;

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

    /// Track ID (required; used to identify the active track).
    #[arg(long)]
    pub(super) track_id: String,

    /// Round type: fast or final.
    #[arg(long, value_enum)]
    pub(super) round_type: CodexRoundTypeArg,

    /// Model for the nested reviewer subprocess.
    #[arg(long)]
    pub(super) reviewer_model: String,

    /// Model for the fixer (Codex) subprocess.
    /// When omitted the model is resolved from `agent-profiles.json`
    /// `review-fix-lead.model` (or `fast_model` for fast round).
    #[arg(long)]
    pub(super) model: Option<String>,

    /// Comma-separated list of files the fixer may modify (modification boundary).
    #[arg(long, value_delimiter = ',', num_args = 0..)]
    pub(super) scope_files: Vec<PathBuf>,
}

pub(super) fn execute_fix_local(args: &FixLocalArgs) -> ExitCode {
    let input = build_run_review_fix_local_input(args);
    match cli_composition::CliApp::new().review_run_fix_local(input) {
        Ok(outcome) => {
            // Smoke-test failures (exit_code 2) and normal outcomes all arrive
            // as Ok(CommandOutcome) — the typed RunReviewFixError::SmokeTestFailed
            // mapping is made in cli-composition/run_fix.rs before stringification,
            // keeping the exit-code decision on the typed boundary, not a string match.
            match emit_fix_local_outcome(&outcome) {
                Ok(()) => ExitCode::from(outcome.exit_code),
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(1)
                }
            }
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

fn build_run_review_fix_local_input(args: &FixLocalArgs) -> RunReviewFixLocalInput {
    let round_type = match args.round_type {
        CodexRoundTypeArg::Fast => "fast".to_owned(),
        CodexRoundTypeArg::Final => "final".to_owned(),
    };

    RunReviewFixLocalInput {
        scope: args.scope.clone(),
        briefing_file: args.briefing_file.clone(),
        track_id: args.track_id.clone(),
        round_type,
        reviewer_model: args.reviewer_model.clone(),
        model: args.model.clone(),
        scope_files: args.scope_files.clone(),
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
fn emit_fix_local_outcome(outcome: &cli_composition::CommandOutcome) -> Result<(), String> {
    if let Some(msg) = &outcome.stderr {
        eprintln!("{msg}");
    }
    if let Some(line) = &outcome.stdout {
        writeln!(io::stdout(), "{line}").map_err(|e| format!("failed to write stdout: {e}"))?;
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[derive(clap::Parser)]
    struct TestCli {
        #[command(flatten)]
        args: FixLocalArgs,
    }

    #[test]
    fn test_fix_local_args_map_to_run_review_fix_local_input() {
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
            "--reviewer-model",
            "gpt-5.4-mini",
            "--model",
            "gpt-5.5",
            "--scope-files",
            "apps/cli/src/commands/make.rs,apps/cli/src/commands/review/fix_local.rs",
        ]);

        let input = build_run_review_fix_local_input(&cli.args);

        assert_eq!(input.scope, "cli");
        assert_eq!(input.briefing_file, PathBuf::from("tmp/reviewer runtime/briefing cli.md"));
        assert_eq!(input.track_id, "review-fix");
        assert_eq!(input.round_type, "fast");
        assert_eq!(input.reviewer_model, "gpt-5.4-mini");
        assert_eq!(input.model, Some("gpt-5.5".to_owned()));
        assert_eq!(
            input.scope_files,
            vec![
                PathBuf::from("apps/cli/src/commands/make.rs"),
                PathBuf::from("apps/cli/src/commands/review/fix_local.rs")
            ]
        );
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
            "--reviewer-model",
            "gpt-5.4-mini",
        ]);

        let input = build_run_review_fix_local_input(&cli.args);

        assert_eq!(input.round_type, "final");
        assert_eq!(input.model, None);
        assert!(input.scope_files.is_empty());
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
        let outcome = cli_composition::CommandOutcome { stdout: None, stderr: None, exit_code: 2 };
        assert!(emit_fix_local_outcome(&outcome).is_ok());
    }

    /// The CLI propagates whatever exit_code the composition layer placed in the
    /// outcome (0, 1, or 2 — including smoke-test exit 2 from run_fix.rs).
    #[test]
    fn test_emit_fix_local_outcome_returns_ok_for_exit_code_0() {
        let outcome = cli_composition::CommandOutcome { stdout: None, stderr: None, exit_code: 0 };
        assert!(emit_fix_local_outcome(&outcome).is_ok());
    }

    /// Finding 2: when --model is omitted, `model` is `None` (profile model will
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
            "--reviewer-model",
            "gpt-5.4-mini",
        ]);

        let input = build_run_review_fix_local_input(&cli.args);

        assert_eq!(
            input.model, None,
            "omitted --model must produce None so the profile model is used as default"
        );
    }

    /// Finding 2: when --model is explicitly provided, it is forwarded in `input.model`
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
            "--reviewer-model",
            "gpt-5.4-mini",
            "--model",
            "my-override-model",
        ]);

        let input = build_run_review_fix_local_input(&cli.args);

        assert_eq!(
            input.model,
            Some("my-override-model".to_owned()),
            "explicit --model must be forwarded as Some(...) to the input DTO"
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
            "--reviewer-model",
            "gpt-5.4-mini",
        ]);

        assert!(err.is_err(), "missing --briefing-file must be rejected by clap");
    }
}
