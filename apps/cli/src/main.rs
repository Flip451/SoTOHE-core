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
    /// Telemetry tools: aggregate and display workflow telemetry for a track.
    Telemetry {
        #[command(subcommand)]
        cmd: commands::telemetry::TelemetryCommand,
    },
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
        Some(CliCommand::Hook { cmd }) => execute_hook_with_telemetry(cmd),
        Some(CliCommand::Track { cmd }) => execute_track_with_telemetry(cmd),
        Some(CliCommand::Git { cmd }) => commands::git::execute(cmd),
        Some(CliCommand::Pr { cmd }) => commands::pr::execute(cmd),
        Some(CliCommand::Plan { cmd }) => commands::plan::execute(cmd),
        Some(CliCommand::Review { cmd }) => commands::review::execute(cmd),
        Some(CliCommand::File { cmd }) => commands::file::execute(cmd),
        Some(CliCommand::Verify { cmd }) => execute_verify_with_telemetry(cmd),
        Some(CliCommand::FindSimilar(args)) => commands::semantic_dup::execute_find_similar(args),
        Some(CliCommand::DupIndex { cmd }) => commands::semantic_dup::execute_dup_index(cmd),
        Some(CliCommand::DupCheck(args)) => commands::semantic_dup::execute_dup_check(args),
        Some(CliCommand::Telemetry { cmd }) => commands::telemetry::execute(cmd),
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
    let is_archive = matches!(&cmd, commands::track::TrackCommand::Archive { .. });

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

    // Emit telemetry on completion - NOT at start (exit_code/duration are only
    // known after dispatch completes; IN-03 / T004 description).
    if let Some((ref w, ref track_id)) = telemetry {
        if is_archive && exit_code == ExitCode::SUCCESS && !telemetry_dir_override_is_set() {
            let _ = emit_archived_track_subcommand(
                &items_dir,
                track_id,
                command_label,
                exit_code_i32,
                start,
            );
        } else {
            emit_track_subcommand(w, track_id, command_label, exit_code_i32, start);
        }

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

fn telemetry_dir_override_is_set() -> bool {
    std::env::var("SOTP_TELEMETRY_DIR").ok().is_some_and(|value| !value.is_empty())
}

fn emit_archived_track_subcommand(
    items_dir: &std::path::Path,
    track_id: &str,
    command_label: &str,
    exit_code: i32,
    start: std::time::Instant,
) -> Result<(), String> {
    use std::io::Write as _;

    let repo_root = repo_root_for_items_dir(items_dir)?;
    let path =
        repo_root.join("track").join("archive").join(track_id).join("logs").join("telemetry.jsonl");
    let duration_ms = u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX);
    let event = serde_json::json!({
        "event_type": "TrackSubcommand",
        "schema_version": 1,
        "track_id": track_id,
        "command": command_label,
        "exit_code": exit_code,
        "duration_ms": duration_ms,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });
    let mut bytes = serde_json::to_vec(&event).map_err(|e| e.to_string())?;
    bytes.push(b'\n');

    let parent = path
        .parent()
        .ok_or_else(|| format!("archive telemetry path has no parent: {}", path.display()))?;
    std::fs::create_dir_all(parent)
        .map_err(|e| format!("failed to create telemetry directory {}: {e}", parent.display()))?;
    let mut file = std::fs::OpenOptions::new()
        .append(true)
        .create(true)
        .open(&path)
        .map_err(|e| format!("failed to open telemetry file {}: {e}", path.display()))?;
    let written = file
        .write(&bytes)
        .map_err(|e| format!("failed to write telemetry file {}: {e}", path.display()))?;
    if written != bytes.len() {
        return Err(format!(
            "short write for telemetry file {}: wrote {written} of {} bytes",
            path.display(),
            bytes.len()
        ));
    }

    Ok(())
}

fn repo_root_for_items_dir(items_dir: &std::path::Path) -> Result<std::path::PathBuf, String> {
    let project_root = commands::track::resolve_project_root(items_dir)?;
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .current_dir(&project_root)
        .output()
        .map_err(|e| format!("failed to run git rev-parse --show-toplevel: {e}"))?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let code = output.status.code().unwrap_or(-1);
        return Err(format!("git rev-parse --show-toplevel failed (exit {code}): {stderr}"));
    }
    let root = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if root.is_empty() {
        return Err("git rev-parse --show-toplevel returned an empty path".to_owned());
    }

    Ok(std::path::PathBuf::from(root))
}

// ---------------------------------------------------------------------------
// Hook dispatch with telemetry (T005)
// ---------------------------------------------------------------------------

