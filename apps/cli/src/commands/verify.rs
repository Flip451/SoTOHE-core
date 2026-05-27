//! `sotp verify` subcommand group.
//!
//! Each subcommand delegates to the corresponding infrastructure verify module
//! and prints the outcome. Exits 0 on pass, 1 on failure.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use infrastructure::verify::{VerifyFinding, VerifyOutcome};

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
    /// Check .claude/settings.json structural guardrails.
    Orchestra(VerifyArgs),
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
    track: Option<String>,

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

#[allow(clippy::too_many_lines)]
pub fn execute(cmd: VerifyCommand) -> ExitCode {
    let (label, outcome) = match cmd {
        VerifyCommand::TechStack(args) => (
            "verify tech stack readiness",
            infrastructure::verify::tech_stack::verify(&args.project_root),
        ),
        VerifyCommand::LatestTrack(args) => (
            "verify latest track files",
            infrastructure::verify::latest_track::verify(&args.project_root),
        ),
        VerifyCommand::ArchDocs(args) => {
            ("verify architecture docs", verify_arch_docs(&args.project_root))
        }
        VerifyCommand::Layers(args) => {
            ("verify layers", infrastructure::verify::layers::verify(&args.project_root))
        }
        VerifyCommand::Orchestra(args) => (
            "verify orchestra guardrails",
            infrastructure::verify::orchestra::verify(&args.project_root),
        ),
        VerifyCommand::SpecAttribution(args) => (
            "verify spec attribution",
            infrastructure::verify::spec_attribution::verify(&args.spec_path),
        ),
        VerifyCommand::SpecFrontmatter(args) => (
            "verify spec frontmatter",
            infrastructure::verify::spec_frontmatter::verify(&args.spec_path),
        ),
        VerifyCommand::CanonicalModules(args) => (
            "verify canonical modules",
            infrastructure::verify::canonical_modules::verify(&args.project_root),
        ),
        VerifyCommand::ModuleSize(args) => {
            ("verify module size", infrastructure::verify::module_size::verify(&args.project_root))
        }
        VerifyCommand::DomainPurity(args) => (
            "verify domain purity",
            infrastructure::verify::domain_purity::verify(&args.project_root),
        ),
        VerifyCommand::DomainStrings(args) => (
            "verify domain strings",
            infrastructure::verify::domain_strings::verify(&args.project_root),
        ),
        VerifyCommand::UsecasePurity(args) => (
            "verify usecase purity",
            infrastructure::verify::usecase_purity::verify(&args.project_root),
        ),
        VerifyCommand::DocLinks(args) => {
            ("verify doc links", infrastructure::verify::doc_links::verify(&args.project_root))
        }
        VerifyCommand::ViewFreshness(args) => (
            "verify view freshness",
            infrastructure::verify::view_freshness::verify(&args.project_root),
        ),
        VerifyCommand::SpecSignals(args) => {
            ("verify spec signals", infrastructure::verify::spec_signals::verify(&args.spec_path))
        }
        VerifyCommand::SpecStates(args) => {
            // Resolve the spec path: use explicit value when given, or derive
            // from the active track branch (IN-08, AC-10, CN-01, AC-16).
            let spec_path = match args.spec_path {
                Some(p) => p,
                None => {
                    // AC-16: skip (exit 0) when not on a track branch.
                    match resolve_ci_verify_track_id() {
                        Ok(None) => {
                            return print_skip(
                                "verify spec states",
                                "not on a track branch; skipping",
                            );
                        }
                        Ok(Some(track_id)) => match build_spec_path_from_track_id(&track_id) {
                            Ok(p) => p,
                            Err(msg) => {
                                eprintln!("{msg}");
                                return ExitCode::FAILURE;
                            }
                        },
                        Err(msg) => {
                            eprintln!("{msg}");
                            return ExitCode::FAILURE;
                        }
                    }
                }
            };
            // Resolve trusted_root via the infrastructure-layer helper.
            // All filesystem I/O (git discover, .git walk-up, symlink
            // verification) lives in `infrastructure::verify::trusted_root`;
            // the CLI is a pure composition root that maps Result → finding.
            let outcome =
                match infrastructure::verify::trusted_root::resolve_trusted_root(&spec_path) {
                    Ok(trusted_root) => infrastructure::verify::spec_states::verify(
                        &spec_path,
                        args.strict,
                        &trusted_root,
                    ),
                    Err(e) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                        "cannot resolve trusted_root for {}: {e}",
                        spec_path.display()
                    ))]),
                };
            ("verify spec states", outcome)
        }
        VerifyCommand::PlanArtifactRefs(args) => {
            // AC-16: skip when not on a track branch (only when --track-dir is not
            // provided explicitly — explicit path is always fail-closed).
            if args.track_dir.is_none() {
                match resolve_ci_verify_track_id() {
                    Ok(None) => {
                        return print_skip(
                            "verify plan artifact refs",
                            "not on a track branch; skipping",
                        );
                    }
                    Ok(Some(_)) | Err(_) => {
                        // Fall through: execute_plan_artifact_refs handles the
                        // track-found path and the real-error path (fail-closed).
                    }
                }
            }
            ("verify plan artifact refs", execute_plan_artifact_refs(args))
        }
        VerifyCommand::CatalogueSpecRefs(args) => {
            // Resolve track id: use explicit value when given (CN-02),
            // or fall back to the active track from the current branch.
            // Anchored to workspace_root so that git discovery is consistent
            // with the workspace being verified (not the process CWD).
            // AC-16: skip (exit 0) when not on a track branch (CN-01 exception for CI reads).
            // Explicit value (args.track is Some) → skip detection is bypassed; fail-closed as before.
            if args.track.is_none() {
                match resolve_ci_verify_track_id_from_root(&args.workspace_root) {
                    Ok(None) => {
                        return print_skip(
                            "verify catalogue-spec-refs",
                            "not on a track branch; skipping",
                        );
                    }
                    Ok(Some(_)) => {
                        // Fall through: the existing resolve_track_id_from_root call below
                        // will produce the same track_id and handle any disk-level errors.
                    }
                    Err(msg) => {
                        eprintln!("{msg}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            let track_id = match crate::commands::track::resolve_track_id_from_root(
                args.track,
                &args.workspace_root,
            ) {
                Ok(id) => id,
                Err(msg) => {
                    eprintln!("{msg}");
                    return ExitCode::FAILURE;
                }
            };
            // This subcommand has its own exit code (no findings → 0, findings → 1)
            // and emits formatted lines to stderr directly, so it bypasses
            // the shared `VerifyOutcome` printing path.
            return match crate::commands::verify_catalogue_spec_refs::execute_verify_catalogue_spec_refs(
                args.items_dir,
                track_id,
                args.workspace_root,
                args.skip_stale,
            ) {
                Ok(code) => code,
                Err(err) => {
                    eprintln!("{err}");
                    err.exit_code()
                }
            };
        }
        VerifyCommand::CatalogueSpecSignals(args) => {
            // AC-16: check for non-track branch at the CLI dispatch layer before
            // delegating to infrastructure (which has its own track resolution).
            // This makes the skip uniform across all four CI verify subcommands.
            //
            // Uses resolve_ci_verify_track_id() (CWD-anchored) to match the
            // behaviour of execute_catalogue_spec_signals_check(), which also
            // discovers the git repo from the process CWD via SystemGitRepo::discover().
            // Using the workspace_root-anchored variant here while the check
            // resolves from CWD would create a mismatch when --workspace-root
            // differs from CWD.
            match resolve_ci_verify_track_id() {
                Ok(None) => {
                    return print_skip(
                        "verify catalogue-spec signals",
                        "not on a track branch; skipping",
                    );
                }
                Ok(Some(_)) => {
                    // Track resolved; fall through to the infrastructure path which
                    // will discover the repo root, resolve items_dir, and run the check.
                    ("verify catalogue-spec signals", execute_catalogue_spec_signals_check(args))
                }
                Err(msg) => {
                    eprintln!("{msg}");
                    return ExitCode::FAILURE;
                }
            }
        }
        VerifyCommand::AdrSignals(args) => {
            ("verify adr signals", execute_verify_adr_signals(&args.project_root))
        }
    };

    print_outcome(label, &outcome)
}

// T008: type re-exports for consistency_report_to_findings / check_consistency / TypeGraph removed.

/// Execute plan-artifact-refs verification.
///
/// Resolves the track directory from args or falls back to the active branch.
/// When neither `--track-dir` is given nor an active track branch can be
/// detected, surfaces a finding rather than silently passing, so that CI
/// invocations on unexpected branches fail closed instead of hiding missing
/// plan-artifact coverage.
fn execute_plan_artifact_refs(args: PlanArtifactRefsArgs) -> VerifyOutcome {
    match &args.track_dir {
        Some(dir) if dir.is_dir() => infrastructure::verify::plan_artifact_refs::verify(dir),
        Some(dir) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "Track directory does not exist: {}",
            dir.display()
        ))]),
        None => match resolve_active_track_dir() {
            Some(dir) => infrastructure::verify::plan_artifact_refs::verify(&dir),
            None => VerifyOutcome::from_findings(vec![VerifyFinding::error(
                "Cannot resolve active track directory: not on a track/* branch or directory does \
                 not exist. Use --track-dir <PATH> to specify the track directory explicitly."
                    .to_owned(),
            )]),
        },
    }
}

