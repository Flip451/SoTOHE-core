//! `sotp verify` subcommand group.
//!
//! Each subcommand delegates to the corresponding `CliApp` method and
//! prints the outcome. Exits 0 on pass, 1 on failure.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::CliApp;

/// Arguments for spec-level verify subcommands.
#[derive(Args)]
pub struct SpecVerifyArgs {
    /// Path to the spec.md file to verify.
    spec_path: PathBuf,
}

/// Arguments for spec-states verify subcommand (includes strict-mode gate).
#[derive(Args)]
pub struct SpecStatesArgs {
    /// Path to the spec.md file to verify.
    /// When omitted, the active track is resolved from the current branch name
    /// and `track/items/<id>/spec.md` is verified. Fail-closed on non-track
    /// branches (CN-01, IN-08, AC-10).
    spec_path: Option<PathBuf>,
    /// Strict mode (merge gate): Yellow signals also fail. Default: only Red fails.
    #[arg(long)]
    strict: bool,
}

/// Verify subcommands for CI validation.
#[derive(Subcommand)]
pub enum VerifyCommand {
    /// Check tech-stack.md for unresolved TODO markers.
    TechStack(VerifyArgs),
    /// Check latest track artifacts for completeness.
    LatestTrack(VerifyArgs),
    /// Check architecture docs synchronization and text patterns.
    ArchDocs(VerifyArgs),
    /// Check workspace layer dependency rules via cargo metadata.
    Layers(VerifyArgs),
    /// Check local Git config uses .githooks as core.hooksPath.
    HooksPath(VerifyArgs),
    /// Check spec.md requirement lines for [source: ...] attribution.
    SpecAttribution(SpecVerifyArgs),
    /// Check spec.md YAML frontmatter for required fields.
    SpecFrontmatter(SpecVerifyArgs),
    /// Check canonical module ownership (no reimplementation outside canonical modules).
    CanonicalModules(VerifyArgs),
    /// Check Rust source file sizes against module_limits thresholds.
    ModuleSize(VerifyArgs),
    /// Check libs/domain/src/ for hexagonal purity violations (forbidden I/O patterns).
    DomainPurity(VerifyArgs),
    /// Check libs/domain/src/ for pub String fields (should be enums or newtypes).
    DomainStrings(VerifyArgs),
    /// Check libs/usecase/src/ for hexagonal purity violations (forbidden patterns).
    UsecasePurity(VerifyArgs),
    /// Check that local file links in Markdown documents resolve to existing files.
    DocLinks(VerifyArgs),
    /// Check that plan.md files are up-to-date with metadata.json renderings.
    ViewFreshness(VerifyArgs),
    /// Check spec.md source tag signals match frontmatter and red == 0 gate.
    SpecSignals(SpecVerifyArgs),
    /// Check spec.md contains a ## Domain States section with table data rows.
    SpecStates(SpecStatesArgs),
    /// Validate structured-ref fields (adr_refs, convention_refs, spec_refs, informal_grounds)
    /// per ADR 2026-04-19-1242 §D2.3.
    PlanArtifactRefs(PlanArtifactRefsArgs),

    /// Verify catalogue-spec ref integrity (SoT Chain ② binary gate):
    /// detects dangling anchors, hash drift, and stale signals.
    ///
    /// ADR `2026-04-23-0344-catalogue-spec-signal-activation.md` §D1.5 / §D3.2.
    CatalogueSpecRefs(CatalogueSpecRefsArgs),

    /// Check catalogue-spec signal gate results
    /// (`check_catalogue_spec_signals`) for each tddd-enabled layer on the
    /// current branch. `strict=false` CI interim mode: Red → error, Yellow
    /// → warning. ADR `2026-04-23-0344-catalogue-spec-signal-activation.md`
    /// §D4.1.
    CatalogueSpecSignals(CatalogueSpecSignalsArgs),

    /// Verify ADR decision signal grounds across `knowledge/adr/`
    /// (SoT Chain ADR-internal binary gate): `red_count >= 1` → exit 1
    /// with a stderr summary; `red_count == 0` → exit 0. ADR
    /// `2026-04-27-1234-adr-decision-traceability-lifecycle.md` §D1 / AC-01.
    AdrSignals(VerifyArgs),
}

/// Arguments for `catalogue-spec-signals` verify subcommand.
#[derive(Args)]
pub struct CatalogueSpecSignalsArgs {
    /// Path to the track items root directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Workspace root directory.
    #[arg(long, default_value = ".")]
    workspace_root: PathBuf,