/// Dispatch a `HookCommand` with telemetry instrumentation.
///
/// Hooks are instrumented per AC-04 / OS-03:
/// - PreToolUse block (exit code 2) → emit `TelemetryEvent::HookBlock`.
/// - Advisory `skill-compliance` that produces a non-empty stdout injection →
///   emit `TelemetryEvent::AdvisoryHookFired`.
/// - All allow / pass-through paths emit NOTHING and have no file IO (OS-03 /
///   AC-06).
///
/// The telemetry writer is resolved using the default items_dir (`"track/items"`
/// relative to CWD), consistent with the hook execution context.
fn execute_hook_with_telemetry(cmd: commands::hook::HookCommand) -> ExitCode {
    use cli_composition::telemetry_wiring::{
        emit_advisory_hook_fired, emit_hook_block, resolve_telemetry_writer,
    };

    // Capture the hook name before consuming the command.
    let hook_name = hook_command_hook_name(&cmd).to_owned();

    // Classify: is this an advisory (UserPromptSubmit) hook?
    let is_advisory = is_advisory_hook_command(&cmd);

    // Execute the hook via the existing dispatch path.
    let outcome_result = commands::hook::execute_inner(cmd);

    // Determine outcome and emit telemetry.
    //
    // `resolve_telemetry_writer` (branch discovery + file IO) is called only on
    // the two paths that actually emit events (block / advisory-fired) so that
    // allow-path and pass-through invocations incur zero I/O (OS-03 / AC-06).
    match &outcome_result {
        Ok(outcome) => {
            // Block verdict: exit code 2 for PreToolUse hooks.
            let is_block = !is_advisory && outcome.exit_code == 2;

            if is_block {
                let items_dir = std::path::PathBuf::from("track/items");
                if let Some((ref w, ref track_id)) = resolve_telemetry_writer(&items_dir) {
                    emit_hook_block(w, track_id, &hook_name);
                }
            } else if is_advisory && outcome.stdout.is_some() {
                // Advisory hook fired (non-empty context injection) — OS-03:
                // only emit when advisory actually produced output.
                let items_dir = std::path::PathBuf::from("track/items");
                if let Some((ref w, ref track_id)) = resolve_telemetry_writer(&items_dir) {
                    emit_advisory_hook_fired(w, track_id, &hook_name);
                }
            }
            // All other paths (allow, advisory with no injection): no emit (OS-03).

            // Print outcome and return exit code (same as commands::hook::execute).
            if let Some(ref stdout) = outcome.stdout {
                println!("{stdout}");
            }
            if let Some(ref stderr) = outcome.stderr {
                eprintln!("{stderr}");
            }
            ExitCode::from(outcome.exit_code)
        }
        Err(msg) => {
            eprintln!("{msg}");
            // Fail-closed for hooks: internal error → block (exit 2).
            // Emit HookBlock so internal failures are visible in telemetry
            // (same as a deliberate block verdict from the dispatch logic).
            let items_dir = std::path::PathBuf::from("track/items");
            if let Some((ref w, ref track_id)) = resolve_telemetry_writer(&items_dir) {
                emit_hook_block(w, track_id, &hook_name);
            }
            ExitCode::from(2u8)
        }
    }
}

/// Returns the hook name string for the given `HookCommand` variant.
fn hook_command_hook_name(cmd: &commands::hook::HookCommand) -> &'static str {
    match cmd {
        commands::hook::HookCommand::Dispatch { hook, .. } => hook.hook_name(),
    }
}

/// Returns `true` when the hook is an advisory (UserPromptSubmit / injection)
/// hook rather than a PreToolUse guard.
fn is_advisory_hook_command(cmd: &commands::hook::HookCommand) -> bool {
    use commands::hook::{CliHookName, HookCommand};
    matches!(cmd, HookCommand::Dispatch { hook: CliHookName::SkillCompliance, .. })
}

// ---------------------------------------------------------------------------
// Verify dispatch with telemetry (T005)
// ---------------------------------------------------------------------------

