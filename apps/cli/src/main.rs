#![warn(clippy::too_many_lines)]

use std::process::ExitCode;

use clap::{Parser, Subcommand};
use cli_composition::CliApp;

mod commands;
mod error;

pub use error::CliError;

/// SoTOHE-core CLI: track state machine and workflow management.
#[derive(Parser)]
#[command(name = "sotp", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<CliCommand>,
}

#[derive(Subcommand)]
enum CliCommand {
    /// Architecture rules analysis tools.
    Arch {
        #[command(subcommand)]
        cmd: commands::arch::ArchCommand,
    },
    /// Convention document management tools.
    Conventions {
        #[command(subcommand)]
        cmd: commands::conventions::ConventionsCommand,
    },
    /// Domain analysis tools (export-schema, etc.).
    Domain {
        #[command(subcommand)]
        cmd: commands::domain::DomainCommand,
    },
    /// Shell command guard for git operation blocking.
    Guard {
        #[command(subcommand)]
        cmd: commands::guard::GuardCommand,
    },
    /// Security-critical hook dispatch (Rust fail-closed).
    Hook {
        #[command(subcommand)]
        cmd: commands::hook::HookCommand,
    },
    /// Track operations (transition, etc.) with file-system persistence.
    Track {
        #[command(subcommand)]
        cmd: commands::track::TrackCommand,
    },
    /// Guarded local git workflow wrappers.
    Git {
        #[command(subcommand)]
        cmd: commands::git::GitCommand,
    },
    /// Pull-request workflow wrappers.
    Pr {
        #[command(subcommand)]
        cmd: commands::pr::PrCommand,
    },
    /// Local planner workflow wrappers.
    Plan {
        #[command(subcommand)]
        cmd: commands::plan::PlanCommand,
    },
    /// Local review workflow wrappers.
    Review {
        #[command(subcommand)]
        cmd: commands::review::ReviewCommand,
    },
    /// File utility operations (atomic write, etc.).
    File {
        #[command(subcommand)]
        cmd: commands::file::FileCommand,
    },
    /// Verification checks for CI validation.
    Verify {
        #[command(subcommand)]
        cmd: commands::verify::VerifyCommand,
    },
    /// Find semantically similar code fragments in the index (information-only).
    FindSimilar(commands::semantic_dup::FindSimilarArgs),
    /// Manage the semantic duplicate detection index (build, measure-quality).
    DupIndex {
        #[command(subcommand)]
        cmd: commands::semantic_dup::DupIndexCommand,
    },
    /// Check diff fragments for semantic near-duplicates (soft gate, exit 0).
    DupCheck(commands::semantic_dup::DupCheckArgs),
    /// DRY violation detection: write, results, check-approved.
    Dry {
        #[command(subcommand)]
        cmd: commands::dry::DryCommand,
    },
    /// Semantic reference verification: run, check-approved.
    RefVerify {
        #[command(subcommand)]
        cmd: commands::ref_verify::RefVerifyCommand,
    },
    /// Run the example track state machine demo.
    Demo,
}

fn main() -> ExitCode {
    // Initialise tracing subscriber once at the composition root entry point
    // (IN-01 / CN-04 / AC-01: subscriber init lives here, not in domain or usecase).
    cli_composition::telemetry_wiring::init_tracing_subscriber();

    run_cli(Cli::parse(), commands::dry::execute)
}

fn run_cli(cli: Cli, dry_execute: impl FnOnce(commands::dry::DryCommand) -> ExitCode) -> ExitCode {
    run_cli_with(cli, dry_execute, commands::ref_verify::execute)
}