    /// Enable strict mode (Yellow also blocks). Default: CI interim mode.
    #[arg(long)]
    strict: bool,
}

/// Arguments for `catalogue-spec-refs` verify subcommand.
#[derive(Args)]
pub struct CatalogueSpecRefsArgs {
    /// Track ID (directory name under items_dir).
    /// When omitted, resolved from the current git branch (`track/<id>`).
    #[arg(long)]
    track_id: Option<String>,

    /// Path to the track items root directory.
    #[arg(long, default_value = "track/items")]
    items_dir: PathBuf,

    /// Workspace root directory (must contain `architecture-rules.json`).
    #[arg(long, default_value = ".")]
    workspace_root: PathBuf,

    /// Skip the stale-signals check (used in pre-commit where signals are
    /// regenerated in the next step after this verification).
    #[arg(long)]
    skip_stale: bool,
}

/// Arguments for plan-artifact-refs verify subcommand.
#[derive(Args)]
pub struct PlanArtifactRefsArgs {
    /// Path to the track directory (e.g., track/items/<id>).
    /// When omitted, the active track is resolved from the current branch name.
    #[arg(long)]
    track_dir: Option<PathBuf>,
}

/// Common arguments for all verify subcommands.
#[derive(Args)]
pub struct VerifyArgs {
    /// Project root directory (defaults to current directory).
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
}

impl VerifyCommand {
    /// Returns the `track/items` path that the underlying command would use as its items root.
    ///
    /// Used by `execute_verify_with_telemetry` to anchor the telemetry writer to the same
    /// base directory as the underlying command, so that non-default `--project-root`,
    /// `--workspace-root`, or `--items-dir` invocations write telemetry to the correct
    /// location (P1 fix: was hardcoded `"track/items"` relative to CWD).
    pub fn items_dir(&self) -> PathBuf {
        match self {
            // Project-root–based commands: derive items_dir from project_root.
            VerifyCommand::TechStack(a)
            | VerifyCommand::LatestTrack(a)
            | VerifyCommand::ArchDocs(a)
            | VerifyCommand::Layers(a)
            | VerifyCommand::HooksPath(a)
            | VerifyCommand::CanonicalModules(a)
            | VerifyCommand::ModuleSize(a)
            | VerifyCommand::DomainPurity(a)
            | VerifyCommand::DomainStrings(a)
            | VerifyCommand::UsecasePurity(a)
            | VerifyCommand::DocLinks(a)
            | VerifyCommand::ViewFreshness(a)
            | VerifyCommand::AdrSignals(a) => a.project_root.join("track").join("items"),

            // Workspace-root–based commands: the explicit --items-dir field is the
            // primary artifact root.  Only rewrite to `<workspace_root>/track/items`
            // when items_dir still holds the CLI default value ("track/items") AND
            // --workspace-root was set to a non-default value.  An explicit non-default
            // --items-dir must be passed through unchanged so that resolve_telemetry_writer
            // anchors to the path the caller actually supplied.
            VerifyCommand::CatalogueSpecRefs(a) => {
                if a.items_dir.as_os_str() == "track/items" && a.workspace_root.as_os_str() != "." {
                    a.workspace_root.join("track").join("items")
                } else {
                    a.items_dir.clone()
                }
            }
            // CatalogueSpecSignals is CWD-anchored for branch gating: the underlying
            // command uses --workspace-root for file I/O but git branch discovery
            // always runs against the CWD repo.  Passing the workspace_root-joined
            // path to resolve_telemetry_writer would anchor branch discovery to a
            // different checkout, causing GateEval misattribution.  Return the raw
            // items_dir field (defaulting to "track/items") so that
            // resolve_telemetry_writer uses CWD for branch discovery.
            VerifyCommand::CatalogueSpecSignals(a) => a.items_dir.clone(),

            // Spec-path–based commands: derive items_dir from the spec path when
            // the path is deep enough (spec.md → <track_id>/ → track/items/).
            // Only trust the derived path when it has more than one component
            // (i.e., it is not the filesystem root "/").  A spec_path outside the
            // canonical `track/items/<id>/` tree (e.g. `/tmp/spec.md`) would
            // yield "/" via parent().parent(), causing resolve_telemetry_writer to
            // return None and silently drop the GateEval event.  Fall back to the
            // CWD-relative "track/items" in that case so git branch discovery can
            // still identify the track.
            VerifyCommand::SpecAttribution(a) | VerifyCommand::SpecFrontmatter(a) => a
                .spec_path
                .parent()
                .and_then(|p| p.parent())
                .filter(|p| p.components().count() > 1)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("track/items")),
            VerifyCommand::SpecSignals(a) => a
                .spec_path
                .parent()
                .and_then(|p| p.parent())
                .filter(|p| p.components().count() > 1)
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("track/items")),
            VerifyCommand::SpecStates(a) => a
                .spec_path
                .as_deref()
                .and_then(|p| p.parent())
                .and_then(|p| p.parent())
                .filter(|p| p.components().count() > 1)
                .map(|p| p.to_path_buf())
                // No explicit spec path: use CWD-relative "track/items" (the same
                // layout resolve_telemetry_writer expects). The underlying command
                // also does git discovery from CWD, so both anchor to the same root.
                .unwrap_or_else(|| PathBuf::from("track/items")),
            // PlanArtifactRefs: track_dir is track/items/<track_id>/, so .parent() =
            // track/items/. When omitted, use the same CWD-relative fallback.
            VerifyCommand::PlanArtifactRefs(a) => a
                .track_dir
                .as_deref()
                .and_then(|p| p.parent())
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| PathBuf::from("track/items")),
        }
    }
}