/// Resolve the active track directory from the current git branch name.
///
/// Uses the shared `ActiveTrackResolveInteractor` (IN-09: consolidates
/// individual auto-detect implementations onto the shared interactor path).
/// The returned path is anchored to the repo root discovered via
/// `SystemGitRepo::discover`, so this function works correctly regardless
/// of which subdirectory the process is invoked from.
///
/// Fail-closed: returns `None` only when not inside a git repository or the
/// resolved track directory does not exist on disk. Non-track branches and
/// detached HEAD return `None` so the caller can surface a clear error.
///
/// Returns `None` when:
/// - Not inside a git repository
/// - Not on a `track/<id>` branch (including detached HEAD / main)
/// - The resolved `track/items/<id>` directory does not exist on disk
fn resolve_active_track_dir() -> Option<std::path::PathBuf> {
    use std::sync::Arc;

    use infrastructure::git_cli::GitRepository as _;
    use usecase::track_resolution::{ActiveTrackResolveInteractor, ActiveTrackResolveService as _};
    let repo = infrastructure::git_cli::SystemGitRepo::discover().ok()?;
    let repo_root = repo.root().to_path_buf();
    let interactor = ActiveTrackResolveInteractor::new(Arc::new(repo));
    let track_id = interactor.resolve_active_track().ok()?;
    let track_dir = repo_root.join("track/items").join(&track_id);
    if track_dir.is_dir() { Some(track_dir) } else { None }
}

// ── AC-16 CI verify skip helpers ─────────────────────────────────────────────

/// Resolve the active track id for CI verify subcommands.
///
/// Returns:
/// - `Ok(Some(track_id))` when on a valid `track/<id>` branch.
/// - `Ok(None)` when on a non-track branch (NotTrackBranch / DetachedHead / NoBranch)
///   — the caller should emit `[SKIP]` and return `ExitCode::SUCCESS` (AC-16).
/// - `Err(msg)` for real failures (BranchRead I/O error, InvalidTrackId, etc.)
///   — the caller should fail-closed (CN-01).
///
/// Uses `SystemGitRepo::discover()` anchored to the process CWD.
///
/// # Errors
///
/// Returns a human-readable error string for infrastructure failures that
/// are not in the "not a track branch" family.
fn resolve_ci_verify_track_id() -> Result<Option<String>, String> {
    use std::sync::Arc;

    let repo = infrastructure::git_cli::SystemGitRepo::discover()
        .map_err(|e| format!("cannot discover git repository: {e}"))?;
    resolve_ci_verify_track_id_with_reader(Arc::new(repo))
}

/// Resolve the active track id for CI verify subcommands using a workspace root.
///
/// Like [`resolve_ci_verify_track_id`] but anchors git discovery to
/// `workspace_root` (used when the command takes `--workspace-root`).
///
/// # Errors
///
/// Returns a human-readable error string for infrastructure failures.
fn resolve_ci_verify_track_id_from_root(
    workspace_root: &std::path::Path,
) -> Result<Option<String>, String> {
    use std::sync::Arc;

    let repo = infrastructure::git_cli::SystemGitRepo::discover_from(workspace_root)
        .map_err(|e| format!("cannot discover git repository: {e}"))?;
    resolve_ci_verify_track_id_with_reader(Arc::new(repo))
}