/// Dispatch a `VerifyCommand` with telemetry instrumentation.
///
/// Emits `TelemetryEvent::GateEval` after every gate evaluation with:
/// - `gate_name`: the verify subcommand name label.
/// - `verdict`: `"ok"` (exit 0) or `"error"` (exit ≠ 0).
/// - `reason_summary`: leading output text (stdout when present, stderr otherwise; first 256 bytes).
/// - `duration_ms`: wall-clock time of the gate evaluation (GO-01).
///
/// Telemetry is only emitted when on a `track/*` branch (AC-11 / IN-04).
fn execute_verify_with_telemetry(cmd: commands::verify::VerifyCommand) -> ExitCode {
    use cli_composition::telemetry_wiring::{emit_gate_eval, resolve_telemetry_writer};
    use std::time::Instant;

    let gate_name = verify_command_gate_name(&cmd);

    // Resolve telemetry writer using the command's own items_dir so non-default
    // --project-root / --workspace-root / --items-dir invocations anchor telemetry
    // to the correct repository (P1 fix: was hardcoded "track/items" relative to CWD).
    let items_dir = cmd.items_dir();
    let telemetry = resolve_telemetry_writer(&items_dir);

    let start = Instant::now();
    // execute_with_summary prints output and returns (exit_code, Option<stdout_text>)
    // so that reason_summary carries the actual gate findings (P1 fix: was static label).
    let (exit_code, stdout_text) = commands::verify::execute_with_summary(cmd);

    if let Some((ref w, ref track_id)) = telemetry {
        let verdict = if exit_code == ExitCode::SUCCESS { "ok" } else { "error" };
        // reason_summary: leading text from the gate output (first 256 bytes,
        // rounded down to a valid UTF-8 boundary). Falls back to the gate name
        // when the output text is absent.
        // reason_summary: full gate output text trimmed of surrounding whitespace.
        // The TelemetryWriter enforces the 4096-byte JSONL line cap and truncates
        // variable-length fields (including reason_summary) only when the serialized
        // line would exceed that budget (CN-05).  Pre-truncating here would drop
        // human-readable diagnostics before the writer has a chance to fit them in.
        let reason_summary = stdout_text
            .as_deref()
            .map(|s| s.trim().to_owned())
            .unwrap_or_else(|| format!("gate: {gate_name}"));
        emit_gate_eval(w, track_id, gate_name, verdict, &reason_summary, start);
    }

    exit_code
}

/// Returns a static label for the given `VerifyCommand` variant used as `gate_name`.
fn verify_command_gate_name(cmd: &commands::verify::VerifyCommand) -> &'static str {
    use commands::verify::VerifyCommand;
    match cmd {
        VerifyCommand::TechStack(_) => "verify-tech-stack",
        VerifyCommand::LatestTrack(_) => "verify-latest-track",
        VerifyCommand::ArchDocs(_) => "verify-arch-docs",
        VerifyCommand::Layers(_) => "verify-layers",
        VerifyCommand::Orchestra(_) => "verify-orchestra",
        VerifyCommand::HooksPath(_) => "verify-hooks-path",
        VerifyCommand::SpecAttribution(_) => "verify-spec-attribution",
        VerifyCommand::SpecFrontmatter(_) => "verify-spec-frontmatter",
        VerifyCommand::CanonicalModules(_) => "verify-canonical-modules",
        VerifyCommand::ModuleSize(_) => "verify-module-size",
        VerifyCommand::DomainPurity(_) => "verify-domain-purity",
        VerifyCommand::DomainStrings(_) => "verify-domain-strings",
        VerifyCommand::UsecasePurity(_) => "verify-usecase-purity",
        VerifyCommand::DocLinks(_) => "verify-doc-links",
        VerifyCommand::ViewFreshness(_) => "verify-view-freshness",
        VerifyCommand::SpecSignals(_) => "verify-spec-signals",
        VerifyCommand::SpecStates(_) => "verify-spec-states",
        VerifyCommand::PlanArtifactRefs(_) => "verify-plan-artifact-refs",
        VerifyCommand::CatalogueSpecRefs(_) => "verify-catalogue-spec-refs",
        VerifyCommand::CatalogueSpecSignals(_) => "verify-catalogue-spec-signals",
        VerifyCommand::AdrSignals(_) => "verify-adr-signals",
    }
}