/// Dispatches `cmd` to the appropriate `CliApp` method and returns the raw `Result<CommandOutcome,
/// String>` without printing anything.
///
/// `execute_with_summary` delegates here so the 20-arm match is not duplicated.
#[allow(clippy::too_many_lines)]
fn dispatch_to_outcome(
    app: &CliApp,
    cmd: VerifyCommand,
) -> Result<cli_composition::CommandOutcome, String> {
    match cmd {
        VerifyCommand::TechStack(args) => app.verify_tech_stack(args.project_root),
        VerifyCommand::LatestTrack(args) => app.verify_latest_track(args.project_root),
        VerifyCommand::ArchDocs(args) => app.verify_arch_docs(args.project_root),
        VerifyCommand::Layers(args) => app.verify_layers(args.project_root),
        VerifyCommand::HooksPath(args) => app.verify_hooks_path(args.project_root),
        VerifyCommand::SpecAttribution(args) => app.verify_spec_attribution(args.spec_path),
        VerifyCommand::SpecFrontmatter(args) => app.verify_spec_frontmatter(args.spec_path),
        VerifyCommand::CanonicalModules(args) => app.verify_canonical_modules(args.project_root),
        VerifyCommand::ModuleSize(args) => app.verify_module_size(args.project_root),
        VerifyCommand::DomainPurity(args) => app.verify_domain_purity(args.project_root),
        VerifyCommand::DomainStrings(args) => app.verify_domain_strings(args.project_root),
        VerifyCommand::UsecasePurity(args) => app.verify_usecase_purity(args.project_root),
        VerifyCommand::DocLinks(args) => app.verify_doc_links(args.project_root),
        VerifyCommand::ViewFreshness(args) => app.verify_view_freshness(args.project_root),
        VerifyCommand::SpecSignals(args) => app.verify_spec_signals(args.spec_path),
        VerifyCommand::SpecStates(args) => app.verify_spec_states(args.spec_path, args.strict),
        VerifyCommand::PlanArtifactRefs(args) => app.verify_plan_artifact_refs(args.track_dir),
        VerifyCommand::CatalogueSpecRefs(args) => app.verify_catalogue_spec_refs(
            args.track_id,
            args.items_dir,
            args.workspace_root,
            args.skip_stale,
        ),
        VerifyCommand::CatalogueSpecSignals(args) => {
            app.verify_catalogue_spec_signals(args.items_dir, args.workspace_root, args.strict)
        }
        VerifyCommand::AdrSignals(args) => app.verify_adr_signals(args.project_root),
    }
}

/// Dispatch `cmd`, print its outcome, and return both the exit code and the raw stdout text.
///
/// The stdout text is used by `execute_verify_with_telemetry` as the `reason_summary` field
/// in the emitted `TelemetryEvent::GateEval` (T005 contract: `reason_summary` should reflect
/// actual findings rather than a static label).
pub fn execute_with_summary(cmd: VerifyCommand) -> (ExitCode, Option<String>) {
    let app = CliApp::new();
    run_capturing(dispatch_to_outcome(&app, cmd))
}