fn run_cli_with(
    cli: Cli,
    dry_execute: impl FnOnce(commands::dry::DryCommand) -> ExitCode,
    ref_verify_execute: impl FnOnce(commands::ref_verify::RefVerifyCommand) -> ExitCode,
) -> ExitCode {
    match cli.command {
        Some(CliCommand::Arch { cmd }) => commands::arch::execute(cmd),
        Some(CliCommand::Conventions { cmd }) => commands::conventions::execute(cmd),
        Some(CliCommand::Domain { cmd }) => commands::domain::execute(cmd),
        Some(CliCommand::Guard { cmd }) => commands::guard::execute(cmd),
        Some(CliCommand::Hook { cmd }) => commands::hook::execute(cmd),
        Some(CliCommand::Track { cmd }) => execute_track_with_telemetry(cmd),
        Some(CliCommand::Git { cmd }) => commands::git::execute(cmd),
        Some(CliCommand::Pr { cmd }) => commands::pr::execute(cmd),
        Some(CliCommand::Plan { cmd }) => commands::plan::execute(cmd),
        Some(CliCommand::Review { cmd }) => commands::review::execute(cmd),
        Some(CliCommand::File { cmd }) => commands::file::execute(cmd),
        Some(CliCommand::Verify { cmd }) => commands::verify::execute(cmd),
        Some(CliCommand::FindSimilar(args)) => commands::semantic_dup::execute_find_similar(args),
        Some(CliCommand::DupIndex { cmd }) => commands::semantic_dup::execute_dup_index(cmd),
        Some(CliCommand::DupCheck(args)) => commands::semantic_dup::execute_dup_check(args),
        Some(CliCommand::Dry { cmd }) => dry_execute(cmd),
        Some(CliCommand::RefVerify { cmd }) => ref_verify_execute(cmd),
        Some(CliCommand::Demo) | None => match CliApp::new().demo() {
            Ok(outcome) => {
                if let Some(msg) = outcome.stdout {
                    println!("{msg}");
                }
                ExitCode::from(outcome.exit_code)
            }
            Err(err) => {
                eprintln!("{err}");
                ExitCode::FAILURE
            }
        },
    }
}

/// Dispatch a `TrackCommand` with telemetry instrumentation.
///
/// Pure display commands (OS-04) are dispatched directly without emitting any
/// telemetry event (ensuring no file IO for those paths — AC-06).
///
/// All other track operation subcommands are timed and emit:
/// - `TelemetryEvent::TrackSubcommand` on completion (AC-02).
/// - `TelemetryEvent::NonZeroExit` additionally when exit code != 0 (IN-03).
///
/// Telemetry is emitted only when the current branch is `track/<id>` (AC-11).
/// On non-track branches or when telemetry env is disabled, dispatch falls
/// through without any file IO (AC-06 / AC-11).
fn execute_track_with_telemetry(cmd: commands::track::TrackCommand) -> ExitCode {
    use cli_composition::telemetry_wiring::{
        emit_non_zero_exit, emit_track_subcommand, resolve_telemetry_writer,
    };
    use std::time::Instant;

    // Classify the command: pure display commands are excluded from telemetry
    // per OS-04 (IN-03, AC-06).
    let command_label = track_command_label(&cmd);
    let is_display_only = is_display_only_track_command(&cmd);

    if is_display_only {
        // Pure display command: dispatch directly, no telemetry IO.
        return commands::track::execute(cmd);
    }

    // Track operation command: resolve telemetry writer (branch-bound, AC-11).
    // Use the command's own items_dir so non-default --items-dir or
    // --workspace-root / --project-root invocations write telemetry to the
    // correct path (P1 fix: workspace_root/project_root variants now derive
    // the correct items_dir rather than returning the constant "track/items").
    let items_dir = cmd.items_dir();
    let telemetry = resolve_telemetry_writer(&items_dir);

    let start = Instant::now();
    // Use execute_with_error_chain so the error message is available for
    // NonZeroExit.error_chain (IN-03).  The error is also printed to stderr
    // by execute_with_error_chain (same behaviour as execute()).
    let (exit_code, error_chain) = commands::track::execute_with_error_chain(cmd);
    // `ExitCode` does not expose its numeric value in stable Rust.
    // Map to a conventional i32: SUCCESS → 0, everything else → 1.
    // This is sufficient for the telemetry event (AC-02 requires exit_code field;
    // exact code > 1 is preserved when ExitCode stabilises numeric access).
    let exit_code_i32: i32 = if exit_code == ExitCode::SUCCESS { 0 } else { 1 };

    // Emit telemetry on completion — NOT at start (exit_code/duration are only
    // known after dispatch completes; IN-03 / T004 description).
    if let Some((ref w, ref track_id)) = telemetry {
        emit_track_subcommand(w, track_id, command_label, exit_code_i32, start);

        if exit_code_i32 != 0 {
            // Populate error_chain from the dispatch error (IN-03).
            // Falls back to "" when the dispatch error has no string representation
            // (e.g. exit-code-only failures from sub-processes).
            let chain = error_chain.as_deref().unwrap_or("");
            emit_non_zero_exit(w, track_id, command_label, exit_code_i32, chain);
        }
    }

    exit_code
}