/// Returns a static label string for the given `TrackCommand` variant.
///
/// Used as the `command` field in `TelemetryEvent::TrackSubcommand` /
/// `TelemetryEvent::NonZeroExit`.
fn track_command_label(cmd: &commands::track::TrackCommand) -> &'static str {
    use commands::track::{BranchAction, TrackCommand, ViewAction};

    match cmd {
        TrackCommand::Archive { .. } => "track archive",
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
    use crate::commands::track::test_support::{process_env_lock, run_in_dir, seed_repo};

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

    // ── CliCommand::Telemetry entrypoint dispatch routing ───────────────────

    /// `sotp telemetry report <track-id>` must be registered at the public CLI
    /// entrypoint and dispatch through `run_cli` to the report command.
    #[test]
    fn test_telemetry_report_dispatch_via_run_cli_succeeds_with_existing_track() {
        let dir = TempDir::new().unwrap();
        let track_id = "telemetry-route-track";
        fs::create_dir_all(dir.path().join(track_id)).unwrap();
        let items_dir = dir.path().to_str().unwrap();

        let cli = Cli::try_parse_from([
            "sotp",
            "telemetry",
            "report",
            track_id,
            "--items-dir",
            items_dir,
        ])
        .unwrap();

        let exit = run_cli(cli, |_cmd| ExitCode::FAILURE);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_archive_telemetry_emit_writes_to_archived_track_logs() {
        let _guard = process_env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let track_id = "archive-telemetry-track";
        seed_repo(root, &format!("track/{track_id}"));
        let items_dir = std::path::PathBuf::from("track/items");
        let archived_track_dir = root.join("track").join("archive").join(track_id);
        fs::create_dir_all(&archived_track_dir).unwrap();
        let nested_cwd = root.join("nested").join("workdir");
        fs::create_dir_all(&nested_cwd).unwrap();

        run_in_dir(&nested_cwd, || {
            super::emit_archived_track_subcommand(
                &items_dir,
                track_id,
                "track archive",
                0,
                std::time::Instant::now(),
            )
            .unwrap();
        });

        let active_log =
            root.join("track").join("items").join(track_id).join("logs").join("telemetry.jsonl");
        let nested_archive_log = nested_cwd
            .join("track")
            .join("archive")
            .join(track_id)
            .join("logs")
            .join("telemetry.jsonl");
        let archived_log = archived_track_dir.join("logs").join("telemetry.jsonl");
        assert!(
            !active_log.exists(),
            "archive telemetry must not recreate the active track log: {active_log:?}"
        );
        assert!(
            !nested_archive_log.exists(),
            "relative items_dir must be anchored at the repo root, not cwd: {nested_archive_log:?}"
        );

        let line = fs::read_to_string(&archived_log).unwrap();
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["event_type"], "TrackSubcommand");
        assert_eq!(value["track_id"], track_id);
        assert_eq!(value["command"], "track archive");
        assert_eq!(value["exit_code"], 0);
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

    // ── Hook telemetry wrapper paths ─────────────────────────────────────────

    /// `sotp hook dispatch skill-compliance` routes through `execute_hook_with_telemetry`.
    /// With empty stdin the advisory hook sees no prompt → no injection → exits 0.
    /// Telemetry is silently skipped (not on a track branch in CI) — no panic.
    #[test]
    fn test_hook_dispatch_skill_compliance_via_run_cli_exits_zero() {
        let cli = Cli::try_parse_from(["sotp", "hook", "dispatch", "skill-compliance"]).unwrap();
        let exit = run_cli(cli, |_cmd| ExitCode::FAILURE);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    // ── Verify telemetry wrapper paths ───────────────────────────────────────

    /// `sotp verify tech-stack` routes through `execute_verify_with_telemetry`.
    /// With a CWD that has no tech-stack.md the command exits non-zero (findings),
    /// but the wrapper itself must not panic and must forward the exit code.
    #[test]
    fn test_verify_tech_stack_dispatch_via_run_cli_does_not_panic() {
        let dir = TempDir::new().unwrap();
        let project_root = dir.path().to_str().unwrap();
        let cli =
            Cli::try_parse_from(["sotp", "verify", "tech-stack", "--project-root", project_root])
                .unwrap();
        // Non-zero exit is expected (no tech-stack.md). The wrapper must not panic.
        let exit = run_cli(cli, |_cmd| ExitCode::FAILURE);
        assert_ne!(exit, ExitCode::from(2u8), "exit 2 reserved for hook blocks");
    }

    /// `sotp verify layers` routes through `execute_verify_with_telemetry`.
    /// With a temp dir (no Cargo.toml) cargo-metadata fails → non-zero exit, but
    /// the wrapper must not panic and must not return exit code 2 (reserved for
    /// hook blocks), confirming gate failures are not conflated with blocks.
    #[test]
    fn test_verify_layers_dispatch_via_run_cli_does_not_panic() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("architecture-rules.json"), MINIMAL_RULES).unwrap();
        let project_root = dir.path().to_str().unwrap();
        let cli = Cli::try_parse_from(["sotp", "verify", "layers", "--project-root", project_root])
            .unwrap();
        // Non-zero exit expected (no Cargo.toml for cargo-metadata). Must not panic.
        let exit = run_cli(cli, |_cmd| ExitCode::FAILURE);
        assert_ne!(exit, ExitCode::from(2u8), "exit 2 reserved for hook blocks");
    }

    // ── verify_command_gate_name coverage ────────────────────────────────────

    /// Each `VerifyCommand` variant parsed from CLI args must map to a stable
    /// gate name starting with "verify-".
    #[test]
    fn test_verify_command_gate_name_uses_verify_prefix() {
        use super::verify_command_gate_name;

        let subcommands = [
            ["sotp", "verify", "tech-stack"],
            ["sotp", "verify", "latest-track"],
            ["sotp", "verify", "arch-docs"],
            ["sotp", "verify", "layers"],
        ];
        for args in &subcommands {
            let cli = Cli::try_parse_from(*args).unwrap();
            if let Some(CliCommand::Verify { cmd }) = cli.command {
                let name = verify_command_gate_name(&cmd);
                assert!(
                    name.starts_with("verify-"),
                    "gate name '{name}' does not start with 'verify-'"
                );
            }
        }
    }
}