/// Dispatch `cmd`, print its outcome, and return the exit code.
///
/// Test-only convenience wrapper around [`execute_with_summary`] that discards
/// the stdout text. Production code goes through [`execute_with_summary`] directly
/// (called by `execute_verify_with_telemetry` in `main.rs`).
#[cfg(test)]
pub(super) fn execute(cmd: VerifyCommand) -> ExitCode {
    execute_with_summary(cmd).0
}

/// Dispatch a `CommandOutcome` result to an `(ExitCode, Option<String>)`.
///
/// Prints stdout (if present) and stderr (if present), then returns the exit code and the
/// summary text. The summary is stdout when present; falls back to stderr when stdout is absent
/// (some gates, e.g. `catalogue-spec-refs`, report findings on stderr only). The caller can use
/// the summary text as `reason_summary` for telemetry events.
pub(super) fn run_capturing(
    result: Result<cli_composition::CommandOutcome, String>,
) -> (ExitCode, Option<String>) {
    match result {
        Ok(outcome) => {
            // Prefer stdout; fall back to stderr so stderr-only gates (e.g.
            // catalogue-spec-refs) still populate reason_summary in telemetry.
            let summary = outcome.stdout.clone().or_else(|| outcome.stderr.clone());
            let exit = print_outcome(&outcome);
            (exit, summary)
        }
        Err(msg) => {
            eprintln!("{msg}");
            // Return the error message as the summary so reason_summary in
            // GateEval telemetry reflects the actual failure rather than
            // falling back to the static gate name.
            (ExitCode::FAILURE, Some(msg))
        }
    }
}

/// Emit a `CommandOutcome` and return the corresponding `ExitCode`.
pub(super) fn print_outcome(outcome: &cli_composition::CommandOutcome) -> ExitCode {
    if let Some(ref s) = outcome.stdout {
        println!("{s}");
    }
    if let Some(ref s) = outcome.stderr {
        eprintln!("{s}");
    }
    ExitCode::from(outcome.exit_code)
}

/// Print a `[SKIP]` message and return `ExitCode::SUCCESS`.
///
/// Used only by `#[cfg(test)]` dispatch helpers to simulate the AC-16 skip output.
#[cfg(test)]
pub(super) fn print_skip(label: &str, reason: &str) -> ExitCode {
    println!("--- {label} ---");
    println!("[SKIP] {reason}");
    println!("--- {label} SKIPPED ---");
    ExitCode::SUCCESS
}

// ── CI verify skip helpers — test-only ───────────────────────────────────────

/// Core skip-or-error logic for CI verify track resolution.
///
/// Separated for unit-test injection of a stub [`BranchReaderPort`].
///
/// # Errors
///
/// Returns a human-readable error string for non-skip failures.
#[cfg(test)]
pub(super) fn resolve_ci_verify_track_id_with_reader(
    branch_reader: std::sync::Arc<dyn usecase::track_resolution::BranchReaderPort>,
) -> Result<Option<String>, String> {
    use usecase::track_resolution::{
        ActiveTrackResolveError, ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
        TrackResolutionError,
    };

    let interactor = ActiveTrackResolveInteractor::new(branch_reader);
    match interactor.resolve_active_track() {
        Ok(track_id) => Ok(Some(track_id)),
        Err(ActiveTrackResolveError::Resolution(
            TrackResolutionError::NotTrackBranch(_)
            | TrackResolutionError::DetachedHead
            | TrackResolutionError::NoBranch,
        )) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

// ── #[cfg(test)] dispatch helpers — mirror AC-16 skip paths with injected resolvers ──
//
// These functions duplicate the skip-dispatch logic of the four `execute()` match arms
// (now delegated to CliApp) so unit tests can inject a stub `BranchReaderPort`.
// They call the shared `resolve_ci_verify_track_id_with_reader` helper directly,
// bypassing the CliApp layer — sufficient for testing the skip detection contract.

/// Execute the `SpecStates` dispatch logic with an injected branch reader.
#[cfg(test)]
pub(super) fn dispatch_spec_states_with_resolver(
    args: SpecStatesArgs,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> ExitCode {
    dispatch_spec_states_with_resolver_and_repo_root(args, resolver, || {
        use infrastructure::git_cli::GitRepository as _;
        infrastructure::git_cli::SystemGitRepo::discover()
            .map(|repo| repo.root().to_path_buf())
            .map_err(|e| format!("cannot discover git repository: {e}"))
    })
}

/// Execute the `SpecStates` dispatch logic with injected branch and repo-root readers.
#[cfg(test)]
pub(super) fn dispatch_spec_states_with_resolver_and_repo_root(
    args: SpecStatesArgs,
    resolver: impl Fn() -> Result<Option<String>, String>,
    repo_root_resolver: impl Fn() -> Result<PathBuf, String>,
) -> ExitCode {
    use infrastructure::verify::{VerifyFinding, VerifyOutcome};

    let spec_path = match args.spec_path {
        Some(p) => p,
        None => match resolver() {
            Ok(None) => {
                return print_skip("verify spec states", "not on a track branch; skipping");
            }
            Ok(Some(track_id)) => {
                let repo_root = match repo_root_resolver() {
                    Ok(root) => root,
                    Err(msg) => {
                        eprintln!("{msg}");
                        return ExitCode::FAILURE;
                    }
                };
                let track_dir = repo_root.join("track/items").join(&track_id);
                let spec_json_path = track_dir.join("spec.json");
                let spec_md_path = track_dir.join("spec.md");
                if spec_json_path.exists() || spec_md_path.exists() {
                    spec_md_path
                } else {
                    // Neither spec artifact exists (Phase 0) — emit SKIP instead of failing.
                    return print_skip(
                        "verify spec states",
                        "spec artifacts not yet generated (Phase 0); skipping",
                    );
                }
            }
            Err(msg) => {
                eprintln!("{msg}");
                return ExitCode::FAILURE;
            }
        },
    };
    let outcome = match infrastructure::verify::trusted_root::resolve_trusted_root(&spec_path) {
        Ok(trusted_root) => {
            infrastructure::verify::spec_states::verify(&spec_path, args.strict, &trusted_root)
        }
        Err(e) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "cannot resolve trusted_root for {}: {e}",
            spec_path.display()
        ))]),
    };
    dispatch_print_outcome("verify spec states", &outcome)
}

