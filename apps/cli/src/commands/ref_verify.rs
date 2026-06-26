//! CLI subcommands for `sotp ref-verify`: semantic reference verification.
//!
//! Provides:
//! - `run`: execute the semantic review pipeline (scope resolved from track
//!   artifact existence — spec.json absent → Chain-1 zero pairs, all
//!   catalogues absent → Chain-2 zero pairs, both present → both chains).
//! - `check-approved`: gate that exits 0 only when all current production
//!   reference pairs have a Pass cache entry.
//! - `results`: display cached verify results filtered by chain, layer, and
//!   verdict (exit code always 0 per D2 / CN-02).
//!
//! All composition (adapter construction, interactor wiring, config I/O) lives
//! in `cli_composition`; this module is a thin arg-parsing + dispatch layer
//! (CN-01 / CN-02).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand, ValueEnum};
use cli_composition::RefVerifyCompositionRoot;
use cli_driver::ref_verify::{
    RefVerifyChainSelect, RefVerifyCheckApprovedInput, RefVerifyInput, RefVerifyResultsInput,
    RefVerifyRunInput, RefVerifyVerdictSelect,
};

use crate::commands::driver_outcome_to_exit;

// ── sotp ref-verify ───────────────────────────────────────────────────────────

/// Clap ValueEnum for the `--chain {1|2|all}` option.
///
/// `Chain1` maps to the clap value `1`, `Chain2` maps to `2`, `All` maps to
/// `all` via `#[value(name = ...)]` attributes. Converted to
/// `cli_driver::ref_verify::RefVerifyChainSelect` when constructing
/// `RefVerifyResultsInput`.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum RefVerifyChainArg {
    /// Include only Chain-1 (spec↔ADR) pairs.
    #[value(name = "1")]
    Chain1,
    /// Include only Chain-2 (catalogue↔spec) pairs.
    #[value(name = "2")]
    Chain2,
    /// Include both Chain-1 and Chain-2 pairs.
    #[value(name = "all")]
    All,
}

/// Clap ValueEnum for the explicit `--filter {pass|fail|pending|all}` option.
///
/// Variants use automatic kebab-case: `pass` / `fail` / `pending` / `all`.
/// The option itself is optional on `RefVerifyResultsArgs`; absence maps to
/// `cli_driver::ref_verify::RefVerifyVerdictSelect::FailPending` to preserve
/// the ADR default record block.
#[derive(Debug, Clone, PartialEq, Eq, ValueEnum)]
pub enum RefVerifyVerdictFilterArg {
    /// Include only pass records.
    Pass,
    /// Include only fail records.
    Fail,
    /// Include only pending records.
    Pending,
    /// Include all records regardless of verdict.
    All,
}

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
    /// Display cached verify results filtered by chain, layer, and verdict.
    ///
    /// Reads the verify-cache artifacts without invoking any model. Exit code
    /// is always 0 regardless of result content (D2 / CN-02); non-zero is only
    /// returned for wiring or I/O errors. The default filter (omitted
    /// `--filter`) shows fail and pending records (FailPending ADR default).
    Results(RefVerifyResultsArgs),
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

// ── sotp ref-verify results ───────────────────────────────────────────────────

/// Arguments for `sotp ref-verify results`.
///
/// Named with the `RefVerify` prefix to avoid collision with the existing
/// `commands::review::ResultsArgs` type. `track_id` is optional (auto-resolved
/// from branch when omitted). `layer` is a plain `String` (default `all`)
/// because valid layer names are resolved dynamically from
/// `architecture-rules.json` at runtime. `chain` uses a typed clap ValueEnum
/// arg with default `all`; `filter` is optional, has no clap default, and maps
/// `None` to `cli_driver::ref_verify::RefVerifyVerdictSelect::FailPending` so
/// default output lists fail/pending pairs while explicit `--filter all` still
/// lists all verdicts.
#[derive(Debug, Args)]
pub struct RefVerifyResultsArgs {
    /// Track ID.
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Path to the track items directory.
    #[arg(long, default_value = "track/items")]
    pub items_dir: PathBuf,

    /// Which chain(s) to include: `1` (spec↔ADR), `2` (catalogue↔spec), or
    /// `all` (both chains).
    #[arg(long, value_enum, default_value = "all")]
    pub chain: RefVerifyChainArg,

    /// Filter results to a specific layer (e.g. `domain`, `usecase`).
    /// Valid layer names are resolved dynamically from `architecture-rules.json`
    /// at runtime; `all` selects all layers.
    #[arg(long, default_value = "all")]
    pub layer: String,

    /// Filter record block by verdict: `pass`, `fail`, `pending`, or `all`.
    /// When omitted, shows fail and pending records (the ADR default).
    #[arg(long = "filter", value_enum)]
    pub filter: Option<RefVerifyVerdictFilterArg>,
}