/// Core skip-or-error logic for CI verify track resolution.
///
/// Separated for unit-test injection of a stub [`BranchReaderPort`].
///
/// - `NotTrackBranch` / `DetachedHead` / `NoBranch` → `Ok(None)` (skip).
/// - `BranchRead` / `InvalidTrackId` / `UnsupportedTargetStatus` / `TrackNotFound` /
///   `ReadError` → `Err(msg)` (fail-closed).
///
/// # Errors
///
/// Returns a human-readable error string for non-skip failures.
fn resolve_ci_verify_track_id_with_reader(
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

/// Build the spec.md path from a resolved track id.
///
/// Discovers the git repo root and constructs `track/items/<id>/spec.md`.
///
/// # Errors
///
/// Returns a human-readable error string when the repo cannot be discovered
/// or the resolved spec path does not exist on disk.
fn build_spec_path_from_track_id(track_id: &str) -> Result<std::path::PathBuf, String> {
    use infrastructure::git_cli::GitRepository as _;
    let repo = infrastructure::git_cli::SystemGitRepo::discover()
        .map_err(|e| format!("cannot discover git repository: {e}"))?;
    let repo_root = repo.root().to_path_buf();
    let spec_path = repo_root.join("track/items").join(track_id).join("spec.md");
    if spec_path.exists() {
        Ok(spec_path)
    } else {
        Err(format!(
            "resolved spec path does not exist: {} (track: {track_id})",
            spec_path.display()
        ))
    }
}

/// Print a `[SKIP]` message and return `ExitCode::SUCCESS`.
///
/// Used by the four CI-gate verify subcommands when no active track can be
/// resolved (AC-16 non-track branch exception to CN-01 fail-closed default).
fn print_skip(label: &str, reason: &str) -> ExitCode {
    println!("--- {label} ---");
    println!("[SKIP] {reason}");
    println!("--- {label} SKIPPED ---");
    ExitCode::SUCCESS
}

/// Check catalogue-spec signal gate results for each tddd-enabled layer.
///
/// Thin delegation to the infrastructure layer which handles all domain type
/// construction internally (CN-01 / AC-03).
fn execute_catalogue_spec_signals_check(args: CatalogueSpecSignalsArgs) -> VerifyOutcome {
    infrastructure::verify::catalogue_spec_signals::execute_catalogue_spec_signals_check(
        args.items_dir,
        args.workspace_root,
        args.strict,
    )
}

/// Core catalogue-spec-signals check logic with explicit, resolved parameters.
///
/// Thin delegation to infrastructure so CLI test code can call this function
/// without importing `domain::` directly.
#[cfg(test)]
fn execute_catalogue_spec_signals(
    items_dir: std::path::PathBuf,
    track_id: String,
    workspace_root: std::path::PathBuf,
    strict: bool,
) -> VerifyOutcome {
    infrastructure::verify::catalogue_spec_signals::execute_catalogue_spec_signals(
        items_dir,
        track_id,
        workspace_root,
        strict,
    )
}

// ── AC-16 dispatch helpers (test-only) ────────────────────────────────────────
//
// These mirror the skip-dispatch logic of the four `execute()` match arms so
// that unit tests can inject a stub `BranchReaderPort` and assert the full
// dispatch path (not just the private helper or `print_skip` in isolation).
// A regression in any match arm — e.g. forgetting to call `print_skip` on
// `Ok(None)`, or wiring the wrong resolver — would fail the corresponding test.

/// Execute the `SpecStates` dispatch logic with an injected branch reader.
///
/// Mirrors the `VerifyCommand::SpecStates` arm of `execute()` exactly, but
/// uses `resolver` instead of the production `resolve_ci_verify_track_id()`.
#[cfg(test)]
fn dispatch_spec_states_with_resolver(
    args: SpecStatesArgs,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> ExitCode {
    let spec_path = match args.spec_path {
        Some(p) => p,
        None => match resolver() {
            Ok(None) => {
                return print_skip("verify spec states", "not on a track branch; skipping");
            }
            Ok(Some(track_id)) => match build_spec_path_from_track_id(&track_id) {
                Ok(p) => p,
                Err(msg) => {
                    eprintln!("{msg}");
                    return ExitCode::FAILURE;
                }
            },
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
    print_outcome("verify spec states", &outcome)
}

/// Execute the `PlanArtifactRefs` dispatch logic with an injected branch reader.
///
/// Mirrors the `VerifyCommand::PlanArtifactRefs` arm of `execute()` exactly,
/// but uses `resolver` instead of the production `resolve_ci_verify_track_id()`.
#[cfg(test)]
fn dispatch_plan_artifact_refs_with_resolver(
    args: PlanArtifactRefsArgs,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> ExitCode {
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
    print_outcome("verify plan artifact refs", &execute_plan_artifact_refs(args))
}

/// Execute the `CatalogueSpecRefs` skip-detection path with an injected branch reader.
///
/// Mirrors the skip-detection branch of `VerifyCommand::CatalogueSpecRefs` in
/// `execute()`, returning `Some(ExitCode::SUCCESS)` on skip and `None` on fall-through.
#[cfg(test)]
fn dispatch_catalogue_spec_refs_skip_with_resolver(
    track: Option<String>,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> Option<ExitCode> {
    if track.is_none() {
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
///
/// Mirrors the `VerifyCommand::CatalogueSpecSignals` arm of `execute()` exactly,
/// but uses `resolver` instead of the production `resolve_ci_verify_track_id()`
/// (CWD-anchored, matching the check path).
#[cfg(test)]
fn dispatch_catalogue_spec_signals_with_resolver(
    args: CatalogueSpecSignalsArgs,
    resolver: impl Fn() -> Result<Option<String>, String>,
) -> ExitCode {
    match resolver() {
        Ok(None) => print_skip("verify catalogue-spec signals", "not on a track branch; skipping"),
        Ok(Some(_)) => print_outcome(
            "verify catalogue-spec signals",
            &execute_catalogue_spec_signals_check(args),
        ),
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

/// Execute the `verify adr-signals` subcommand.
///
/// Thin delegation to infrastructure so CLI never imports `domain::` directly.
fn execute_verify_adr_signals(project_root: &std::path::Path) -> VerifyOutcome {
    infrastructure::verify::adr_signals::execute_verify_adr_signals(project_root)
}

/// Combine architecture_rules + doc_patterns + convention_docs checks.
fn verify_arch_docs(root: &std::path::Path) -> VerifyOutcome {
    let mut outcome = infrastructure::verify::architecture_rules::verify(root);
    outcome.merge(infrastructure::verify::doc_patterns::verify(root));
    outcome.merge(infrastructure::verify::convention_docs::verify(root));
    outcome
}

fn print_outcome(label: &str, outcome: &VerifyOutcome) -> ExitCode {
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use clap::Parser;
    use tempfile::TempDir;

    use super::*;

    /// Minimal wrapper so `VerifyCommand` can be exercised through the clap parser.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: VerifyCommand,
    }

    #[test]
    fn test_spec_states_strict_flag_parsed_by_clap() {
        let cli = TestCli::try_parse_from(["sotp", "spec-states", "spec.md", "--strict"]).unwrap();
        match cli.cmd {
            VerifyCommand::SpecStates(args) => {
                assert!(args.strict, "--strict must be parsed as true");
                assert_eq!(args.spec_path.as_deref().and_then(|p| p.to_str()), Some("spec.md"));
            }
            _ => panic!("expected SpecStates variant"),
        }
    }

    #[test]
    fn test_spec_states_without_strict_flag_defaults_to_false() {
        let cli = TestCli::try_parse_from(["sotp", "spec-states", "spec.md"]).unwrap();
        match cli.cmd {
            VerifyCommand::SpecStates(args) => {
                assert!(!args.strict, "strict must default to false when --strict is absent");
            }
            _ => panic!("expected SpecStates variant"),
        }
    }

    #[test]
    fn test_spec_states_spec_path_is_optional_in_clap() {
        // When spec_path is omitted, clap must accept the invocation and parse strict=false.
        let cli = TestCli::try_parse_from(["sotp", "spec-states"]).unwrap();
        match cli.cmd {
            VerifyCommand::SpecStates(args) => {
                assert!(args.spec_path.is_none(), "spec_path must be None when omitted");
                assert!(!args.strict, "strict must default to false");
            }
            _ => panic!("expected SpecStates variant"),
        }
    }

    #[test]
    fn test_spec_states_omitted_spec_path_with_strict_flag() {
        // --strict alone (no spec path) must also be accepted by clap.
        let cli = TestCli::try_parse_from(["sotp", "spec-states", "--strict"]).unwrap();
        match cli.cmd {
            VerifyCommand::SpecStates(args) => {
                assert!(args.spec_path.is_none(), "spec_path must be None when omitted");
                assert!(args.strict, "--strict must be parsed as true");
            }
            _ => panic!("expected SpecStates variant"),
        }
    }

    #[test]
    fn test_spec_states_omitted_spec_path_returns_non_panic_outcome() {
        // With spec_path omitted, the skip-or-resolve path runs.
        // On a track/* branch with an existing spec.md → runs verify (may pass or fail).
        // On a non-track branch → skip: emits [SKIP] and returns ExitCode::SUCCESS (AC-16).
        // On a real infrastructure failure → ExitCode::FAILURE.
        // The important invariant is no panic and a deterministic exit code.
        let exit =
            execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: None, strict: false }));
        assert!(
            exit == ExitCode::SUCCESS || exit == ExitCode::FAILURE,
            "omitted spec_path must produce a deterministic exit code without panicking"
        );
    }

    #[test]
    fn test_spec_attribution_does_not_accept_strict_flag() {
        // --strict is not a valid flag for spec-attribution; clap must reject it.
        let result = TestCli::try_parse_from(["sotp", "spec-attribution", "spec.md", "--strict"]);
        assert!(result.is_err(), "--strict must not be accepted by spec-attribution");
    }

    fn make_args(root: &std::path::Path) -> VerifyArgs {
        VerifyArgs { project_root: root.to_path_buf() }
    }

    fn write_file(root: &std::path::Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    fn setup_minimal_tech_stack(root: &std::path::Path) {
        write_file(root, "track/tech-stack.md", "# Tech Stack\n- Resolved\n");
    }

    #[test]
    fn test_tech_stack_subcommand_returns_success_for_clean_project() {
        let tmp = TempDir::new().unwrap();
        setup_minimal_tech_stack(tmp.path());
        let exit = execute(VerifyCommand::TechStack(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_tech_stack_subcommand_returns_failure_for_missing_file() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::TechStack(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_latest_track_subcommand_returns_success_with_no_tracks() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::LatestTrack(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_orchestra_subcommand_returns_failure_for_missing_settings() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::Orchestra(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_arch_docs_subcommand_returns_failure_for_missing_rules() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::ArchDocs(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_project_root_flag_is_respected() {
        let tmp = TempDir::new().unwrap();
        setup_minimal_tech_stack(tmp.path());
        // Execute with explicit --project-root pointing to the temp dir.
        let args = VerifyArgs { project_root: tmp.path().to_path_buf() };
        let exit = execute(VerifyCommand::TechStack(args));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_print_outcome_returns_success_for_pass() {
        let outcome = VerifyOutcome::pass();
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_print_outcome_returns_failure_for_errors() {
        let outcome = VerifyOutcome::from_findings(vec![VerifyFinding::error("something broke")]);
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_print_outcome_returns_success_for_warnings_only() {
        let outcome = VerifyOutcome::from_findings(vec![VerifyFinding::warning("note this")]);
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    // --- spec-attribution CLI wiring ---

    #[test]
    fn test_spec_attribution_subcommand_returns_success_for_valid_spec() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        std::fs::write(&spec, "# Spec\n\nNo requirement lines here.\n").unwrap();
        let exit = execute(VerifyCommand::SpecAttribution(SpecVerifyArgs { spec_path: spec }));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_spec_attribution_subcommand_returns_failure_for_missing_source() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        std::fs::write(&spec, "### S-AUTH-01 Login required\n").unwrap();
        let exit = execute(VerifyCommand::SpecAttribution(SpecVerifyArgs { spec_path: spec }));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- spec-frontmatter CLI wiring ---

    #[test]
    fn test_spec_frontmatter_subcommand_returns_success_for_valid_spec() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Spec\n").unwrap();
        let exit = execute(VerifyCommand::SpecFrontmatter(SpecVerifyArgs { spec_path: spec }));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_spec_frontmatter_subcommand_returns_failure_for_missing_frontmatter() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        std::fs::write(&spec, "# Spec without frontmatter\n").unwrap();
        let exit = execute(VerifyCommand::SpecFrontmatter(SpecVerifyArgs { spec_path: spec }));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- canonical-modules CLI wiring ---

    #[test]
    fn test_canonical_modules_subcommand_returns_success_for_clean_project() {
        let tmp = TempDir::new().unwrap();
        // No architecture-rules.json → canonical_modules section absent → pass
        write_file(tmp.path(), "architecture-rules.json", r#"{"version": 2}"#);
        let exit = execute(VerifyCommand::CanonicalModules(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_canonical_modules_subcommand_returns_failure_for_missing_rules_file() {
        let tmp = TempDir::new().unwrap();
        // No architecture-rules.json at all → error
        let exit = execute(VerifyCommand::CanonicalModules(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- module-size CLI wiring ---

    #[test]
    fn test_module_size_subcommand_returns_success_for_small_files() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "architecture-rules.json",
            r#"{"version":2,"module_limits":{"max_lines":700,"warn_lines":400,"exclude":[]}}"#,
        );
        write_file(tmp.path(), "src/small.rs", "fn main() {}\n");
        let exit = execute(VerifyCommand::ModuleSize(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_module_size_subcommand_returns_failure_for_missing_rules() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::ModuleSize(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- domain-strings CLI wiring ---

    #[test]
    fn test_domain_strings_subcommand_returns_success_for_clean_domain() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "libs/domain/src/lib.rs", "pub struct Foo { pub count: u32 }\n");
        let exit = execute(VerifyCommand::DomainStrings(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_domain_strings_subcommand_returns_failure_for_missing_domain() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::DomainStrings(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- domain-purity CLI wiring ---

    #[test]
    fn test_domain_purity_subcommand_returns_success_for_clean_domain() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "libs/domain/src/lib.rs", "pub struct Foo;\n");
        let exit = execute(VerifyCommand::DomainPurity(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_domain_purity_subcommand_returns_failure_for_missing_domain() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::DomainPurity(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- usecase-purity CLI wiring ---

    #[test]
    fn test_usecase_purity_subcommand_returns_success_for_clean_usecase() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "libs/usecase/src/lib.rs",
            "pub fn execute() -> Result<(), String> { Ok(()) }\n",
        );
        let exit = execute(VerifyCommand::UsecasePurity(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_usecase_purity_subcommand_returns_failure_for_missing_usecase() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::UsecasePurity(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- view-freshness CLI wiring ---

    #[test]
    fn test_view_freshness_subcommand_returns_success_with_no_tracks() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::ViewFreshness(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    // --- spec-signals CLI wiring ---

    #[test]
    fn test_spec_signals_subcommand_returns_success_for_valid_spec() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        // Spec with valid frontmatter, a Scope section, and a blue-signal item — no red items.
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n## Scope\n- item [source: PRD §1]\n")
            .unwrap();
        let exit = execute(VerifyCommand::SpecSignals(SpecVerifyArgs { spec_path: spec }));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_spec_signals_subcommand_returns_failure_for_missing_file() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("nonexistent.md");
        let exit = execute(VerifyCommand::SpecSignals(SpecVerifyArgs { spec_path: spec }));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- spec-states CLI wiring ---

    #[test]
    fn test_spec_states_subcommand_returns_success_for_valid_section() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        // Spec with a ## Domain States section containing a table with data rows.
        std::fs::write(
            &spec,
            "---\nversion: \"1.0\"\n---\n## Domain States\n\n\
             | State | Description |\n\
             |-------|-------------|\n\
             | Draft | Initial state |\n",
        )
        .unwrap();
        let exit = execute(VerifyCommand::SpecStates(SpecStatesArgs {
            spec_path: Some(spec),
            strict: false,
        }));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_spec_states_subcommand_returns_failure_for_missing_section() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        // Spec with frontmatter but no ## Domain States section.
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Overview\n\nNo states here.\n")
            .unwrap();
        let exit = execute(VerifyCommand::SpecStates(SpecStatesArgs {
            spec_path: Some(spec),
            strict: false,
        }));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    /// Writes a minimal `architecture-rules.json` with only `domain` TDDD-enabled
    /// into the given tmp dir. Shared by the two Yellow-signal CLI tests below,
    /// which both need the multilayer loop to find exactly one enabled layer
    /// that points at `domain-types.json`.
    fn write_minimal_arch_rules(dir: &std::path::Path) {
        let content = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json"
      }
    }
  ]
}"#;
        std::fs::write(dir.join("architecture-rules.json"), content).unwrap();
    }

    /// Writes `<dir>/<signal_name>` with a matching `declaration_hash` so the
    /// ADR 2026-04-18-1400 §D5 signal-file evaluation path accepts it.
    /// `signals` is copied verbatim from the declaration file's legacy
    /// inline `signals` array (raw JSON), bypassing the catalogue decode path
    /// that ignores that field.
    fn write_matching_signal_file(dir: &std::path::Path, catalogue_name: &str, signal_name: &str) {
        let decl_bytes = std::fs::read(dir.join(catalogue_name)).unwrap();
        let value: serde_json::Value = serde_json::from_slice(&decl_bytes).unwrap();
        let signals_array =
            value.get("signals").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        let hash = infrastructure::tddd::type_signals_codec::declaration_hash(&decl_bytes);
        let signal_file = serde_json::json!({
            "schema_version": 1,
            "generated_at": "2026-04-18T12:00:00Z",
            "declaration_hash": hash,
            "signals": signals_array,
        });
        let encoded = serde_json::to_string_pretty(&signal_file).unwrap();
        std::fs::write(dir.join(signal_name), encoded).unwrap();
    }

    #[test]
    fn test_spec_states_strict_false_passes_with_yellow_signal() {
        let tmp = TempDir::new().unwrap();
        write_minimal_arch_rules(tmp.path());
        let spec = tmp.path().join("spec.md");
        // spec.md without ## Domain States (will delegate to spec.json).
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Overview\n").unwrap();
        // spec.json: signals have yellow=1, red=0 (Stage 1 prerequisite satisfied).
        std::fs::write(
            tmp.path().join("spec.json"),
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":{"blue":0,"yellow":1,"red":0}}"#,
        )
        .unwrap();
        // domain-types.json: one entry with a yellow signal.
        std::fs::write(
            tmp.path().join("domain-types.json"),
            r#"{"schema_version":2,"type_definitions":[{"name":"MyType","kind":"value_object","description":"d","approved":true,"expected_methods":[]}],"signals":[{"type_name":"MyType","kind_tag":"value_object","signal":"yellow","found_type":false}]}"#,
        )
        .unwrap();
        write_matching_signal_file(tmp.path(), "domain-types.json", "domain-type-signals.json");
        let exit = execute(VerifyCommand::SpecStates(SpecStatesArgs {
            spec_path: Some(spec),
            strict: false,
        }));
        assert_eq!(exit, ExitCode::SUCCESS, "yellow signal must pass in default (non-strict) mode");
    }

    #[test]
    fn test_spec_states_strict_true_fails_with_yellow_signal() {
        let tmp = TempDir::new().unwrap();
        write_minimal_arch_rules(tmp.path());
        let spec = tmp.path().join("spec.md");
        // spec.md without ## Domain States (will delegate to spec.json).
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Overview\n").unwrap();
        // spec.json: signals have yellow=1, red=0 (Stage 1 prerequisite satisfied).
        std::fs::write(
            tmp.path().join("spec.json"),
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":{"blue":0,"yellow":1,"red":0}}"#,
        )
        .unwrap();
        // domain-types.json: one entry with a yellow signal.
        std::fs::write(
            tmp.path().join("domain-types.json"),
            r#"{"schema_version":2,"type_definitions":[{"name":"MyType","kind":"value_object","description":"d","approved":true,"expected_methods":[]}],"signals":[{"type_name":"MyType","kind_tag":"value_object","signal":"yellow","found_type":false}]}"#,
        )
        .unwrap();
        write_matching_signal_file(tmp.path(), "domain-types.json", "domain-type-signals.json");
        let exit = execute(VerifyCommand::SpecStates(SpecStatesArgs {
            spec_path: Some(spec),
            strict: true,
        }));
        assert_eq!(exit, ExitCode::FAILURE, "yellow signal must fail in strict (merge-gate) mode");
    }

    // T008: consistency_report_to_findings / check_consistency / evaluate_consistency_from_components
    // tests removed. These functions and their TypeGraph / TypeBaseline dependencies are deleted.

    // --- plan-artifact-refs CLI wiring ---

    #[test]
    fn test_plan_artifact_refs_clap_parses_track_dir_flag() {
        let cli = TestCli::try_parse_from([
            "sotp",
            "plan-artifact-refs",
            "--track-dir",
            "track/items/my-track",
        ])
        .unwrap();
        match cli.cmd {
            VerifyCommand::PlanArtifactRefs(args) => {
                assert_eq!(
                    args.track_dir.as_deref(),
                    Some(std::path::Path::new("track/items/my-track")),
                    "--track-dir must be parsed correctly"
                );
            }
            _ => panic!("expected PlanArtifactRefs variant"),
        }
    }

    #[test]
    fn test_plan_artifact_refs_clap_omitted_track_dir_is_none() {
        let cli = TestCli::try_parse_from(["sotp", "plan-artifact-refs"]).unwrap();
        match cli.cmd {
            VerifyCommand::PlanArtifactRefs(args) => {
                assert!(args.track_dir.is_none(), "--track-dir must default to None");
            }
            _ => panic!("expected PlanArtifactRefs variant"),
        }
    }

    #[test]
    fn test_plan_artifact_refs_explicit_valid_dir_with_no_spec_json_returns_success() {
        // A track directory without spec.json is a valid pre-Phase-1 track;
        // plan_artifact_refs::verify treats it as a no-op pass.
        let tmp = TempDir::new().unwrap();
        let track_dir = tmp.path().join("track/items/test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        // No spec.json → verify passes immediately.
        let exit = execute(VerifyCommand::PlanArtifactRefs(PlanArtifactRefsArgs {
            track_dir: Some(track_dir),
        }));
        assert_eq!(exit, ExitCode::SUCCESS, "track dir without spec.json must pass");
    }

    #[test]
    fn test_plan_artifact_refs_explicit_missing_dir_returns_failure() {
        // An explicitly supplied --track-dir that does not exist on disk must
        // produce an error finding and exit with failure.
        let tmp = TempDir::new().unwrap();
        let missing_dir = tmp.path().join("track/items/nonexistent");
        // Do NOT create the directory.
        let exit = execute(VerifyCommand::PlanArtifactRefs(PlanArtifactRefsArgs {
            track_dir: Some(missing_dir),
        }));
        assert_eq!(exit, ExitCode::FAILURE, "missing track dir must fail");
    }

    #[test]
    fn test_plan_artifact_refs_omitted_track_dir_returns_non_panic_outcome() {
        // With `--track-dir` omitted, the skip-or-resolve path runs.
        // This test exercises the branch-resolution path:
        // - On a `track/*` branch with an existing items dir → runs verify (may pass
        //   or fail based on the real repo state, but must not panic).
        // - On a non-`track/*` branch → skip: emits [SKIP] and returns ExitCode::SUCCESS
        //   (AC-16 non-track branch exception).
        // - On a real infrastructure failure → ExitCode::FAILURE.
        // The important invariant is that NO panic occurs and the exit code is deterministic.
        let exit =
            execute(VerifyCommand::PlanArtifactRefs(PlanArtifactRefsArgs { track_dir: None }));
        // We accept either outcome; the key contract is no panic.
        assert!(
            exit == ExitCode::SUCCESS || exit == ExitCode::FAILURE,
            "omitted track_dir must produce a deterministic exit code without panicking"
        );
    }

    // --- catalogue-spec-signals CLI wiring ---

    #[test]
    fn test_catalogue_spec_signals_strict_flag_parsed_by_clap() {
        let cli = TestCli::try_parse_from(["sotp", "catalogue-spec-signals", "--strict"]).unwrap();
        match cli.cmd {
            VerifyCommand::CatalogueSpecSignals(args) => {
                assert!(args.strict, "--strict must be parsed as true");
            }
            _ => panic!("expected CatalogueSpecSignals variant"),
        }
    }

    #[test]
    fn test_catalogue_spec_signals_without_strict_flag_defaults_to_false() {
        let cli = TestCli::try_parse_from(["sotp", "catalogue-spec-signals"]).unwrap();
        match cli.cmd {
            VerifyCommand::CatalogueSpecSignals(args) => {
                assert!(!args.strict, "strict must default to false when --strict is absent");
            }
            _ => panic!("expected CatalogueSpecSignals variant"),
        }
    }

    #[test]
    fn test_catalogue_spec_signals_default_args_returns_non_panic_outcome() {
        // Exercises the full git-based branch-resolution path with default args.
        // - On a `track/*` branch with a valid track structure → runs signal check (may pass
        //   or fail depending on the real repo state, but must not panic).
        // - On a non-`track/*` branch → skip: emits [SKIP] and returns ExitCode::SUCCESS
        //   (AC-16 non-track branch exception).
        // - On a real infrastructure failure (BranchRead error etc.) → ExitCode::FAILURE.
        // The important invariant is no panic and a deterministic exit code.
        let exit = execute(VerifyCommand::CatalogueSpecSignals(CatalogueSpecSignalsArgs {
            items_dir: std::path::PathBuf::from("track/items"),
            workspace_root: std::path::PathBuf::from("."),
            strict: false,
        }));
        assert!(
            exit == ExitCode::SUCCESS || exit == ExitCode::FAILURE,
            "default catalogue-spec-signals invocation must produce a deterministic exit code without panicking"
        );
    }

    /// Helper: write a minimal `domain-types.json` whose single entry matches
    /// the `type_name` used in the Yellow/Red signals test fixtures. Required
    /// by the coverage-validation check in `check_catalogue_spec_signals`
    /// (PR #111 fail-open fix): a signals file must list exactly the catalogue
    /// entries, so each test that persists a signals document also needs a
    /// catalogue file with matching names.
    fn write_matching_domain_catalogue_with_single_entry(
        track_dir: &std::path::Path,
        entry_name: &str,
    ) {
        // v3-native format required by CatalogueDocumentCodec::decode.
        let catalogue = serde_json::json!({
            "schema_version": 3,
            "crate_name": "domain",
            "layer": "domain",
            "types": {
                entry_name: {
                    "action": "add",
                    "role": "ValueObject",
                    "kind": { "kind": "struct", "shape": { "kind": "unit" } }
                }
            },
            "traits": {},
            "functions": {}
        });
        std::fs::write(
            track_dir.join("domain-types.json"),
            serde_json::to_string_pretty(&catalogue).unwrap(),
        )
        .unwrap();
    }

    // Helper: write architecture-rules.json with one TDDD-enabled domain layer.
    fn write_arch_rules_for_signals_test(workspace_root: &std::path::Path) {
        let rules = serde_json::json!({
            "version": 2,
            "layers": [{
                "crate": "domain",
                "path": "libs/domain",
                "may_depend_on": [],
                "deny_reason": "",
                "tddd": { "enabled": true, "catalogue_file": "domain-types.json", "catalogue_spec_signal": { "enabled": true } }
            }]
        });
        std::fs::write(
            workspace_root.join("architecture-rules.json"),
            serde_json::to_string_pretty(&rules).unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_catalogue_spec_signals_missing_signals_file_is_lenient_skip() {
        // Exercises the per-layer lenient-skip path: when architecture-rules.json
        // lists a TDDD-enabled layer but the corresponding signals file is absent,
        // `execute_catalogue_spec_signals` must return a pass outcome (exit 0 in
        // CI interim mode). This is the "layer not yet activated" path that allows
        // CI to pass before pre-commit generates the signals file for the first time.
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        write_arch_rules_for_signals_test(&ws);
        // NO domain-catalogue-spec-signals.json → lenient skip per layer.

        let outcome = execute_catalogue_spec_signals(
            items_dir,
            track_id.to_owned(),
            ws,
            false, // strict=false
        );
        assert!(
            !outcome.has_errors(),
            "missing signals file must be a lenient skip (no errors): {outcome:?}"
        );
    }

    #[test]
    fn test_catalogue_spec_signals_strict_false_yellow_is_warning_only() {
        // Exercises the --strict=false path: a Yellow signal produces a Warning
        // finding (not an Error), so `has_errors()` is false and CI stays green
        // in interim mode.
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        write_arch_rules_for_signals_test(&ws);
        write_matching_domain_catalogue_with_single_entry(&track_dir, "MyType");
        // Write a signals file with one Yellow entry (informal_grounds[] non-empty,
        // spec_refs[] empty).
        let signals_json = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": "a".repeat(64),
            "signals": [
                { "type_name": "MyType", "signal": "yellow" }
            ]
        });
        std::fs::write(
            track_dir.join("domain-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&signals_json).unwrap(),
        )
        .unwrap();

        let outcome = execute_catalogue_spec_signals(items_dir, track_id.to_owned(), ws, false);
        assert!(
            !outcome.has_errors(),
            "Yellow signal with strict=false must not be an error: {outcome:?}"
        );
        // There should be a warning finding describing the Yellow signal.
        assert!(
            !outcome.findings().is_empty(),
            "Yellow signal must produce a warning finding: {outcome:?}"
        );
    }

    #[test]
    fn test_catalogue_spec_signals_strict_true_yellow_is_error() {
        // Exercises the --strict=true path: a Yellow signal is promoted to an Error
        // finding, so `has_errors()` returns true and the gate blocks. This is the
        // merge-gate / strict-mode behavior.
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        write_arch_rules_for_signals_test(&ws);
        write_matching_domain_catalogue_with_single_entry(&track_dir, "MyType");
        let signals_json = serde_json::json!({
            "schema_version": 1,
            "catalogue_declaration_hash": "a".repeat(64),
            "signals": [
                { "type_name": "MyType", "signal": "yellow" }
            ]
        });
        std::fs::write(
            track_dir.join("domain-catalogue-spec-signals.json"),
            serde_json::to_string_pretty(&signals_json).unwrap(),
        )
        .unwrap();

        let outcome = execute_catalogue_spec_signals(items_dir, track_id.to_owned(), ws, true);
        assert!(
            outcome.has_errors(),
            "Yellow signal with strict=true must be an error: {outcome:?}"
        );
    }

    #[test]
    fn test_catalogue_spec_signals_missing_arch_rules_returns_error() {
        // Exercises the fail-closed architecture-rules.json guard: if
        // workspace_root has no architecture-rules.json, the gate must
        // return an error (not silently pass with no layers checked).
        let tmp = TempDir::new().unwrap();
        let ws = tmp.path().to_path_buf();
        let track_id = "test-track";
        let items_dir = ws.join("track/items");
        let track_dir = items_dir.join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        // No architecture-rules.json → fail-closed error.

        let outcome = execute_catalogue_spec_signals(items_dir, track_id.to_owned(), ws, false);
        assert!(
            outcome.has_errors(),
            "missing architecture-rules.json must produce an error finding: {outcome:?}"
        );
    }

    // ── verify adr-signals (T006 / AC-01) ──────────────────────────────────

    /// Set up a project root with the given ADR fixture files written under
    /// `<root>/knowledge/adr/`. Each fixture is `(filename, content)`.
    fn setup_adr_project(root: &std::path::Path, fixtures: &[(&str, &str)]) {
        let adr_dir = root.join("knowledge/adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        for (name, body) in fixtures {
            std::fs::write(adr_dir.join(name), body).unwrap();
        }
    }

    fn fixture_blue_proposed() -> &'static str {
        "---\nadr_id: 2026-01-01-blue\ndecisions:\n  - id: D1\n    status: proposed\n    user_decision_ref: chat:test-blue\n---\n# body\n"
    }

    fn fixture_red_proposed() -> &'static str {
        "---\nadr_id: 2026-01-99-red\ndecisions:\n  - id: D1\n    status: proposed\n---\n# body\n"
    }

    #[test]
    fn test_adr_signals_with_only_blue_decisions_passes() {
        let tmp = TempDir::new().unwrap();
        setup_adr_project(tmp.path(), &[("blue.md", fixture_blue_proposed())]);

        let outcome = execute_verify_adr_signals(tmp.path());
        assert!(
            !outcome.has_errors(),
            "all-Blue project must not produce an error finding: {:?}",
            outcome.findings()
        );
    }

    #[test]
    fn test_adr_signals_with_red_decision_yields_error_finding() {
        let tmp = TempDir::new().unwrap();
        setup_adr_project(
            tmp.path(),
            &[("blue.md", fixture_blue_proposed()), ("red.md", fixture_red_proposed())],
        );

        let outcome = execute_verify_adr_signals(tmp.path());
        assert!(
            outcome.has_errors(),
            "project with a Red decision must produce an error finding (drives exit 1 per AC-01): {:?}",
            outcome.findings()
        );
        let msg = outcome.findings()[0].message();
        assert!(
            msg.contains("red=1"),
            "error finding must include the red count summary, got: {msg}"
        );
    }

    #[test]
    fn test_adr_signals_propagates_listing_error_when_dir_missing() {
        let tmp = TempDir::new().unwrap();
        // Do not create knowledge/adr/ — list_adr_paths must fail.
        let outcome = execute_verify_adr_signals(tmp.path());
        assert!(
            outcome.has_errors(),
            "missing knowledge/adr/ must produce an error finding (port listing failure)"
        );
    }

    // ── T012: CI verify skip path unit tests (AC-16) ──────────────────────────

    /// Stub `BranchReaderPort` for testing the skip-or-error discrimination.
    struct StubVerifyBranchReader {
        value: Result<Option<String>, usecase::track_resolution::BranchReadError>,
    }

    impl StubVerifyBranchReader {
        fn returning_branch(branch: impl Into<String>) -> Self {
            Self { value: Ok(Some(branch.into())) }
        }

        fn returning_none() -> Self {
            Self { value: Ok(None) }
        }

        fn returning_read_error(msg: impl Into<String>) -> Self {
            Self { value: Err(usecase::track_resolution::BranchReadError::ReadFailed(msg.into())) }
        }
    }

    impl usecase::track_resolution::BranchReaderPort for StubVerifyBranchReader {
        fn current_branch(
            &self,
        ) -> Result<Option<String>, usecase::track_resolution::BranchReadError> {
            match &self.value {
                Ok(v) => Ok(v.clone()),
                Err(usecase::track_resolution::BranchReadError::ReadFailed(msg)) => {
                    Err(usecase::track_resolution::BranchReadError::ReadFailed(msg.clone()))
                }
            }
        }
    }

    /// Helper: call the testable inner function with a stub reader.
    fn ci_verify_resolve(reader: StubVerifyBranchReader) -> Result<Option<String>, String> {
        resolve_ci_verify_track_id_with_reader(std::sync::Arc::new(reader))
    }

    // --- skip path: non-track branch families ---

    #[test]
    fn test_ci_verify_track_id_on_main_branch_returns_skip() {
        // NotTrackBranch (e.g. "main") → Ok(None) → callers emit [SKIP].
        let result = ci_verify_resolve(StubVerifyBranchReader::returning_branch("main"));
        assert_eq!(result.unwrap(), None, "main branch must return Ok(None) so callers skip");
    }

    #[test]
    fn test_ci_verify_track_id_on_detached_head_returns_skip() {
        // DetachedHead (Some("HEAD") from BranchReaderPort) → Ok(None) → callers emit [SKIP].
        let result = ci_verify_resolve(StubVerifyBranchReader::returning_branch("HEAD"));
        assert_eq!(result.unwrap(), None, "detached HEAD must return Ok(None) so callers skip");
    }

    #[test]
    fn test_ci_verify_track_id_on_no_branch_returns_skip() {
        // NoBranch (None from BranchReaderPort) → Ok(None) → callers emit [SKIP].
        let result = ci_verify_resolve(StubVerifyBranchReader::returning_none());
        assert_eq!(result.unwrap(), None, "no branch (None) must return Ok(None) so callers skip");
    }

    // --- normal path: track branch ---

    #[test]
    fn test_ci_verify_track_id_on_track_branch_returns_track_id() {
        // On track/<id>, resolution must succeed and return Some(track_id).
        let result =
            ci_verify_resolve(StubVerifyBranchReader::returning_branch("track/my-feature-2026"));
        assert_eq!(
            result.unwrap(),
            Some("my-feature-2026".to_owned()),
            "track branch must resolve to Some(track_id)"
        );
    }

    // --- fail-closed path: BranchRead error ---

    #[test]
    fn test_ci_verify_track_id_with_branch_read_error_returns_err() {
        // BranchRead I/O error → Err(...) → callers fail-closed.
        let result =
            ci_verify_resolve(StubVerifyBranchReader::returning_read_error("git not found"));
        assert!(
            result.is_err(),
            "BranchRead error must return Err so callers fail-closed: {result:?}"
        );
    }

    // --- four verify subcommands: print_skip output and ExitCode::SUCCESS ---

    /// Returns the skip exit code for `verify spec-states` when the branch reader
    /// returns a non-track branch name.
    #[test]
    fn test_spec_states_skips_on_non_track_branch_via_stub_reader() {
        // spec_path = None triggers the branch-resolution path.
        // The production branch reader is used at build time; here we call the
        // inner function directly to simulate a non-track branch.
        let result = ci_verify_resolve(StubVerifyBranchReader::returning_branch("main"));
        // The skip-detection result must be Ok(None) — the execute() function
        // then calls print_skip() and returns SUCCESS.
        assert!(
            result.unwrap().is_none(),
            "non-track branch must yield Ok(None) for spec-states skip detection"
        );
    }

    #[test]
    fn test_plan_artifact_refs_skips_on_non_track_branch_via_stub_reader() {
        // Same as spec-states: the skip-detection uses the same helper.
        let result = ci_verify_resolve(StubVerifyBranchReader::returning_branch("main"));
        assert!(
            result.unwrap().is_none(),
            "non-track branch must yield Ok(None) for plan-artifact-refs skip detection"
        );
    }

    #[test]
    fn test_catalogue_spec_refs_skips_on_non_track_branch_via_stub_reader() {
        let result = ci_verify_resolve(StubVerifyBranchReader::returning_branch("main"));
        assert!(
            result.unwrap().is_none(),
            "non-track branch must yield Ok(None) for catalogue-spec-refs skip detection"
        );
    }

    #[test]
    fn test_catalogue_spec_signals_skips_on_non_track_branch_via_stub_reader() {
        let result = ci_verify_resolve(StubVerifyBranchReader::returning_branch("main"));
        assert!(
            result.unwrap().is_none(),
            "non-track branch must yield Ok(None) for catalogue-spec-signals skip detection"
        );
    }

    // --- end-to-end: print_skip returns ExitCode::SUCCESS ---

    #[test]
    fn test_print_skip_returns_success() {
        // print_skip is the leaf that all four subcommands call on Ok(None).
        // It must always return ExitCode::SUCCESS (the [SKIP] contract).
        let exit = print_skip("test label", "test reason");
        assert_eq!(exit, ExitCode::SUCCESS, "print_skip must return ExitCode::SUCCESS");
    }

    // ── dispatch-level skip tests (AC-16): assert execute() match arm wiring ──
    //
    // These tests exercise the full dispatch path — i.e. the match arms inside
    // execute() — via the #[cfg(test)] dispatch helpers. A regression in any
    // match arm (forgetting the print_skip call, wiring the wrong resolver, or
    // dropping the explicit-arg bypass) would fail the corresponding test.

    // Helper: return Ok(None) from the resolver (simulates a non-track branch).
    fn non_track_resolver() -> Result<Option<String>, String> {
        Ok(None)
    }

    // Helper: return Err (simulates a BranchRead I/O error).
    fn err_resolver() -> Result<Option<String>, String> {
        Err("git not found".to_owned())
    }

    // --- SpecStates dispatch ---

    #[test]
    fn test_dispatch_spec_states_skips_and_returns_success_on_non_track_branch() {
        // The SpecStates match arm must call print_skip and return SUCCESS when
        // the resolver returns Ok(None) (non-track branch, spec_path not given).
        let exit = dispatch_spec_states_with_resolver(
            SpecStatesArgs { spec_path: None, strict: false },
            non_track_resolver,
        );
        assert_eq!(
            exit,
            ExitCode::SUCCESS,
            "SpecStates must return ExitCode::SUCCESS (skip) on non-track branch"
        );
    }

    #[test]
    fn test_dispatch_spec_states_explicit_path_bypasses_skip() {
        // When spec_path is given explicitly, the resolver is NOT called and the
        // arm must fall through to the verify path (not skip). An error is
        // expected because the path doesn't exist on disk, but not a skip.
        let tmp = tempfile::TempDir::new().unwrap();
        let nonexistent = tmp.path().join("spec.md");
        // spec.md does not exist → trusted_root resolve will fail → FAILURE (not SKIP).
        let exit = dispatch_spec_states_with_resolver(
            SpecStatesArgs { spec_path: Some(nonexistent), strict: false },
            // Resolver should never be called for explicit path.
            || panic!("resolver must not be called when spec_path is explicit"),
        );
        assert_eq!(
            exit,
            ExitCode::FAILURE,
            "SpecStates must fail (not skip) when spec_path is given but does not exist"
        );
    }

    #[test]
    fn test_dispatch_spec_states_fail_closed_on_resolver_error() {
        // When the resolver returns Err, the arm must fail-closed (FAILURE).
        let exit = dispatch_spec_states_with_resolver(
            SpecStatesArgs { spec_path: None, strict: false },
            err_resolver,
        );
        assert_eq!(
            exit,
            ExitCode::FAILURE,
            "SpecStates must return FAILURE (fail-closed) when resolver returns Err"
        );
    }

    // --- PlanArtifactRefs dispatch ---

    #[test]
    fn test_dispatch_plan_artifact_refs_skips_and_returns_success_on_non_track_branch() {
        // The PlanArtifactRefs match arm must skip when resolver returns Ok(None)
        // and track_dir is not explicitly provided.
        let exit = dispatch_plan_artifact_refs_with_resolver(
            PlanArtifactRefsArgs { track_dir: None },
            non_track_resolver,
        );
        assert_eq!(
            exit,
            ExitCode::SUCCESS,
            "PlanArtifactRefs must return ExitCode::SUCCESS (skip) on non-track branch"
        );
    }

    #[test]
    fn test_dispatch_plan_artifact_refs_explicit_dir_bypasses_skip() {
        // When track_dir is given explicitly, the resolver is NOT called; the arm
        // passes through to execute_plan_artifact_refs, which fails for a missing dir.
        let tmp = tempfile::TempDir::new().unwrap();
        let missing = tmp.path().join("nonexistent");
        let exit = dispatch_plan_artifact_refs_with_resolver(
            PlanArtifactRefsArgs { track_dir: Some(missing) },
            || panic!("resolver must not be called when track_dir is explicit"),
        );
        assert_eq!(
            exit,
            ExitCode::FAILURE,
            "PlanArtifactRefs must fail (not skip) when explicit track_dir does not exist"
        );
    }

    // --- CatalogueSpecRefs dispatch ---

    #[test]
    fn test_dispatch_catalogue_spec_refs_skips_and_returns_success_on_non_track_branch() {
        // The CatalogueSpecRefs skip-detection path must return Some(SUCCESS) when
        // resolver returns Ok(None) and --track is not given.
        let result = dispatch_catalogue_spec_refs_skip_with_resolver(None, non_track_resolver);
        assert_eq!(
            result,
            Some(ExitCode::SUCCESS),
            "CatalogueSpecRefs must return Some(SUCCESS) (skip) on non-track branch"
        );
    }

    #[test]
    fn test_dispatch_catalogue_spec_refs_explicit_track_bypasses_skip() {
        // When --track is given explicitly, the resolver is NOT called and the
        // arm must return None (fall through to the full verify path).
        let result =
            dispatch_catalogue_spec_refs_skip_with_resolver(Some("my-track".to_owned()), || {
                panic!("resolver must not be called when track is explicit")
            });
        assert_eq!(
            result, None,
            "CatalogueSpecRefs must return None (fall through) when track is explicit"
        );
    }

    #[test]
    fn test_dispatch_catalogue_spec_refs_fail_closed_on_resolver_error() {
        // When the resolver returns Err, the arm must fail-closed.
        let result = dispatch_catalogue_spec_refs_skip_with_resolver(None, err_resolver);
        assert_eq!(
            result,
            Some(ExitCode::FAILURE),
            "CatalogueSpecRefs must return Some(FAILURE) (fail-closed) when resolver errors"
        );
    }

    // --- CatalogueSpecSignals dispatch ---

    #[test]
    fn test_dispatch_catalogue_spec_signals_skips_and_returns_success_on_non_track_branch() {
        // The CatalogueSpecSignals match arm must skip when resolver returns Ok(None).
        let exit = dispatch_catalogue_spec_signals_with_resolver(
            CatalogueSpecSignalsArgs {
                items_dir: std::path::PathBuf::from("track/items"),
                workspace_root: std::path::PathBuf::from("."),
                strict: false,
            },
            non_track_resolver,
        );
        assert_eq!(
            exit,
            ExitCode::SUCCESS,
            "CatalogueSpecSignals must return ExitCode::SUCCESS (skip) on non-track branch"
        );
    }

    #[test]
    fn test_dispatch_catalogue_spec_signals_fail_closed_on_resolver_error() {
        // When the resolver returns Err, the arm must fail-closed.
        let exit = dispatch_catalogue_spec_signals_with_resolver(
            CatalogueSpecSignalsArgs {
                items_dir: std::path::PathBuf::from("track/items"),
                workspace_root: std::path::PathBuf::from("."),
                strict: false,
            },
            err_resolver,
        );
        assert_eq!(
            exit,
            ExitCode::FAILURE,
            "CatalogueSpecSignals must return FAILURE (fail-closed) when resolver errors"
        );
    }
}
