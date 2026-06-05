//! `sotp dry` subcommand group: `write`, `results`, `check-approved`, `fix-local`.
//!
//! Delegates argument parsing to clap, constructs the corresponding
//! `cli_composition` input DTOs, and calls the matching `CliApp` method.
//! All composition (adapter construction, interactor wiring) is performed
//! inside `cli_composition::CliApp`, following the existing pattern (CN-01).
//!
//! No domain types are imported here (CN-02). The `filter` arg is passed as
//! a string to `DryResultsInput`; cli-composition parses it to `VerdictFilter`.

use std::io::{self, Write};
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand, ValueEnum};
use cli_composition::{
    CliApp, DryCheckApprovedInput, DryResultsInput, DryWriteInput, RunDryFixLocalInput,
};

use crate::commands::outcome_to_exit;

// ── sotp dry ──────────────────────────────────────────────────────────────────

/// Subcommands for `sotp dry`.
#[derive(Debug, Subcommand)]
pub enum DryCommand {
    /// Run the DRY-check write cycle: detect near-duplicate violations in the
    /// current diff scope and record results to dry-check.json.
    Write(DryWriteArgs),
    /// Show the historical dry-check results (informational, always exits 0).
    Results(DryResultsArgs),
    /// Gate: exit 0 when all above-threshold pairs are verified; non-zero otherwise.
    CheckApproved(DryCheckApprovedArgs),
    /// Run the dry-fix-lead fixer with provider auto-resolved from agent-profiles.json.
    ///
    /// Resolves `dry-fix-lead` capability from agent-profiles.json, constructs
    /// the fixer (currently Codex only), and executes the DRY fix cycle.
    /// Accepts `--track-id` / `--briefing-file` (required) and an optional
    /// `--model` override. Prints exactly one terminal status line:
    /// `completed`, `blocked`, or `failed`.
    FixLocal(DryFixLocalArgs),
}

// ── sotp dry write ────────────────────────────────────────────────────────────

/// Arguments for `sotp dry write`.
#[derive(Debug, Args)]
pub struct DryWriteArgs {
    /// Track ID used to locate the per-track dry-check.json and .commit_hash.
    #[arg(long)]
    pub track_id: String,

    /// Optional explicit base commit (overrides the .commit_hash store lookup).
    ///
    /// When omitted, the diff base is resolved via the per-track .commit_hash
    /// file with a fail-closed three-branch policy (absent / non-ancestor → main
    /// fallback; malformed → warn + main fallback).
    #[arg(long)]
    pub base_commit: Option<String>,

    /// Path to the local LanceDB semantic index database.
    #[arg(long, default_value = ".semantic_index")]
    pub db_path: PathBuf,

    /// Cosine similarity threshold (0.0–1.0) above which a pair is flagged.
    #[arg(long, default_value_t = 0.85_f32)]
    pub threshold: f32,

    /// Workspace root to scan for Rust sources (corpus + diff fragment extraction).
    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,

    /// Codex model name used by the DryCheckAgentPort.
    /// When omitted the model is resolved from `agent-profiles.json`
    /// `dry-checker.model`.
    #[arg(long)]
    pub model: Option<String>,

    /// Capability name forwarded to the CodexDryChecker.
    #[arg(long, default_value = "dry-checker")]
    pub capability_name: String,
}

/// Execute `sotp dry write`.
pub fn execute_dry_write(args: DryWriteArgs) -> ExitCode {
    outcome_to_exit(CliApp::new().dry_write(DryWriteInput {
        track_id: args.track_id,
        base_commit: args.base_commit,
        db_path: args.db_path,
        threshold: args.threshold,
        workspace_root: args.workspace_root,
        items_dir: args.items_dir,
        model: args.model,
        capability_name: args.capability_name,
    }))
}

// ── sotp dry results ──────────────────────────────────────────────────────────

/// Verdict filter for `sotp dry results --filter ...`.
///
/// The string value is forwarded to `DryResultsInput.filter` and parsed to
/// `domain::dry_check::VerdictFilter` in cli-composition (CN-02: no domain
/// types in the CLI layer).
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum VerdictFilterArg {
    /// Return all records (default).
    All,
    /// Return only not-a-violation records.
    NotAViolation,
    /// Return only accepted records.
    Accepted,
    /// Return only violation records.
    Violation,
}

impl VerdictFilterArg {
    /// Convert to the canonical string expected by `DryResultsInput.filter`.
    pub fn as_filter_str(self) -> &'static str {
        match self {
            VerdictFilterArg::All => "all",
            VerdictFilterArg::NotAViolation => "not-a-violation",
            VerdictFilterArg::Accepted => "accepted",
            VerdictFilterArg::Violation => "violation",
        }
    }
}