/// Execute the `PlanArtifactRefs` dispatch logic with an injected branch reader.
#[cfg(test)]
pub(super) fn dispatch_plan_artifact_refs_with_resolver(
    args: PlanArtifactRefsArgs,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> ExitCode {
    use infrastructure::verify::{VerifyFinding, VerifyOutcome};

    if args.track_dir.is_none() {
        match resolver() {
            Ok(None) => {
                return print_skip("verify plan artifact refs", "not on a track branch; skipping");
            }
            Ok(Some(_)) | Err(_) => {
                // Fall through: execute_plan_artifact_refs handles the rest.
            }
        }
    }
    let outcome = match &args.track_dir {
        Some(dir) if dir.is_dir() => infrastructure::verify::plan_artifact_refs::verify(dir),
        Some(dir) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "Track directory does not exist: {}",
            dir.display()
        ))]),
        None => {
            use std::sync::Arc;

            use infrastructure::git_cli::GitRepository as _;
            use usecase::track_resolution::{
                ActiveTrackResolveInteractor, ActiveTrackResolveService as _,
            };
            let maybe_dir =
                infrastructure::git_cli::SystemGitRepo::discover().ok().and_then(|repo| {
                    let repo_root = repo.root().to_path_buf();
                    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
                    let track_id = interactor.resolve_active_track().ok()?;
                    let track_dir = repo_root.join("track/items").join(&track_id);
                    if track_dir.is_dir() { Some(track_dir) } else { None }
                });
            match maybe_dir {
                Some(dir) => infrastructure::verify::plan_artifact_refs::verify(&dir),
                None => VerifyOutcome::from_findings(vec![VerifyFinding::error(
                    "Cannot resolve active track directory: not on a track/* branch or directory \
                     does not exist. Use --track-dir <PATH> to specify the track directory \
                     explicitly."
                        .to_owned(),
                )]),
            }
        }
    };
    dispatch_print_outcome("verify plan artifact refs", &outcome)
}