/// Returns a static label string for the given `TrackCommand` variant.
///
/// Used as the `command` field in `TelemetryEvent::TrackSubcommand` /
/// `TelemetryEvent::NonZeroExit`.
fn track_command_label(cmd: &commands::track::TrackCommand) -> &'static str {
    use commands::track::{BranchAction, TrackCommand, ViewAction};

    match cmd {
        TrackCommand::Transition { .. } => "track transition",
        TrackCommand::Branch { action: BranchAction::Create(_) } => "track branch create",
        TrackCommand::Branch { action: BranchAction::Switch(_) } => "track branch switch",
        TrackCommand::Resolve(_) => "track resolve",
        TrackCommand::Views { action: ViewAction::Validate { .. } } => "track views validate",
        TrackCommand::Views { action: ViewAction::Sync { .. } } => "track views sync",
        TrackCommand::AddTask { .. } => "track add-task",
        TrackCommand::SetOverride { .. } => "track set-override",
        TrackCommand::ClearOverride { .. } => "track clear-override",
        TrackCommand::NextTask { .. } => "track next-task",
        TrackCommand::TaskCounts { .. } => "track task-counts",
        TrackCommand::Signals { .. } => "track signals",
        TrackCommand::TypeSignals { .. } => "track type-signals",
        TrackCommand::TypeGraph { .. } => "track type-graph",
        TrackCommand::BaselineGraph { .. } => "track baseline-graph",
        TrackCommand::ContractMap { .. } => "track contract-map",
        TrackCommand::CatalogueSpecSignals { .. } => "track catalogue-spec-signals",
        TrackCommand::SpecElementHash { .. } => "track spec-element-hash",
        TrackCommand::BaselineCapture { .. } => "track baseline-capture",
        TrackCommand::CatalogueImplSignals { .. } => "track catalogue-impl-signals",
        TrackCommand::Lint { .. } => "track lint",
        TrackCommand::SetCommitHash(_) => "track set-commit-hash",
    }
}