/// Arguments for `sotp dry results`.
#[derive(Debug, Args)]
pub struct DryResultsArgs {
    /// Track ID used to locate the per-track dry-check.json.
    #[arg(long)]
    pub track_id: String,

    /// Verdict filter: all / not-a-violation / accepted / violation (default: all).
    #[arg(long, value_enum, default_value_t = VerdictFilterArg::All)]
    pub filter: VerdictFilterArg,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

/// Execute `sotp dry results`.
///
/// INFORMATIONAL — always exits 0 on successful read.
pub fn execute_dry_results(args: DryResultsArgs) -> ExitCode {
    outcome_to_exit(CliApp::new().dry_results(DryResultsInput {
        track_id: args.track_id,
        filter: args.filter.as_filter_str().to_owned(),
        items_dir: args.items_dir,
    }))
}

// ── sotp dry check-approved ───────────────────────────────────────────────────

/// Arguments for `sotp dry check-approved`.
#[derive(Debug, Args)]
pub struct DryCheckApprovedArgs {
    /// Track ID used to locate the per-track dry-check.json and .commit_hash.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Optional explicit base commit (overrides the .commit_hash store lookup).
    ///
    /// When omitted, the diff base is resolved via the per-track .commit_hash
    /// file with a fail-closed three-branch policy (absent / non-ancestor → main
    /// fallback; malformed → warn + main fallback).
    #[arg(long)]
    pub base_commit: Option<String>,

    /// Path to the local LanceDB semantic index database.
    #[arg(long, default_value = ".semantic_index")]
    pub db_path: PathBuf,

    /// Cosine similarity threshold (0.0–1.0).
    #[arg(long, default_value_t = 0.85_f32)]
    pub threshold: f32,

    /// Workspace root to scan for Rust sources (corpus + diff fragment extraction).
    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

/// Execute `sotp dry check-approved`.
///
/// Exits 0 on Approved; exits non-zero on Blocked.
pub fn execute_dry_check_approved(args: DryCheckApprovedArgs) -> ExitCode {
    let track_id = match crate::commands::track::resolve_track_id(args.track_id, &args.items_dir) {
        Ok(id) => id,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::FAILURE;
        }
    };
    outcome_to_exit(CliApp::new().dry_check_approved(DryCheckApprovedInput {
        track_id,
        base_commit: args.base_commit,
        db_path: args.db_path,
        threshold: args.threshold,
        workspace_root: args.workspace_root,
        items_dir: args.items_dir,
    }))
}

// ── sotp dry fix-local ────────────────────────────────────────────────────────

/// Arguments for `sotp dry fix-local`.
#[derive(Debug, Args)]
pub struct DryFixLocalArgs {
    /// Track ID (required; used to identify the active track and locate dry-check.json).
    #[arg(long)]
    pub track_id: String,

    /// Path to the briefing file that the dry-fix-lead fixer should read.
    #[arg(long)]
    pub briefing_file: PathBuf,