/// Execute the `CatalogueSpecRefs` skip-detection path with an injected branch reader.
#[cfg(test)]
pub(super) fn dispatch_catalogue_spec_refs_skip_with_resolver(
    track_id: Option<String>,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> Option<ExitCode> {
    if track_id.is_none() {
        match resolver() {
            Ok(None) => {
                return Some(print_skip(
                    "verify catalogue-spec-refs",
                    "not on a track branch; skipping",
                ));
            }
            Ok(Some(_)) => {
                // Fall through.
            }
            Err(msg) => {
                eprintln!("{msg}");
                return Some(ExitCode::FAILURE);
            }
        }
    }
    None
}

/// Execute the `CatalogueSpecSignals` dispatch logic with an injected branch reader.
#[cfg(test)]
pub(super) fn dispatch_catalogue_spec_signals_with_resolver(
    args: CatalogueSpecSignalsArgs,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> ExitCode {
    match resolver() {
        Ok(None) => print_skip("verify catalogue-spec signals", "not on a track branch; skipping"),
        Ok(Some(_)) => {
            let outcome =
                infrastructure::verify::catalogue_spec_signals::execute_catalogue_spec_signals_check(
                    args.items_dir,
                    args.workspace_root,
                    args.strict,
                );
            dispatch_print_outcome("verify catalogue-spec signals", &outcome)
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

/// `print_outcome` variant for `VerifyOutcome` used exclusively by `#[cfg(test)]` dispatch helpers.
///
/// Production code uses `print_outcome(&CommandOutcome)`. This variant is kept
/// so the dispatch helpers can continue to test the skip/fail-closed discrimination
/// without importing CliApp.
#[cfg(test)]
pub(super) fn dispatch_print_outcome(
    label: &str,
    outcome: &infrastructure::verify::VerifyOutcome,
) -> ExitCode {
    println!("--- {label} ---");
    if outcome.findings().is_empty() {
        println!("[OK] All checks passed.");
        println!("--- {label} PASSED ---");
        ExitCode::SUCCESS
    } else {
        for finding in outcome.findings() {
            println!("{finding}");
        }
        if outcome.has_errors() {
            println!("--- {label} FAILED ---");
            ExitCode::FAILURE
        } else {
            println!("--- {label} PASSED ---");
            ExitCode::SUCCESS
        }
    }
}

/// `execute_catalogue_spec_signals` for `#[cfg(test)]` direct-infra tests.
///
/// Production delegates to CliApp; tests that need `VerifyOutcome` semantics
/// (`.has_errors()`, `.findings()`) call this to check the infrastructure layer
/// directly without going through the CommandOutcome boundary.
#[cfg(test)]
pub(super) fn execute_catalogue_spec_signals(
    items_dir: std::path::PathBuf,
    track_id: String,
    workspace_root: std::path::PathBuf,
    strict: bool,
) -> infrastructure::verify::VerifyOutcome {
    infrastructure::verify::catalogue_spec_signals::execute_catalogue_spec_signals(
        items_dir,
        track_id,
        workspace_root,
        strict,
    )
}

/// `execute_verify_adr_signals` for `#[cfg(test)]` direct-infra tests.
#[cfg(test)]
pub(super) fn execute_verify_adr_signals(
    project_root: &std::path::Path,
) -> infrastructure::verify::VerifyOutcome {
    infrastructure::verify::adr_signals::execute_verify_adr_signals(project_root)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod hooks_path_cli_tests {
    use std::path::PathBuf;
    use std::process::{Command, ExitCode};

    use clap::Parser;
    use tempfile::TempDir;

    use super::{VerifyArgs, VerifyCommand, execute};

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: VerifyCommand,
    }

    #[test]
    fn test_verify_hooks_path_parse_with_project_root_maps_to_hooks_path_variant() {
        let cli = TestCli::try_parse_from([
            "sotp",
            "hooks-path",
            "--project-root",
            "/tmp/hooks-path-project",
        ])
        .unwrap();

        match cli.cmd {
            VerifyCommand::HooksPath(args) => {
                assert_eq!(args.project_root, PathBuf::from("/tmp/hooks-path-project"));
            }
            _ => panic!("expected HooksPath variant"),
        }
    }

    #[test]
    fn test_verify_hooks_path_execute_with_githooks_configured_returns_success() {
        let tmp = TempDir::new().unwrap();
        run_git(tmp.path(), &["init"]);
        run_git(tmp.path(), &["config", "--local", "core.hooksPath", ".githooks"]);

        let exit = execute(VerifyCommand::HooksPath(VerifyArgs {
            project_root: tmp.path().to_path_buf(),
        }));

        assert_eq!(exit, ExitCode::SUCCESS);
    }

    fn run_git(root: &std::path::Path, args: &[&str]) {
        let output = Command::new("git")
            .current_dir(root)
            .args(args)
            .output()
            .expect("git command must run in verify hooks-path CLI tests");
        assert!(
            output.status.success(),
            "git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
#[path = "verify_tests.rs"]
mod tests;