/// Returns `true` for pure display commands that must not emit telemetry
/// (OS-04 / AC-06).
///
/// Excluded set rationale:
/// - `Resolve` / `NextTask` / `TaskCounts`: read-only queries with no side
///   effects; emitting on these would pollute the log with noise.
/// - `Views { Validate }`: read-only metadata validation.
/// - `SpecElementHash`: read-only hash output helper.
/// - `CatalogueImplSignals` / `TypeGraph` / `ContractMap`: diagnostic/display
///   commands; run on demand for viewing purposes, not part of the workflow
///   execution path that telemetry is meant to track.
fn is_display_only_track_command(cmd: &commands::track::TrackCommand) -> bool {
    use commands::track::{TrackCommand, ViewAction};

    matches!(
        cmd,
        TrackCommand::Resolve(_)
            | TrackCommand::NextTask { .. }
            | TrackCommand::TaskCounts { .. }
            | TrackCommand::Views { action: ViewAction::Validate { .. } }
            | TrackCommand::SpecElementHash { .. }
            | TrackCommand::CatalogueImplSignals { .. }
            | TrackCommand::TypeGraph { .. }
            | TrackCommand::ContractMap { .. }
    )
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::process::ExitCode;

    use clap::Parser as _;
    use cli_composition::CliApp;
    use tempfile::TempDir;

    use super::run_cli_with;
    use super::{Cli, CliCommand, run_cli};
    use crate::commands::dry::DryCommand;
    use crate::commands::ref_verify::RefVerifyCommand;

    const MINIMAL_RULES: &str = r#"{
  "layers": [
    { "crate": "domain",  "path": "libs/domain",  "may_depend_on": [] },
    { "crate": "usecase", "path": "libs/usecase", "may_depend_on": ["domain"] }
  ]
}"#;

    /// End-to-end dispatch: `sotp arch tree --project-root <dir>` parses via `Cli::try_parse_from`
    /// and is dispatched through `run_cli` to `commands::arch::execute`, returning success.
    #[test]
    fn test_arch_tree_dispatch_via_run_cli_succeeds_with_valid_rules() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("architecture-rules.json"), MINIMAL_RULES).unwrap();
        let project_root = dir.path().to_str().unwrap();
        let cli =
            Cli::try_parse_from(["sotp", "arch", "tree", "--project-root", project_root]).unwrap();
        let exit = run_cli(cli, |_cmd| ExitCode::FAILURE);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn example_cli_flow_saves_track_successfully() {
        // Delegates to infrastructure::demo::run_example_demo which creates an in-memory
        // track, persists it, derives status "planned", and returns the display string.
        let result = CliApp::new().demo();
        assert!(result.is_ok(), "demo failed: {:?}", result.err());
        let outcome = result.unwrap();
        let msg = outcome.stdout.unwrap_or_default();
        assert!(msg.contains("planned"), "expected 'planned' in output: {msg}");
    }

    // ── CliCommand::Dry entrypoint dispatch routing ───────────────────────────

    /// `sotp dry write --track-id x` must resolve to `CliCommand::Dry { cmd: DryCommand::Write }`.
    /// This also runs the same dispatch helper used by `main()` and checks that
    /// the DRY executor's `ExitCode` is returned to the process entrypoint.
    #[test]
    fn test_dry_dispatch_write_routes_to_dry_write_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "my-track"]).unwrap();
        let exit = run_cli(cli, |cmd| {
            match cmd {
                DryCommand::Write(args) => {
                    assert_eq!(args.track_id, "my-track");
                }
                other => panic!("expected Write, got {other:?}"),
            }
            ExitCode::from(37)
        });
        assert_eq!(exit, ExitCode::from(37));
    }

    /// `sotp dry write --track-id x` must parse into `CliCommand::Dry { cmd: DryCommand::Write }`.
    #[test]
    fn test_dry_dispatch_write_parses_to_dry_write_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "my-track"]).unwrap();
        match cli.command {
            Some(CliCommand::Dry { cmd: DryCommand::Write(args) }) => {
                assert_eq!(args.track_id, "my-track");
            }
            _ => panic!("expected Dry {{ Write }}, got a different variant"),
        }
    }

    /// `sotp dry results --track-id x` must resolve to `CliCommand::Dry { cmd: DryCommand::Results }`.
    #[test]
    fn test_dry_dispatch_results_routes_to_dry_results_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "dry", "results", "--track-id", "my-track"]).unwrap();
        let exit = run_cli(cli, |cmd| {
            match cmd {
                DryCommand::Results(args) => {
                    assert_eq!(args.track_id, "my-track");
                }
                other => panic!("expected Results, got {other:?}"),
            }
            ExitCode::SUCCESS
        });
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    /// `sotp dry results --track-id x` must parse into `CliCommand::Dry { cmd: DryCommand::Results }`.
    #[test]
    fn test_dry_dispatch_results_parses_to_dry_results_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "dry", "results", "--track-id", "my-track"]).unwrap();
        match cli.command {
            Some(CliCommand::Dry { cmd: DryCommand::Results(args) }) => {
                assert_eq!(args.track_id, "my-track");
            }
            _ => panic!("expected Dry {{ Results }}, got a different variant"),
        }
    }

    /// `sotp dry check-approved --track-id x` must resolve to
    /// `CliCommand::Dry { cmd: DryCommand::CheckApproved }`.
    #[test]
    fn test_dry_dispatch_check_approved_routes_to_dry_check_approved_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "check-approved", "--track-id", "my-track"])
            .unwrap();
        let exit = run_cli(cli, |cmd| {
            match cmd {
                DryCommand::CheckApproved(args) => {
                    assert_eq!(args.track_id.as_deref(), Some("my-track"));
                }
                other => panic!("expected CheckApproved, got {other:?}"),
            }
            ExitCode::FAILURE
        });
        assert_eq!(exit, ExitCode::FAILURE);
    }

    /// `sotp dry check-approved --track-id x` must parse into
    /// `CliCommand::Dry { cmd: DryCommand::CheckApproved }`.
    #[test]
    fn test_dry_dispatch_check_approved_parses_to_dry_check_approved_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "check-approved", "--track-id", "my-track"])
            .unwrap();
        match cli.command {
            Some(CliCommand::Dry { cmd: DryCommand::CheckApproved(args) }) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
            }
            _ => panic!("expected Dry {{ CheckApproved }}, got a different variant"),
        }
    }

    /// An unrecognized `sotp dry` subcommand must be rejected by clap (Err),
    /// not silently fall through or panic.
    #[test]
    fn test_dry_dispatch_unknown_subcommand_is_rejected() {
        let result = Cli::try_parse_from(["sotp", "dry", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized dry subcommand must be rejected by clap");
    }

    // ── CliCommand::RefVerify entrypoint dispatch routing ────────────────────

    /// `sotp ref-verify run --track-id x` must resolve to
    /// `CliCommand::RefVerify { cmd: RefVerifyCommand::Run }`.
    #[test]
    fn test_ref_verify_dispatch_run_routes_to_ref_verify_run_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "ref-verify", "run", "--track-id", "my-track"]).unwrap();
        let exit = run_cli_with(
            cli,
            |_cmd| ExitCode::FAILURE,
            |cmd| {
                match cmd {
                    RefVerifyCommand::Run(args) => {
                        assert_eq!(args.track_id.as_deref(), Some("my-track"));
                    }
                    other => panic!("expected Run, got {other:?}"),
                }
                ExitCode::from(41)
            },
        );
        assert_eq!(exit, ExitCode::from(41));
    }

    /// `sotp ref-verify run --track-id x` must parse into
    /// `CliCommand::RefVerify { cmd: RefVerifyCommand::Run }`.
    #[test]
    fn test_ref_verify_dispatch_run_parses_to_ref_verify_run_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "ref-verify", "run", "--track-id", "my-track"]).unwrap();
        match cli.command {
            Some(CliCommand::RefVerify { cmd: RefVerifyCommand::Run(args) }) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
            }
            _ => panic!("expected RefVerify {{ Run }}, got a different variant"),
        }
    }

    /// `sotp ref-verify check-approved --track-id x` must resolve to
    /// `CliCommand::RefVerify { cmd: RefVerifyCommand::CheckApproved }`.
    #[test]
    fn test_ref_verify_dispatch_check_approved_routes_to_ref_verify_check_approved_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "ref-verify", "check-approved", "--track-id", "my-track"])
                .unwrap();
        let exit = run_cli_with(
            cli,
            |_cmd| ExitCode::FAILURE,
            |cmd| {
                match cmd {
                    RefVerifyCommand::CheckApproved(args) => {
                        assert_eq!(args.track_id.as_deref(), Some("my-track"));
                    }
                    other => panic!("expected CheckApproved, got {other:?}"),
                }
                ExitCode::from(43)
            },
        );
        assert_eq!(exit, ExitCode::from(43));
    }

    /// `sotp ref-verify check-approved --track-id x` must parse into
    /// `CliCommand::RefVerify { cmd: RefVerifyCommand::CheckApproved }`.
    #[test]
    fn test_ref_verify_dispatch_check_approved_parses_to_ref_verify_check_approved_variant() {
        let cli =
            Cli::try_parse_from(["sotp", "ref-verify", "check-approved", "--track-id", "my-track"])
                .unwrap();
        match cli.command {
            Some(CliCommand::RefVerify { cmd: RefVerifyCommand::CheckApproved(args) }) => {
                assert_eq!(args.track_id.as_deref(), Some("my-track"));
            }
            _ => panic!("expected RefVerify {{ CheckApproved }}, got a different variant"),
        }
    }

    /// An unrecognized `sotp ref-verify` subcommand must be rejected by clap (Err),
    /// not silently fall through or panic.
    #[test]
    fn test_ref_verify_dispatch_unknown_subcommand_is_rejected() {
        let result = Cli::try_parse_from(["sotp", "ref-verify", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized ref-verify subcommand must be rejected by clap");
    }

    // ── CliCommand::Conventions entrypoint dispatch routing ──────────────────

    const CONV_INDEX_START: &str = "<!-- convention-docs:start -->";
    const CONV_INDEX_END: &str = "<!-- convention-docs:end -->";

    /// Set up a minimal conventions directory with a README index and one doc
    /// so that `verify-index` returns success.
    ///
    /// The README block must exactly match what `render_index_block` produces:
    /// `- \`<filename>\`: <first-heading>` for each non-README `.md` file.
    fn setup_conventions_dir_with_doc(root: &std::path::Path) {
        let conv_dir = root.join("knowledge").join("conventions");
        fs::create_dir_all(&conv_dir).unwrap();
        // Write a placeholder convention doc with a heading line.
        fs::write(conv_dir.join("sample.md"), "# Sample\n").unwrap();
        // Write the README with the exact block format render_index_block produces:
        // `- \`<file>\`: <heading>` (backtick filename, colon, heading text).
        let readme = format!(
            "# Conventions\n\n{CONV_INDEX_START}\n- `sample.md`: Sample\n{CONV_INDEX_END}\n"
        );
        fs::write(conv_dir.join("README.md"), readme).unwrap();
    }

    /// End-to-end dispatch: `sotp conventions verify-index --project-root <dir>` parses via
    /// `Cli::try_parse_from` and is dispatched through `run_cli` to
    /// `commands::conventions::execute`, returning success when the index is in sync.
    #[test]
    fn test_conventions_verify_index_dispatch_via_run_cli_succeeds_with_synced_index() {
        let dir = TempDir::new().unwrap();
        setup_conventions_dir_with_doc(dir.path());
        let project_root = dir.path().to_str().unwrap();
        let cli = Cli::try_parse_from([
            "sotp",
            "conventions",
            "verify-index",
            "--project-root",
            project_root,
        ])
        .unwrap();
        let exit = run_cli(cli, |_cmd| ExitCode::FAILURE);
        assert_eq!(exit, ExitCode::SUCCESS);
    }
}
