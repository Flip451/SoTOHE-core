#![warn(clippy::too_many_lines)]

use std::process::ExitCode;

use clap::{CommandFactory, Parser, Subcommand};

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

// Subcommand payloads intentionally stay unboxed so clap can derive the command
// surface from the same DTO shapes documented in the catalogue contract.
#[allow(clippy::large_enum_variant)]
#[derive(Subcommand)]
enum CliCommand {
    /// Architecture rules analysis tools.
    Arch {
        #[command(subcommand)]
        cmd: commands::arch::ArchCommand,
    },
    /// ADR baseline snapshot, restore, and freeze-check operations.
    AdrBaseline {
        #[command(subcommand)]
        cmd: commands::adr_baseline::AdrBaselineCommand,
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
    /// Configure sccache and clean configured build artifacts.
    Maintenance {
        #[command(subcommand)]
        cmd: commands::maintenance::MaintenanceCommand,
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
    /// Generic profile-driven capability dispatch.
    Capability {
        #[command(subcommand)]
        cmd: commands::capability::CapabilityCommand,
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
    /// Test-obligation gate and binding authoring helpers.
    TestObligation {
        #[command(subcommand)]
        cmd: commands::test_obligation::TestObligationSubcommand,
    },
    /// Signal evaluation commands: calc-*/check-* for all 4 SoT Chain chains.
    Signal {
        #[command(subcommand)]
        cmd: commands::signal::SignalCommand,
    },
    /// Pre-review conformance gate: verify contracted catalogue entries have blue signals.
    TaskContract {
        #[command(subcommand)]
        cmd: commands::task_contract::TaskContractCommand,
    },
    /// Catalogue generation + annotation: init, add, import, cite, check.
    Catalog {
        #[command(subcommand)]
        cmd: commands::catalog::CatalogCommand,
    },
    /// Catalogue lint: primitive-obsession guard across TDDD layer catalogues.
    CatalogueLint {
        #[command(subcommand)]
        cmd: commands::catalogue_lint::CatalogueLintCommand,
    },
    /// Template export: build a generic template tree from the boundary manifest.
    Template {
        #[command(subcommand)]
        cmd: commands::template::TemplateCommand,
    },
    /// Resolve, verify, and provision the repository-local Codex runtime link.
    CodexRuntime {
        #[command(subcommand)]
        cmd: commands::codex_runtime::CodexRuntimeCommand,
    },
    /// Phase 3 batch-plan gate: check declared batches against ceilings and the plan.
    BatchPlan {
        #[command(subcommand)]
        cmd: commands::batch_plan::BatchPlanCommand,
    },
    /// Run the example track state machine demo.
    #[cfg(not(doc))]
    Demo,
}

macro_rules! run_cli_with_context {
    (
        $cli:expr,
        $dry_execute:expr,
        $ref_verify_execute:expr,
        $command_identity:expr,
        $telemetry_items_dir:expr $(,)?
    ) => {{
        use cli_composition::{DemoCompositionRoot, TelemetryCompositionRoot};
        use cli_driver::demo::DemoInput;
        use cli_driver::telemetry::{TelemetryInput, duration_millis, exit_code_value};
        use $crate::commands;
        use $crate::{CliCommand, execute_hook_with_telemetry, execute_verify_with_telemetry};

        let cli = $cli;
        let dry_execute = $dry_execute;
        let ref_verify_execute = $ref_verify_execute;
        let command_identity = $command_identity;
        let telemetry_items_dir = $telemetry_items_dir;
        let telemetry_driver = TelemetryCompositionRoot::new().telemetry_driver();
        // Capture the branch-bound track context before dispatch so a command
        // that changes branches cannot retarget its completion event.
        let source_track_id =
            cli_composition::telemetry_wiring::resolve_telemetry_writer(&telemetry_items_dir)
                .map(|(_, track_id)| track_id);
        let started = std::time::Instant::now();
        let (exit_code, error_chain) = match cli.command {
            Some(CliCommand::Arch { cmd }) => (commands::arch::execute(cmd), None),
            Some(CliCommand::AdrBaseline { cmd }) => (commands::adr_baseline::execute(cmd), None),
            Some(CliCommand::Conventions { cmd }) => (commands::conventions::execute(cmd), None),
            Some(CliCommand::Domain { cmd }) => (commands::domain::execute(cmd), None),
            Some(CliCommand::Guard { cmd }) => (commands::guard::execute(cmd), None),
            Some(CliCommand::Hook { cmd }) => (execute_hook_with_telemetry(cmd), None),
            Some(CliCommand::Maintenance { cmd }) => (commands::maintenance::execute(cmd), None),
            Some(CliCommand::Track { cmd }) => commands::track::execute_with_error_chain(cmd),
            Some(CliCommand::Git { cmd }) => (commands::git::execute(cmd), None),
            Some(CliCommand::Pr { cmd }) => (commands::pr::execute(cmd), None),
            Some(CliCommand::Capability { cmd }) => (commands::capability::execute(cmd), None),
            Some(CliCommand::Review { cmd }) => (commands::review::execute(cmd), None),
            Some(CliCommand::File { cmd }) => (commands::file::execute(cmd), None),
            Some(CliCommand::Verify { cmd }) => (execute_verify_with_telemetry(cmd), None),
            Some(CliCommand::FindSimilar(args)) => {
                (commands::semantic_dup::execute_find_similar(args), None)
            }
            Some(CliCommand::DupIndex { cmd }) => {
                (commands::semantic_dup::execute_dup_index(cmd), None)
            }
            Some(CliCommand::DupCheck(args)) => {
                (commands::semantic_dup::execute_dup_check(args), None)
            }
            Some(CliCommand::Telemetry { cmd }) => (commands::telemetry::execute(cmd), None),
            Some(CliCommand::Dry { cmd }) => (dry_execute(cmd), None),
            Some(CliCommand::RefVerify { cmd }) => (ref_verify_execute(cmd), None),
            Some(CliCommand::TestObligation { cmd }) => (
                commands::test_obligation::execute(
                    commands::test_obligation::TestObligationArgs::new(cmd),
                ),
                None,
            ),
            Some(CliCommand::Signal { cmd }) => (commands::signal::execute(cmd), None),
            Some(CliCommand::TaskContract { cmd }) => (commands::task_contract::execute(cmd), None),
            Some(CliCommand::BatchPlan { cmd }) => (commands::batch_plan::execute(cmd), None),
            Some(CliCommand::Catalog { cmd }) => (commands::catalog::execute(cmd), None),
            Some(CliCommand::CatalogueLint { cmd }) => {
                (commands::catalogue_lint::execute(cmd), None)
            }
            Some(CliCommand::Template { cmd }) => (commands::template::execute(cmd), None),
            Some(CliCommand::CodexRuntime { cmd }) => (commands::codex_runtime::execute(cmd), None),
            #[cfg(not(doc))]
            Some(CliCommand::Demo) | None => {
                let outcome = DemoCompositionRoot::new().demo_driver().handle(DemoInput::Run);
                if let Some(msg) = outcome.stdout {
                    println!("{msg}");
                }
                if let Some(msg) = outcome.stderr {
                    eprintln!("{msg}");
                }
                (ExitCode::from(outcome.exit_code), None)
            }
            #[cfg(doc)]
            None => {
                let outcome = DemoCompositionRoot::new().demo_driver().handle(DemoInput::Run);
                if let Some(msg) = outcome.stdout {
                    println!("{msg}");
                }
                if let Some(msg) = outcome.stderr {
                    eprintln!("{msg}");
                }
                (ExitCode::from(outcome.exit_code), None)
            }
        };
        let exit_code_i32 = exit_code_value(exit_code);
        let normalized = command_identity.strip_prefix("sotp ").unwrap_or(&command_identity);
        if telemetry_completion_eligible(normalized) {
            let duration_ms = duration_millis(started);
            let archived_track_id = (normalized == "track archive"
                && exit_code == ExitCode::SUCCESS)
                .then(|| source_track_id.as_ref().cloned())
                .flatten();
            if let Some(track_id) = archived_track_id {
                let _ = telemetry_driver.handle(TelemetryInput::EmitArchivedTrackSubcommand {
                    items_dir: telemetry_items_dir,
                    track_id,
                    subcommand: command_identity,
                    exit_code: exit_code_i32,
                    duration_ms,
                });
            } else {
                let _ = telemetry_driver.handle(TelemetryInput::EmitCompletedCommand {
                    items_dir: telemetry_items_dir,
                    source_track_id,
                    subcommand: command_identity,
                    exit_code: exit_code_i32,
                    duration_ms,
                    error_chain,
                });
            }
        }
        exit_code
    }};
}

fn main() -> ExitCode {
    // Initialise tracing subscriber once at the composition root entry point
    // (IN-01 / CN-04 / AC-01: subscriber init lives here, not in domain or usecase).
    cli_composition::telemetry_wiring::init_tracing_subscriber();

    run_cli(Cli::parse(), commands::dry::execute)
}

/// Thin entrypoint retained for the existing CLI boundary.
///
/// The common completion lifecycle remains owned by `TelemetryDriver`; this
/// wrapper only supplies the parsed command and the raw-argv context needed by
/// the driver-facing macro.
fn run_cli(cli: Cli, dry_execute: fn(commands::dry::DryCommand) -> ExitCode) -> ExitCode {
    run_cli_with(cli, dry_execute, commands::ref_verify::execute)
}

/// Common CLI entrypoint that captures command context before dispatch, then
/// submits the resulting completion as a data-only telemetry input.
fn run_cli_with(
    cli: Cli,
    dry_execute: fn(commands::dry::DryCommand) -> ExitCode,
    ref_verify_execute: fn(commands::ref_verify::RefVerifyCommand) -> ExitCode,
) -> ExitCode {
    let argv = std::env::args_os().collect::<Vec<_>>();
    let command_identity = command_identity_from_args(&argv);
    let items_dir = cli_driver::telemetry::items_dir_from_args(&argv);
    run_cli_with_context!(cli, dry_execute, ref_verify_execute, command_identity, items_dir,)
}

/// Derive only the clap subcommand path for telemetry. `ArgMatches` exposes
/// the command tree while ignoring positional payloads and option values, so
/// prompts, track ids, and other user data never become part of the event
/// identity.
fn command_identity_from_args(args: &[std::ffi::OsString]) -> String {
    let Ok(matches) = Cli::command().try_get_matches_from(args.to_owned()) else {
        return "sotp demo".to_owned();
    };
    let mut names = Vec::new();
    let mut current = &matches;
    while let Some((name, subcommand)) = current.subcommand() {
        names.push(name);
        current = subcommand;
    }
    if names.is_empty() { "sotp demo".to_owned() } else { format!("sotp {}", names.join(" ")) }
}

/// Composition policy for the common command-completion telemetry boundary.
///
/// The CLI owns command-family routing because it is the only layer that sees
/// the parsed clap command tree. The driver receives only the resulting data
/// payload through `TelemetryInput::EmitCompletedCommand`.
fn telemetry_completion_eligible(subcommand: &str) -> bool {
    let top_level = subcommand.split_whitespace().next().unwrap_or_default();
    if matches!(top_level, "arch" | "track" | "hook" | "verify" | "find-similar" | "telemetry") {
        return top_level == "track" && !telemetry_track_display_only(subcommand);
    }
    !matches!(
        subcommand,
        "conventions resolve"
            | "review results"
            | "review classify"
            | "review files"
            | "dry results"
            | "ref-verify results"
            | "signal report"
            | "test-obligation results"
            | "test-obligation bindings-skeleton"
            | "dup-index measure-quality"
    )
}

fn telemetry_track_display_only(subcommand: &str) -> bool {
    matches!(
        subcommand,
        "track resolve"
            | "track next-task"
            | "track task-counts"
            | "track views validate"
            | "track spec-element-hash"
            | "track fixpoint-resolve"
            | "track catalogue-impl-signals"
            | "track type-graph"
            | "track contract-map"
    )
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
        Err(e) => {
            eprintln!("{e}");
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
        VerifyCommand::LatestTrack(_) => "verify-latest-track",
        VerifyCommand::RetentionGate(_) => "verify-retention-gate",
        VerifyCommand::SotpVersionTag(_) => "verify-sotp-version-tag",
        VerifyCommand::MachinePaths(_) => "verify-machine-paths",
        VerifyCommand::TemplateRefs(_) => "verify-template-refs",
        VerifyCommand::ArchDocs(_) => "verify-arch-docs",
        VerifyCommand::Layers(_) => "verify-layers",
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
        VerifyCommand::PlanArtifactRefs(_) => "verify-plan-artifact-refs",
        VerifyCommand::CatalogueSpecRefs(_) => "verify-catalogue-spec-refs",
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::fs;
    use std::process::ExitCode;

    use clap::Parser as _;
    use cli_composition::DemoCompositionRoot;
    use cli_driver::demo::DemoInput as DriverDemoInput;
    use tempfile::TempDir;

    use super::{Cli, CliCommand, command_identity_from_args, telemetry_completion_eligible};
    use crate::commands::dry::DryCommand;
    use crate::commands::ref_verify::RefVerifyCommand;
    use crate::commands::track::test_support::{process_env_lock, run_git, run_in_dir, seed_repo};

    const MINIMAL_RULES: &str = r#"{
  "layers": [
    { "crate": "domain",  "path": "libs/domain",  "may_depend_on": [] },
    { "crate": "usecase", "path": "libs/usecase", "may_depend_on": ["domain"] }
  ]
}"#;

    macro_rules! dispatch_cli_test {
        ($cli:expr, $dry_execute:expr $(,)?) => {
            run_cli_with_context!(
                $cli,
                $dry_execute,
                crate::commands::ref_verify::execute,
                "sotp telemetry".to_owned(),
                std::path::PathBuf::from("track/items"),
            )
        };
    }

    macro_rules! dispatch_cli_with_test {
        ($cli:expr, $dry_execute:expr, $ref_verify_execute:expr $(,)?) => {
            run_cli_with_context!(
                $cli,
                $dry_execute,
                $ref_verify_execute,
                "sotp telemetry".to_owned(),
                std::path::PathBuf::from("track/items"),
            )
        };
    }

    /// End-to-end dispatch: `sotp arch tree --project-root <dir>` parses via `Cli::try_parse_from`
    /// and is dispatched through the common CLI entrypoint to `commands::arch::execute`, returning success.
    #[test]
    fn test_arch_tree_dispatch_via_run_cli_succeeds_with_valid_rules() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("architecture-rules.json"), MINIMAL_RULES).unwrap();
        let project_root = dir.path().to_str().unwrap();
        let cli =
            Cli::try_parse_from(["sotp", "arch", "tree", "--project-root", project_root]).unwrap();
        let exit = dispatch_cli_test!(cli, |_cmd| ExitCode::FAILURE);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn example_cli_flow_saves_track_successfully() {
        // Delegates to infrastructure::demo::run_example_demo which creates an in-memory
        // track, persists it, derives status "planned", and returns the display string.
        let outcome = DemoCompositionRoot::new().demo_driver().handle(DriverDemoInput::Run);
        assert_eq!(outcome.exit_code, 0, "demo failed: {:?}", outcome.stderr);
        let msg = outcome.stdout.unwrap_or_default();
        assert!(msg.contains("planned"), "expected 'planned' in output: {msg}");
    }

    #[test]
    fn test_cli_command_capability_exec_parses_to_capability_variant() {
        let cli = Cli::try_parse_from([
            "sotp",
            "capability",
            "exec",
            "implementer",
            "--host",
            "codex",
            "--briefing-file",
            "tmp/briefing.md",
        ])
        .expect("capability exec must parse at the top-level command boundary");

        assert!(
            matches!(cli.command, Some(CliCommand::Capability { .. })),
            "capability exec must select the Capability variant"
        );
    }

    #[test]
    fn test_cli_command_batch_plan_check_parses_to_batch_plan_variant() {
        let cli = Cli::try_parse_from([
            "sotp",
            "batch-plan",
            "check",
            "--track-id",
            "some-track",
            "--items-dir",
            "track/items",
        ])
        .expect("batch-plan check must parse at the top-level command boundary");

        assert!(
            matches!(cli.command, Some(CliCommand::BatchPlan { .. })),
            "batch-plan check must select the BatchPlan variant"
        );
    }

    #[test]
    fn test_cli_command_codex_runtime_provision_parses_to_runtime_variant() {
        let cli = Cli::try_parse_from([
            "sotp",
            "codex-runtime",
            "provision",
            "--project-root",
            "/workspace/project",
        ])
        .expect("codex-runtime provision must parse at the top-level command boundary");

        assert!(matches!(cli.command, Some(CliCommand::CodexRuntime { .. })));
    }

    #[test]
    fn test_cli_command_retired_plan_codex_local_is_rejected() {
        assert!(
            Cli::try_parse_from(["sotp", "plan", "codex-local"]).is_err(),
            "the retired plan codex-local command must not parse"
        );
    }

    #[test]
    fn test_completed_command_success_appends_existing_telemetry_event() {
        let _guard = process_env_lock().lock().unwrap();
        let directory = TempDir::new().unwrap();
        seed_repo(directory.path(), "track/trace-success");
        let cli =
            Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "trace-success"]).unwrap();
        let path = directory.path().join("track/items/trace-success/logs/telemetry.jsonl");
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(
            &path,
            "{\"event_type\":\"HookBlock\",\"schema_version\":1,\"track_id\":\"trace-success\",\"hook_name\":\"existing\",\"timestamp\":\"2026-08-02T00:00:00Z\"}\n",
        )
        .unwrap();

        let exit = temp_env::with_vars(
            [("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None)],
            || {
                run_in_dir(directory.path(), || {
                    run_cli_with_context!(
                        cli,
                        |_command| ExitCode::SUCCESS,
                        |_command| ExitCode::FAILURE,
                        "sotp dry".to_owned(),
                        std::path::PathBuf::from("track/items"),
                    )
                })
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
        let lines = fs::read_to_string(&path).unwrap();
        assert_eq!(lines.lines().count(), 2, "completion must append to existing telemetry");
        assert!(lines.lines().next().unwrap().contains("\"hook_name\":\"existing\""));
        let record: serde_json::Value =
            serde_json::from_str(lines.lines().last().unwrap()).unwrap();
        assert_eq!(record["event_type"], "TrackSubcommand");
        assert_eq!(record["command"], "sotp dry");
        assert_eq!(record["exit_code"], 0);
        assert!(record["duration_ms"].is_u64());
    }

    #[test]
    fn test_completed_command_failure_preserves_exit_and_appends_nonzero_event() {
        let _guard = process_env_lock().lock().unwrap();
        let directory = TempDir::new().unwrap();
        seed_repo(directory.path(), "track/trace-failure");
        let cli =
            Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "trace-failure"]).unwrap();

        let exit = temp_env::with_vars(
            [("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None)],
            || {
                run_in_dir(directory.path(), || {
                    run_cli_with_context!(
                        cli,
                        |_command| ExitCode::from(u8::MAX),
                        |_command| ExitCode::SUCCESS,
                        "sotp dry".to_owned(),
                        std::path::PathBuf::from("track/items"),
                    )
                })
            },
        );

        assert_eq!(exit, ExitCode::from(u8::MAX));
        let path = directory.path().join("track/items/trace-failure/logs/telemetry.jsonl");
        let lines = fs::read_to_string(path).unwrap();
        assert!(lines.lines().any(|line| line.contains("\"event_type\":\"TrackSubcommand\"")));
        assert!(lines.lines().any(|line| line.contains("\"event_type\":\"NonZeroExit\"")));
    }

    #[test]
    fn test_completed_command_append_failure_preserves_original_exit() {
        let _guard = process_env_lock().lock().unwrap();
        let directory = TempDir::new().unwrap();
        let track_id = "trace-append-failure";
        seed_repo(directory.path(), &format!("track/{track_id}"));
        let logs_path = directory.path().join("track/items").join(track_id).join("logs");
        fs::create_dir_all(logs_path.parent().unwrap()).unwrap();
        fs::write(&logs_path, "not a directory").unwrap();
        let cli = Cli::try_parse_from(["sotp", "dry", "write", "--track-id", track_id]).unwrap();

        let exit = temp_env::with_vars(
            [("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None)],
            || {
                run_in_dir(directory.path(), || {
                    run_cli_with_context!(
                        cli,
                        |_command| ExitCode::from(42),
                        |_command| ExitCode::SUCCESS,
                        "sotp dry".to_owned(),
                        std::path::PathBuf::from("track/items"),
                    )
                })
            },
        );

        assert_eq!(exit, ExitCode::from(42));
        assert!(logs_path.is_file(), "append failure must not replace the original path");
    }

    #[test]
    fn test_completed_command_non_track_branch_leaves_no_telemetry_log() {
        let _guard = process_env_lock().lock().unwrap();
        let directory = TempDir::new().unwrap();
        seed_repo(directory.path(), "main");
        let cli =
            Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "not-a-track"]).unwrap();

        let exit = temp_env::with_vars(
            [("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None)],
            || {
                run_in_dir(directory.path(), || {
                    run_cli_with_context!(
                        cli,
                        |_command| ExitCode::SUCCESS,
                        |_command| ExitCode::FAILURE,
                        "sotp dry".to_owned(),
                        std::path::PathBuf::from("track/items"),
                    )
                })
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
        assert!(
            !directory.path().join("track/items/not-a-track/logs/telemetry.jsonl").exists(),
            "non-track branches must not create telemetry logs"
        );
    }

    #[test]
    fn test_completed_command_uses_only_existing_telemetry_jsonl_sink() {
        let _guard = process_env_lock().lock().unwrap();
        let directory = TempDir::new().unwrap();
        let track_id = "trace-single-sink";
        seed_repo(directory.path(), &format!("track/{track_id}"));
        let cli = Cli::try_parse_from(["sotp", "dry", "write", "--track-id", track_id]).unwrap();

        let exit = temp_env::with_vars(
            [("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None)],
            || {
                run_in_dir(directory.path(), || {
                    run_cli_with_context!(
                        cli,
                        |_command| ExitCode::SUCCESS,
                        |_command| ExitCode::FAILURE,
                        "sotp dry".to_owned(),
                        std::path::PathBuf::from("track/items"),
                    )
                })
            },
        );

        assert_eq!(exit, ExitCode::SUCCESS);
        let logs_path = directory.path().join("track/items").join(track_id).join("logs");
        let file_names: Vec<_> =
            fs::read_dir(&logs_path).unwrap().map(|entry| entry.unwrap().file_name()).collect();
        assert_eq!(file_names, vec![std::ffi::OsString::from("telemetry.jsonl")]);
    }

    #[test]
    fn test_command_identity_ignores_positional_payload_and_option_values() {
        let args = vec![
            std::ffi::OsString::from("sotp"),
            std::ffi::OsString::from("review"),
            std::ffi::OsString::from("codex-local"),
            std::ffi::OsString::from("--prompt"),
            std::ffi::OsString::from("do-not-record-this"),
            std::ffi::OsString::from("--model"),
            std::ffi::OsString::from("gpt-5.6-terra"),
            std::ffi::OsString::from("--round-type"),
            std::ffi::OsString::from("final"),
            std::ffi::OsString::from("--group"),
            std::ffi::OsString::from("cli"),
        ];

        assert_eq!(command_identity_from_args(&args), "sotp review codex-local");
    }

    #[test]
    fn test_telemetry_completion_eligible_accepts_track_workflow_commands() {
        assert!(telemetry_completion_eligible("track spec-design"));
        assert!(telemetry_completion_eligible("dry write"));
    }

    #[test]
    fn test_telemetry_completion_eligible_excludes_display_only_and_report_commands() {
        assert!(!telemetry_completion_eligible("track resolve"));
        assert!(!telemetry_completion_eligible("telemetry report"));
        assert!(!telemetry_completion_eligible("review results"));
    }

    // ── CliCommand::Dry entrypoint dispatch routing ───────────────────────────

    /// `sotp dry write --track-id x` must resolve to `CliCommand::Dry { cmd: DryCommand::Write }`.
    /// This also runs the same dispatch helper used by `main()` and checks that
    /// the DRY executor's `ExitCode` is returned to the process entrypoint.
    #[test]
    fn test_dry_dispatch_write_routes_to_dry_write_variant() {
        let cli = Cli::try_parse_from(["sotp", "dry", "write", "--track-id", "my-track"]).unwrap();
        let exit = dispatch_cli_test!(cli, |cmd| {
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
        let exit = dispatch_cli_test!(cli, |cmd| {
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
        let exit = dispatch_cli_test!(cli, |cmd| {
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
        let exit = dispatch_cli_with_test!(cli, |_cmd| ExitCode::FAILURE, |cmd| {
            match cmd {
                RefVerifyCommand::Run(args) => {
                    assert_eq!(args.track_id.as_deref(), Some("my-track"));
                }
                other => panic!("expected Run, got {other:?}"),
            }
            ExitCode::from(41)
        },);
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
        let exit = dispatch_cli_with_test!(cli, |_cmd| ExitCode::FAILURE, |cmd| {
            match cmd {
                RefVerifyCommand::CheckApproved(args) => {
                    assert_eq!(args.track_id.as_deref(), Some("my-track"));
                }
                other => panic!("expected CheckApproved, got {other:?}"),
            }
            ExitCode::from(43)
        },);
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
    /// entrypoint and dispatch through the common CLI entrypoint to the report command.
    #[test]
    fn test_telemetry_report_dispatch_via_run_cli_succeeds_with_existing_track() {
        let _guard = process_env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let track_id = "telemetry-route-track";
        seed_repo(root, &format!("track/{track_id}"));
        fs::create_dir_all(root.join("track").join("items").join(track_id)).unwrap();

        let exit = run_in_dir(root, || {
            let cli = Cli::try_parse_from([
                "sotp",
                "telemetry",
                "report",
                track_id,
                "--items-dir",
                "track/items",
            ])
            .unwrap();
            dispatch_cli_test!(cli, |_cmd| ExitCode::FAILURE)
        });
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_archive_completion_telemetry_moves_to_archived_track_logs() {
        let _guard = process_env_lock().lock().unwrap();
        let dir = TempDir::new().unwrap();
        let root = dir.path();
        let track_id = "archive-telemetry-track";
        seed_repo(root, &format!("track/{track_id}"));
        let items_dir = root.join("track").join("items");
        let track_dir = items_dir.join(track_id);
        fs::create_dir_all(&track_dir).unwrap();
        fs::create_dir_all(root.join("track").join("archive")).unwrap();
        fs::write(root.join(".gitignore"), "track/items/*/logs/\n").unwrap();
        fs::write(track_dir.join("tracked.txt"), "archive fixture\n").unwrap();
        run_git(root, &["add", ".gitignore", "track/items/archive-telemetry-track/tracked.txt"]);
        run_git(root, &["commit", "-m", "archive fixture", "--no-gpg-sign"]);
        let nested_cwd = root.join("nested").join("workdir");
        fs::create_dir_all(&nested_cwd).unwrap();
        let items_dir_text = items_dir.to_string_lossy().into_owned();
        temp_env::with_vars([("SOTP_TELEMETRY", Some("1")), ("SOTP_TELEMETRY_DIR", None)], || {
            run_in_dir(&nested_cwd, || {
                let cli = Cli::try_parse_from([
                    "sotp",
                    "track",
                    "archive",
                    "--track-id",
                    track_id,
                    "--items-dir",
                    items_dir_text.as_str(),
                ])
                .unwrap();
                let exit = run_cli_with_context!(
                    cli,
                    |_command| ExitCode::FAILURE,
                    |_command| ExitCode::FAILURE,
                    "sotp track archive".to_owned(),
                    items_dir.clone(),
                );
                assert_eq!(exit, ExitCode::SUCCESS);
            });
        });

        let active_log = items_dir.join(track_id).join("logs").join("telemetry.jsonl");
        let archived_log =
            root.join("track").join("archive").join(track_id).join("logs").join("telemetry.jsonl");
        assert!(
            archived_log.exists(),
            "successful archive completion must append to the archived track log: {archived_log:?}"
        );
        assert!(
            !active_log.exists(),
            "successful archive completion must not recreate the active track sink: {active_log:?}"
        );
        let line = fs::read_to_string(&archived_log).unwrap();
        let value: serde_json::Value = serde_json::from_str(line.trim()).unwrap();
        assert_eq!(value["event_type"], "TrackSubcommand");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["track_id"], track_id);
        assert_eq!(value["command"], "sotp track archive");
        assert_eq!(value["exit_code"], 0);
        let duration_ms = value["duration_ms"].as_u64().expect("duration_ms must be u64");
        assert!(
            (1..60_000).contains(&duration_ms),
            "duration_ms must be recorded and within a sane range: {duration_ms}"
        );
        assert!(
            value.get("timestamp").is_some(),
            "timestamp field must be present in the JSONL line"
        );
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
    /// `Cli::try_parse_from` and is dispatched through the common CLI entrypoint to
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
        let exit = dispatch_cli_test!(cli, |_cmd| ExitCode::FAILURE);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    // ── CliCommand::Template entrypoint dispatch routing ─────────────────────

    /// `sotp template export …` must parse into
    /// `CliCommand::Template { cmd: TemplateCommand::Export }` with every path
    /// argument mapped through the public CLI entrypoint.
    #[test]
    fn test_template_export_parses_to_template_export_variant() {
        use crate::commands::template::{TemplateCommand, TemplateExportArgs};

        let cli = Cli::try_parse_from([
            "sotp",
            "template",
            "export",
            "--workspace-root",
            "/ws",
            "--manifest-path",
            "/ws/boundary.json",
            "--overlay-dir",
            "/ws/overlay",
            "--output-dir",
            "/out",
        ])
        .unwrap();

        match cli.command {
            Some(CliCommand::Template {
                cmd: TemplateCommand::Export(TemplateExportArgs { output_dir, .. }),
            }) => {
                assert_eq!(output_dir, std::path::PathBuf::from("/out"));
            }
            _ => panic!("expected Template {{ Export }}, got a different variant"),
        }
    }

    /// An unrecognized `sotp template` subcommand must be rejected by clap.
    #[test]
    fn test_template_unknown_subcommand_is_rejected() {
        let result = Cli::try_parse_from(["sotp", "template", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized template subcommand must be rejected by clap");
    }

    // ── Hook telemetry wrapper paths ─────────────────────────────────────────

    /// `sotp hook dispatch skill-compliance` routes through `execute_hook_with_telemetry`.
    /// With empty stdin the advisory hook sees no prompt → no injection → exits 0.
    /// Telemetry is silently skipped (not on a track branch in CI) — no panic.
    #[test]
    fn test_hook_dispatch_skill_compliance_via_run_cli_exits_zero() {
        let cli = Cli::try_parse_from(["sotp", "hook", "dispatch", "skill-compliance"]).unwrap();
        let exit = dispatch_cli_test!(cli, |_cmd| ExitCode::FAILURE);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    // ── Verify telemetry wrapper paths ───────────────────────────────────────

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
        let exit = dispatch_cli_test!(cli, |_cmd| ExitCode::FAILURE);
        assert_ne!(exit, ExitCode::from(2u8), "exit 2 reserved for hook blocks");
    }

    // ── verify_command_gate_name coverage ────────────────────────────────────

    /// Each sampled `VerifyCommand` variant parsed from CLI args must map to its
    /// stable gate name.
    #[test]
    fn test_verify_command_gate_name_uses_expected_labels() {
        use super::verify_command_gate_name;

        let subcommands = [
            (["sotp", "verify", "latest-track"], "verify-latest-track"),
            (["sotp", "verify", "retention-gate"], "verify-retention-gate"),
            (["sotp", "verify", "arch-docs"], "verify-arch-docs"),
            (["sotp", "verify", "layers"], "verify-layers"),
        ];
        for (args, expected_gate_name) in &subcommands {
            let cli = Cli::try_parse_from(args).unwrap();
            if let Some(CliCommand::Verify { cmd }) = cli.command {
                let name = verify_command_gate_name(&cmd);
                assert_eq!(name, *expected_gate_name);
            }
        }
    }
}