// ── Dispatch ──────────────────────────────────────────────────────────────────

/// Execute `sotp ref-verify <subcommand>`.
pub fn execute(cmd: RefVerifyCommand) -> ExitCode {
    match cmd {
        RefVerifyCommand::Run(args) => execute_run(&args),
        RefVerifyCommand::CheckApproved(args) => execute_check_approved(&args),
        RefVerifyCommand::Results(args) => execute_results(&args),
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
    driver_outcome_to_exit(RefVerifyCompositionRoot::new().ref_verify_driver().handle(
        RefVerifyInput::Run(RefVerifyRunInput { track_id, items_dir: args.items_dir.clone() }),
    ))
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
    driver_outcome_to_exit(RefVerifyCompositionRoot::new().ref_verify_driver().handle(
        RefVerifyInput::CheckApproved(RefVerifyCheckApprovedInput {
            track_id,
            items_dir: args.items_dir.clone(),
        }),
    ))
}

fn execute_results(args: &RefVerifyResultsArgs) -> ExitCode {
    let track_id =
        match crate::commands::track::resolve_track_id(args.track_id.clone(), &args.items_dir) {
            Ok(id) => id,
            Err(msg) => {
                eprintln!("{msg}");
                return ExitCode::FAILURE;
            }
        };
    let chain = match &args.chain {
        RefVerifyChainArg::Chain1 => RefVerifyChainSelect::Chain1,
        RefVerifyChainArg::Chain2 => RefVerifyChainSelect::Chain2,
        RefVerifyChainArg::All => RefVerifyChainSelect::All,
    };
    let verdict = match &args.filter {
        None => RefVerifyVerdictSelect::FailPending,
        Some(RefVerifyVerdictFilterArg::Pass) => RefVerifyVerdictSelect::Pass,
        Some(RefVerifyVerdictFilterArg::Fail) => RefVerifyVerdictSelect::Fail,
        Some(RefVerifyVerdictFilterArg::Pending) => RefVerifyVerdictSelect::Pending,
        Some(RefVerifyVerdictFilterArg::All) => RefVerifyVerdictSelect::All,
    };
    driver_outcome_to_exit(RefVerifyCompositionRoot::new().ref_verify_driver().handle(
        RefVerifyInput::Results(RefVerifyResultsInput {
            track_id,
            items_dir: args.items_dir.clone(),
            chain,
            layer: args.layer.clone(),
            verdict,
        }),
    ))
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

    // ── sotp ref-verify results: arg parsing ──────────────────────────────────

    #[test]
    fn test_ref_verify_results_parses_with_no_args_uses_defaults() {
        let cmd = parse_ref_verify(&["ref-verify", "results"]);
        match cmd {
            RefVerifyCommand::Results(args) => {
                assert!(args.track_id.is_none(), "track_id must be None when omitted");
                assert_eq!(args.items_dir, PathBuf::from("track/items"));
                assert_eq!(args.chain, RefVerifyChainArg::All);
                assert_eq!(args.layer, "all");
                assert!(args.filter.is_none(), "filter must be None when omitted");
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_results_explicit_chain_1_parses_to_chain1() {
        let cmd = parse_ref_verify(&["ref-verify", "results", "--chain", "1"]);
        match cmd {
            RefVerifyCommand::Results(args) => {
                assert_eq!(args.chain, RefVerifyChainArg::Chain1);
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_results_explicit_filter_pass_parses_to_some_pass() {
        let cmd = parse_ref_verify(&["ref-verify", "results", "--filter", "pass"]);
        match cmd {
            RefVerifyCommand::Results(args) => {
                assert_eq!(args.filter, Some(RefVerifyVerdictFilterArg::Pass));
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_results_explicit_filter_all_parses_to_some_all() {
        let cmd = parse_ref_verify(&["ref-verify", "results", "--filter", "all"]);
        match cmd {
            RefVerifyCommand::Results(args) => {
                assert_eq!(args.filter, Some(RefVerifyVerdictFilterArg::All));
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_results_explicit_layer_stores_value() {
        let cmd = parse_ref_verify(&["ref-verify", "results", "--layer", "domain"]);
        match cmd {
            RefVerifyCommand::Results(args) => {
                assert_eq!(args.layer, "domain");
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_results_explicit_track_id_stores_value() {
        let cmd = parse_ref_verify(&["ref-verify", "results", "--track-id", "my-track"]);
        match cmd {
            RefVerifyCommand::Results(args) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
            }
            other => panic!("expected Results, got {other:?}"),
        }
    }

    #[test]
    fn test_ref_verify_results_unknown_format_flag_is_rejected() {
        let result = TestCli::try_parse_from(["ref-verify", "results", "--format", "json"]);
        assert!(result.is_err(), "unknown --format flag must be rejected by clap");
    }
}
