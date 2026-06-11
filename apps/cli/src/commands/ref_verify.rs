//! CLI subcommands for `sotp ref-verify`: semantic reference verification.
//!
//! Provides:
//! - `run`: execute the semantic review pipeline (scope resolved from track
//!   artifact existence — spec.json absent → Chain-1 zero pairs, all
//!   catalogues absent → Chain-2 zero pairs, both present → both chains).
//! - `check-approved`: gate that exits 0 only when all current production
//!   reference pairs have a Pass cache entry.
//!
//! All composition (adapter construction, interactor wiring, config I/O) lives
//! in `cli_composition`; this module is a thin arg-parsing + dispatch layer
//! (CN-01 / CN-02).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::{CliApp, RefVerifyCheckApprovedInput, RefVerifyRunInput};

use crate::commands::outcome_to_exit;

// ── sotp ref-verify ───────────────────────────────────────────────────────────

/// Subcommands for `sotp ref-verify`.
#[derive(Debug, Subcommand)]
pub enum RefVerifyCommand {
    /// Execute the semantic reference review pipeline (Chain1 / Chain2 / All).
    ///
    /// Scope is resolved from track artifact existence: spec.json absent →
    /// Chain-1 has zero pairs (a SKIP reason is printed), all catalogues
    /// absent → Chain-2 has zero pairs, both present → both chains.
    /// Configuration (known-bad rates, parallelism) is read from
    /// `.harness/config/ref-verify.json`; defaults are used when the file is absent.
    Run(RunArgs),
    /// Gate: exit 0 only when all current production reference pairs have a
    /// verified Pass cache entry in the verify-cache artifacts.
    ///
    /// Reads `spec-adr-verify-cache.json` and
    /// `<layer>-catalogue-spec-verify-cache.json`. Does not read `review.json`
    /// or invoke any model. Exits 1 when any pair is missing, Pending, or Fail.
    CheckApproved(CheckApprovedArgs),
}

// ── sotp ref-verify run ───────────────────────────────────────────────────────

/// Arguments for `sotp ref-verify run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

// ── sotp ref-verify check-approved ───────────────────────────────────────────

/// Arguments for `sotp ref-verify check-approved`.
#[derive(Debug, Args)]
pub struct CheckApprovedArgs {
    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Execute `sotp ref-verify <subcommand>`.
pub fn execute(cmd: RefVerifyCommand) -> ExitCode {
    match cmd {
        RefVerifyCommand::Run(args) => execute_run(&args),
        RefVerifyCommand::CheckApproved(args) => execute_check_approved(&args),
    }
}

fn execute_run(args: &RunArgs) -> ExitCode {
    let track_id =
        match crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir) {
            Ok(id) => id,
            Err(msg) => {
                eprintln!("{msg}");
                return ExitCode::FAILURE;
            }
        };
    outcome_to_exit(
        CliApp::new()
            .ref_verify_run(RefVerifyRunInput { track_id, items_dir: args.items_dir.clone() }),
    )
}

fn execute_check_approved(args: &CheckApprovedArgs) -> ExitCode {
    let track_id =
        match crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir) {
            Ok(id) => id,
            Err(msg) => {
                eprintln!("{msg}");
                return ExitCode::FAILURE;
            }
        };
    outcome_to_exit(CliApp::new().ref_verify_check_approved(RefVerifyCheckApprovedInput {
        track_id,
        items_dir: args.items_dir.clone(),
    }))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use clap::Parser;

    use super::*;

    /// Thin clap wrapper for parsing `sotp ref-verify <subcmd>` in tests.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: RefVerifyCommand,
    }

    fn parse_ref_verify(args: &[&str]) -> RefVerifyCommand {
        TestCli::parse_from(args).cmd
    }

    // ── sotp ref-verify run: arg parsing ──────────────────────────────────────

    #[test]
    fn test_ref_verify_run_parses_with_no_args() {
        let cmd = parse_ref_verify(&["ref-verify", "run"]);
        match cmd {
            RefVerifyCommand::Run(args) => {
                assert!(args.track_id.is_none(), "track_id must be None when omitted");
                assert_eq!(args.items_dir, PathBuf::from("track/items"));
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_run_rejects_removed_context_arg() {
        // Regression (AC-03): the firing-surface context flag was removed —
        // passing it must be a clap error, not a silently ignored argument.
        let result = TestCli::try_parse_from(["ref-verify", "run", "--context", "commit-gate"]);
        assert!(result.is_err(), "removed --context flag must be rejected by clap");
    }

    #[test]
    fn test_ref_verify_run_rejects_removed_layer_arg() {
        // Regression (AC-03): the per-layer narrowing flag was removed —
        // passing it must be a clap error, not a silently ignored argument.
        let result = TestCli::try_parse_from(["ref-verify", "run", "--layer", "domain"]);
        assert!(result.is_err(), "removed --layer flag must be rejected by clap");
    }

    #[test]
    fn test_ref_verify_run_parses_explicit_track_id() {
        let cmd = parse_ref_verify(&["ref-verify", "run", "--track-id", "my-track"]);
        match cmd {
            RefVerifyCommand::Run(args) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_run_parses_custom_items_dir() {
        let cmd = parse_ref_verify(&["ref-verify", "run", "--items-dir", "custom/track/items"]);
        match cmd {
            RefVerifyCommand::Run(args) => {
                assert_eq!(args.items_dir, PathBuf::from("custom/track/items"));
            }
            other => panic!("expected Run, got {other:?}"),
        }
    }

    // ── sotp ref-verify check-approved: arg parsing ───────────────────────────

    #[test]
    fn test_ref_verify_check_approved_parses_with_no_args() {
        let cmd = parse_ref_verify(&["ref-verify", "check-approved"]);
        match cmd {
            RefVerifyCommand::CheckApproved(args) => {
                assert!(args.track_id.is_none(), "track_id must be None when omitted");
                assert_eq!(args.items_dir, PathBuf::from("track/items"));
            }
            other => panic!("expected CheckApproved, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_check_approved_parses_explicit_track_id() {
        let cmd = parse_ref_verify(&["ref-verify", "check-approved", "--track-id", "my-track"]);
        match cmd {
            RefVerifyCommand::CheckApproved(args) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
            }
            other => panic!("expected CheckApproved, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_check_approved_parses_custom_items_dir() {
        let cmd = parse_ref_verify(&[
            "ref-verify",
            "check-approved",
            "--items-dir",
            "custom/track/items",
        ]);
        match cmd {
            RefVerifyCommand::CheckApproved(args) => {
                assert_eq!(args.items_dir, PathBuf::from("custom/track/items"));
            }
            other => panic!("expected CheckApproved, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["ref-verify", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized ref-verify subcommand must be rejected by clap");
    }
}