    /// Model for the fixer (Codex) subprocess.
    /// When omitted the model is resolved from `agent-profiles.json`
    /// `dry-fix-lead.model`.
    #[arg(long)]
    pub model: Option<String>,
}

/// Execute `sotp dry fix-local`.
///
/// Launches the dry-fix-lead Codex agent, which runs the
/// `sotp dry write` → fix → `sotp dry check-approved` loop until
/// the DRY gate passes, the loop is exhausted, or a tooling error occurs.
/// Emits exactly one of: `completed`, `blocked`, or `failed`.
pub fn execute_dry_fix_local(args: DryFixLocalArgs) -> ExitCode {
    let input = RunDryFixLocalInput {
        track_id: args.track_id,
        briefing_file: args.briefing_file,
        model: args.model,
    };
    match CliApp::new().dry_run_fix_local(input) {
        Ok(outcome) => {
            if let Some(msg) = &outcome.stderr {
                eprintln!("{msg}");
            }
            if let Some(line) = &outcome.stdout {
                if let Err(e) = writeln!(io::stdout(), "{line}") {
                    eprintln!("failed to write stdout: {e}");
                    return ExitCode::from(1);
                }
            }
            ExitCode::from(outcome.exit_code)
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::from(1)
        }
    }
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Execute `sotp dry <subcommand>`.
pub fn execute(cmd: DryCommand) -> ExitCode {
    match cmd {
        DryCommand::Write(args) => execute_dry_write(args),
        DryCommand::Results(args) => execute_dry_results(args),
        DryCommand::CheckApproved(args) => execute_dry_check_approved(args),
        DryCommand::FixLocal(args) => execute_dry_fix_local(args),
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use clap::Parser;

    use super::*;

    // ── Arg-parsing wrapper ───────────────────────────────────────────────────

    /// Thin clap wrapper for unit testing argument parsing of `sotp dry <subcmd>`.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: DryCommand,
    }

    fn parse_dry(args: &[&str]) -> DryCommand {
        TestCli::parse_from(args).cmd
    }

    // ── sotp dry write: arg parsing ───────────────────────────────────────────

    #[test]
    fn test_dry_write_required_args_parse_correctly() {
        let cmd = parse_dry(&["dry", "write", "--track-id", "my-track"]);
        match cmd {
            DryCommand::Write(args) => {
                assert_eq!(args.track_id, "my-track");
                assert!(args.base_commit.is_none(), "base_commit must be absent by default");
                assert_eq!(args.db_path, PathBuf::from(".semantic_index"));
                assert!((args.threshold - 0.85).abs() < 1e-6);
                assert!(
                    args.model.is_none(),
                    "model must be absent by default (resolved from agent-profiles)"
                );
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_write_explicit_model_parses_when_given() {
        let cmd = parse_dry(&["dry", "write", "--track-id", "my-track", "--model", "gpt-5.5"]);
        match cmd {
            DryCommand::Write(args) => {
                assert_eq!(
                    args.model.as_deref(),
                    Some("gpt-5.5"),
                    "explicit --model must be captured as Some(\"gpt-5.5\")"
                );
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_write_optional_base_commit_parses_when_given() {
        let cmd =
            parse_dry(&["dry", "write", "--track-id", "my-track", "--base-commit", "abc1234"]);
        match cmd {
            DryCommand::Write(args) => {
                assert_eq!(args.base_commit.as_deref(), Some("abc1234"));
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_write_custom_threshold_parses() {
        let cmd = parse_dry(&["dry", "write", "--track-id", "my-track", "--threshold", "0.9"]);
        match cmd {
            DryCommand::Write(args) => {
                assert!((args.threshold - 0.9).abs() < 1e-5);
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_write_custom_db_path_parses() {
        let cmd = parse_dry(&["dry", "write", "--track-id", "my-track", "--db-path", "/tmp/my.db"]);
        match cmd {
            DryCommand::Write(args) => {
                assert_eq!(args.db_path, PathBuf::from("/tmp/my.db"));
            }
            other => panic!("expected Write, got {other:?}"),
        }
    }

    // ── sotp dry results: arg parsing ─────────────────────────────────────────

    #[test]
    fn test_dry_results_default_filter_is_all() {
        let cmd = parse_dry(&["dry", "results", "--track-id", "my-track"]);
        match cmd {
            DryCommand::Results(args) => {
                assert_eq!(args.track_id, "my-track");
                assert_eq!(args.filter, VerdictFilterArg::All, "default filter must be All");
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_results_filter_violation_parses() {
        let cmd = parse_dry(&["dry", "results", "--track-id", "my-track", "--filter", "violation"]);
        match cmd {
            DryCommand::Results(args) => {
                assert_eq!(args.filter, VerdictFilterArg::Violation);
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_results_filter_not_a_violation_parses() {
        let cmd =
            parse_dry(&["dry", "results", "--track-id", "my-track", "--filter", "not-a-violation"]);
        match cmd {
            DryCommand::Results(args) => {
                assert_eq!(args.filter, VerdictFilterArg::NotAViolation);
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_results_filter_accepted_parses() {
        let cmd = parse_dry(&["dry", "results", "--track-id", "my-track", "--filter", "accepted"]);
        match cmd {
            DryCommand::Results(args) => {
                assert_eq!(args.filter, VerdictFilterArg::Accepted);
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    // ── sotp dry check-approved: arg parsing ──────────────────────────────────

    #[test]
    fn test_dry_check_approved_explicit_track_id_parses_correctly() {
        let cmd = parse_dry(&["dry", "check-approved", "--track-id", "my-track"]);
        match cmd {
            DryCommand::CheckApproved(args) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
                assert!(args.base_commit.is_none(), "base_commit must be absent by default");
                assert_eq!(args.db_path, PathBuf::from(".semantic_index"));
            }
            other => panic!("expected CheckApproved, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_check_approved_track_id_is_optional() {
        let cmd = parse_dry(&["dry", "check-approved"]);
        match cmd {
            DryCommand::CheckApproved(args) => {
                assert!(args.track_id.is_none(), "track_id must be None when omitted");
            }
            other => panic!("expected CheckApproved, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_check_approved_optional_base_commit_parses_when_given() {
        let cmd = parse_dry(&[
            "dry",
            "check-approved",
            "--track-id",
            "my-track",
            "--base-commit",
            "def5678",
        ]);
        match cmd {
            DryCommand::CheckApproved(args) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
                assert_eq!(args.base_commit.as_deref(), Some("def5678"));
            }
            other => panic!("expected CheckApproved, got {other:?}"),
        }
    }

    // ── VerdictFilterArg → filter string conversion ───────────────────────────

    #[test]
    fn test_verdict_filter_arg_converts_to_filter_string() {
        assert_eq!(VerdictFilterArg::All.as_filter_str(), "all");
        assert_eq!(VerdictFilterArg::NotAViolation.as_filter_str(), "not-a-violation");
        assert_eq!(VerdictFilterArg::Accepted.as_filter_str(), "accepted");
        assert_eq!(VerdictFilterArg::Violation.as_filter_str(), "violation");
    }

    // ── sotp dry fix-local: arg parsing ──────────────────────────────────────

    #[test]
    fn test_dry_fix_local_required_args_parse_correctly() {
        let cmd = parse_dry(&[
            "dry",
            "fix-local",
            "--track-id",
            "my-track",
            "--briefing-file",
            "tmp/briefing.md",
        ]);
        match cmd {
            DryCommand::FixLocal(args) => {
                assert_eq!(args.track_id, "my-track");
                assert_eq!(args.briefing_file, PathBuf::from("tmp/briefing.md"));
                assert!(args.model.is_none(), "model must be absent by default");
            }
            other => panic!("expected FixLocal, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_fix_local_optional_model_parses_when_given() {
        let cmd = parse_dry(&[
            "dry",
            "fix-local",
            "--track-id",
            "my-track",
            "--briefing-file",
            "tmp/briefing.md",
            "--model",
            "gpt-5.5",
        ]);
        match cmd {
            DryCommand::FixLocal(args) => {
                assert_eq!(args.model.as_deref(), Some("gpt-5.5"));
            }
            other => panic!("expected FixLocal, got {other:?}"),
        }
    }

    #[test]
    fn test_dry_fix_local_missing_track_id_is_rejected() {
        let result =
            TestCli::try_parse_from(["dry", "fix-local", "--briefing-file", "tmp/briefing.md"]);
        assert!(result.is_err(), "missing --track-id must be rejected by clap");
    }

    #[test]
    fn test_dry_fix_local_missing_briefing_file_is_rejected() {
        let result = TestCli::try_parse_from(["dry", "fix-local", "--track-id", "my-track"]);
        assert!(result.is_err(), "missing --briefing-file must be rejected by clap");
    }

    // ── --filter: invalid token rejection (P1 gap) ────────────────────────────

    /// An unrecognized `--filter` value must be rejected by clap at parse time.
    /// `VerdictFilterArg` is a `ValueEnum`, so clap enforces the allowed set
    /// before the value ever reaches application code.
    #[test]
    fn test_dry_results_filter_bogus_value_is_rejected_by_clap() {
        let result =
            TestCli::try_parse_from(["dry", "results", "--track-id", "x", "--filter", "bogus"]);
        assert!(result.is_err(), "clap must reject unknown --filter value 'bogus'");
    }

    /// Every valid `--filter` token maps to the correct `VerdictFilterArg` variant
    /// and produces the expected downstream filter string.  This covers all four
    /// variants end-to-end through clap parsing + `as_filter_str()`.
    #[test]
    fn test_dry_results_filter_all_valid_tokens_round_trip() {
        let cases: &[(&str, VerdictFilterArg, &str)] = &[
            ("all", VerdictFilterArg::All, "all"),
            ("not-a-violation", VerdictFilterArg::NotAViolation, "not-a-violation"),
            ("accepted", VerdictFilterArg::Accepted, "accepted"),
            ("violation", VerdictFilterArg::Violation, "violation"),
        ];
        for (token, expected_variant, expected_str) in cases {
            let cmd = parse_dry(&["dry", "results", "--track-id", "x", "--filter", token]);
            match cmd {
                DryCommand::Results(args) => {
                    assert_eq!(
                        args.filter, *expected_variant,
                        "--filter '{token}' must parse to {expected_variant:?}"
                    );
                    assert_eq!(
                        args.filter.as_filter_str(),
                        *expected_str,
                        "--filter '{token}': as_filter_str() must return '{expected_str}'"
                    );
                }
                other => panic!("--filter '{token}': expected Results, got {other:?}"),
            }
        }
    }

    /// clap's ValueEnum matching is case-sensitive by default (no `ignore_case` configured).
    /// Upper-cased tokens such as `ALL` or `Violation` must be REJECTED, enforcing that
    /// callers always use the canonical lowercase form documented in `--help`.
    #[test]
    fn test_dry_results_filter_value_enum_is_case_sensitive() {
        let result =
            TestCli::try_parse_from(["dry", "results", "--track-id", "x", "--filter", "ALL"]);
        assert!(result.is_err(), "clap must reject upper-cased --filter value 'ALL'");

        let result =
            TestCli::try_parse_from(["dry", "results", "--track-id", "x", "--filter", "Violation"]);
        assert!(result.is_err(), "clap must reject mixed-case --filter value 'Violation'");
    }
}
