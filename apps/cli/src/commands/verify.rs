//! `sotp verify` subcommand group.
//!
//! Each subcommand delegates to the corresponding infrastructure verify module
//! and prints the outcome. Exits 0 on pass, 1 on failure.

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use domain::verify::VerifyOutcome;

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
    spec_path: PathBuf,
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
    /// Check requirement-to-task coverage for a track (resolved from branch or --track-dir).
    SpecCoverage(SpecCoverageArgs),
    /// Check bidirectional spec ↔ code consistency (domain-types.json vs rustdoc TypeGraph).
    SpecCodeConsistency(SpecCodeConsistencyArgs),
    /// Validate structured-ref fields (adr_refs, convention_refs, spec_refs, informal_grounds)
    /// introduced in T002 / T003 / T005 per ADR 2026-04-19-1242 §D2.3.
    PlanArtifactRefs(PlanArtifactRefsArgs),
}

/// Arguments for spec-coverage verify subcommand.
#[derive(Args)]
pub struct SpecCoverageArgs {
    /// Path to the track directory (e.g., track/items/<id>).
    /// If not provided, the command is a no-op (pass).
    #[arg(long)]
    track_dir: Option<PathBuf>,
}

/// Arguments for plan-artifact-refs verify subcommand.
#[derive(Args)]
pub struct PlanArtifactRefsArgs {
    /// Path to the track directory (e.g., track/items/<id>).
    /// When omitted, the active track is resolved from the current branch name.
    #[arg(long)]
    track_dir: Option<PathBuf>,
}

/// Arguments for spec-code-consistency verify subcommand.
#[derive(Args)]
pub struct SpecCodeConsistencyArgs {
    /// Track ID (e.g., `spec-code-consistency-2026-04-08`).
    #[arg(long)]
    track_id: String,
    /// Crate name to export schema from (e.g., `domain`).
    #[arg(long = "crate", default_value = "domain")]
    crate_name: String,
    /// Project root directory.
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
}

/// Common arguments for all verify subcommands.
#[derive(Args)]
pub struct VerifyArgs {
    /// Project root directory (defaults to current directory).
    #[arg(long, default_value = ".")]
    project_root: PathBuf,
}

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
            // Resolve trusted_root via the infrastructure-layer helper.
            // All filesystem I/O (git discover, .git walk-up, symlink
            // verification) lives in `infrastructure::verify::trusted_root`;
            // the CLI is a pure composition root that maps Result → finding.
            let outcome =
                match infrastructure::verify::trusted_root::resolve_trusted_root(&args.spec_path) {
                    Ok(trusted_root) => infrastructure::verify::spec_states::verify(
                        &args.spec_path,
                        args.strict,
                        &trusted_root,
                    ),
                    Err(e) => {
                        VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                            format!(
                                "cannot resolve trusted_root for {}: {e}",
                                args.spec_path.display()
                            ),
                        )])
                    }
                };
            ("verify spec states", outcome)
        }
        VerifyCommand::SpecCoverage(args) => {
            let outcome = match &args.track_dir {
                Some(dir) if dir.is_dir() => infrastructure::verify::spec_coverage::verify(dir),
                Some(dir) => {
                    VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                        format!("Track directory does not exist: {}", dir.display()),
                    )])
                }
                None => VerifyOutcome::pass(),
            };
            ("verify spec coverage", outcome)
        }
        VerifyCommand::SpecCodeConsistency(args) => {
            ("verify spec-code consistency", execute_spec_code_consistency(args))
        }
        VerifyCommand::PlanArtifactRefs(args) => {
            ("verify plan artifact refs", execute_plan_artifact_refs(args))
        }
    };

    print_outcome(label, &outcome)
}

/// Execute bidirectional spec ↔ code consistency check.
#[allow(clippy::too_many_lines)]
fn execute_spec_code_consistency(args: SpecCodeConsistencyArgs) -> VerifyOutcome {
    use domain::schema::{SchemaExportError, SchemaExporter};

    // Validate track ID.
    let track_id = match domain::TrackId::try_new(&args.track_id) {
        Ok(id) => id,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                format!("invalid track ID: {e}"),
            )]);
        }
    };

    let track_dir = args.project_root.join("track/items").join(track_id.as_ref());
    let domain_types_path = track_dir.join("domain-types.json");

    // Read and decode domain-types.json.
    let json = match std::fs::read_to_string(&domain_types_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                format!("cannot read {}: {e}", domain_types_path.display()),
            )]);
        }
    };

    let doc = match infrastructure::tddd::catalogue_codec::decode(&json) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                format!("domain-types.json decode error: {e}"),
            )]);
        }
    };

    // Export schema via rustdoc JSON.
    let exporter =
        infrastructure::schema_export::RustdocSchemaExporter::new(args.project_root.clone());
    let schema = match exporter.export(&args.crate_name) {
        Ok(s) => s,
        Err(e) => {
            let hint = if matches!(e, SchemaExportError::NightlyNotFound) {
                " (install with: rustup toolchain install nightly)"
            } else {
                ""
            };
            return VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                format!("schema export failed: {e}{hint}"),
            )]);
        }
    };

    // Collect typestate names and build TypeGraph.
    let typestate_names = doc.typestate_names();
    let graph = infrastructure::code_profile_builder::build_type_graph(&schema, &typestate_names);

    // Load baseline for 4-group evaluation.
    let baseline_path = track_dir.join("domain-types-baseline.json");
    let baseline = match std::fs::read_to_string(&baseline_path) {
        Ok(bl_json) => match infrastructure::tddd::baseline_codec::decode(&bl_json) {
            Ok(bl) => bl,
            Err(e) => {
                return VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                    format!("baseline decode error: {e}"),
                )]);
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                format!(
                    "domain-types-baseline.json not found — run `sotp track baseline-capture {}`",
                    args.track_id
                ),
            )]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                format!("cannot read {}: {e}", baseline_path.display()),
            )]);
        }
    };

    // Run bidirectional consistency check with baseline-aware 4-group evaluation.
    evaluate_consistency_from_components(&doc, &graph, &baseline)
}

/// Core spec-code consistency evaluation given pre-built domain components.
///
/// Separated from `execute_spec_code_consistency` so the wiring from
/// `check_consistency` → `consistency_report_to_findings` → `VerifyOutcome` can be
/// exercised in unit tests without requiring the nightly toolchain.
///
/// # Arguments
/// * `doc` — decoded `TypeCatalogueDocument` (entries read from `domain-types.json`)
/// * `graph` — `TypeGraph` built from the schema export
/// * `baseline` — decoded `TypeBaseline` from `domain-types-baseline.json`
fn evaluate_consistency_from_components(
    doc: &domain::TypeCatalogueDocument,
    graph: &domain::TypeGraph,
    baseline: &domain::TypeBaseline,
) -> VerifyOutcome {
    let report = domain::check_consistency(doc.entries(), graph, baseline);
    let findings = consistency_report_to_findings(&report);
    print_consistency_report_json(&report);
    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
}

/// Convert a `ConsistencyReport` into a flat list of `VerifyFinding`s for `VerifyOutcome`.
///
/// The domain layer patches an invalid `action=delete` (no baseline match) entry into a
/// Red forward signal AND stores the name in `delete_errors()`.  To avoid duplicate
/// findings for the same root cause, this function identifies the synthetic Red via its
/// fingerprint (`found_type==false`, empty `missing_items`, empty `extra_items`) and
/// suppresses it in favour of the more specific `delete_errors` message.  This is safe
/// for same-name delete+add pairs because the `add` half has a different kind/found_type
/// state and will not match the fingerprint.
///
/// # Returns
///
/// A `Vec<VerifyFinding>` with errors for Red signals, errors for undeclared/baseline-red
/// items, warnings for contradictions, and errors for invalid delete declarations.
fn consistency_report_to_findings(
    report: &domain::ConsistencyReport,
) -> Vec<domain::verify::VerifyFinding> {
    let mut findings = Vec::new();

    // Collect delete_error names upfront so their patched forward signals can be
    // suppressed.  The fingerprint (name in delete_errors + found_type==false +
    // empty missing + empty extra) identifies only the synthetic patch produced by
    // the domain layer's delete-baseline validation — not the paired add entry.
    let delete_error_names: std::collections::HashSet<&str> =
        report.delete_errors().iter().map(String::as_str).collect();

    // Forward: red signals become errors (excluding synthetic delete-error patches).
    for sig in report.forward_signals() {
        if sig.signal() == domain::ConfidenceSignal::Red {
            let is_delete_error_signal = delete_error_names.contains(sig.type_name())
                && !sig.found_type()
                && sig.missing_items().is_empty()
                && sig.extra_items().is_empty();
            if !is_delete_error_signal {
                findings.push(domain::verify::VerifyFinding::error(format!(
                    "{} ({}): Red — missing={:?}, extra={:?}",
                    sig.type_name(),
                    sig.kind_tag(),
                    sig.missing_items(),
                    sig.extra_items(),
                )));
            }
        }
    }

    // Group 4: undeclared types/traits (not in baseline, not declared) → Red.
    for name in report.undeclared_types() {
        findings.push(domain::verify::VerifyFinding::error(format!(
            "undeclared new type in code: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.undeclared_traits() {
        findings.push(domain::verify::VerifyFinding::error(format!(
            "undeclared new trait in code: `{name}` — add to domain-types.json"
        )));
    }

    // Group 3: baseline structural changes or deletions → Red.
    for name in report.baseline_red_types() {
        findings.push(domain::verify::VerifyFinding::error(format!(
            "undeclared structural change to baseline type: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.baseline_red_traits() {
        findings.push(domain::verify::VerifyFinding::error(format!(
            "undeclared structural change to baseline trait: `{name}` — add to domain-types.json"
        )));
    }

    // Action-baseline contradictions → warnings (advisory, not CI-blocking).
    for contradiction in report.contradictions() {
        findings.push(domain::verify::VerifyFinding::warning(format!(
            "{} (action={}): {:?}",
            contradiction.name(),
            contradiction.action().action_tag(),
            contradiction.kind(),
        )));
    }

    // Delete baseline validation errors → hard errors (CI-blocking, specific diagnostic).
    // These replace the suppressed generic Red forward signal for the same entry.
    for name in report.delete_errors() {
        findings.push(domain::verify::VerifyFinding::error(format!(
            "action=delete for `{name}` but type not in baseline — cannot delete non-existent type"
        )));
    }

    findings
}

/// Serialize a `ConsistencyReport` as a JSON line to stdout via serde_json.
fn print_consistency_report_json(report: &domain::ConsistencyReport) {
    let signal_str = |s: domain::ConfidenceSignal| match s {
        domain::ConfidenceSignal::Blue => "blue",
        domain::ConfidenceSignal::Yellow => "yellow",
        domain::ConfidenceSignal::Red => "red",
        _ => "unknown",
    };
    let forward: Vec<serde_json::Value> = report
        .forward_signals()
        .iter()
        .map(|s| {
            serde_json::json!({
                "type_name": s.type_name(),
                "kind_tag": s.kind_tag(),
                "signal": signal_str(s.signal()),
                "found_type": s.found_type(),
                "found_items": s.found_items(),
                "missing_items": s.missing_items(),
                "extra_items": s.extra_items(),
            })
        })
        .collect();
    let contradictions: Vec<serde_json::Value> = report
        .contradictions()
        .iter()
        .map(|c| {
            serde_json::json!({
                "name": c.name(),
                "action": c.action().action_tag(),
                "kind": format!("{:?}", c.kind()),
            })
        })
        .collect();
    let output = serde_json::json!({
        "forward_signals": forward,
        "undeclared_types": report.undeclared_types(),
        "undeclared_traits": report.undeclared_traits(),
        "skipped_count": report.skipped_count(),
        "baseline_red_types": report.baseline_red_types(),
        "baseline_red_traits": report.baseline_red_traits(),
        "contradictions": contradictions,
        "delete_errors": report.delete_errors(),
    });
    println!("{output}");
}

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
        Some(dir) => VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
            format!("Track directory does not exist: {}", dir.display()),
        )]),
        None => match resolve_active_track_dir() {
            Some(dir) => infrastructure::verify::plan_artifact_refs::verify(&dir),
            None => VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
                "Cannot resolve active track directory: not on a track/* branch or directory does \
                 not exist. Use --track-dir <PATH> to specify the track directory explicitly."
                    .to_owned(),
            )]),
        },
    }
}

/// Resolve the active track directory from the current git branch name.
///
/// Accepts both `track/<id>` and `plan/<id>` branches (the canonical
/// `resolve_track_or_plan_id_from_branch` helper is reused so the branch
/// detection logic stays in one place). The returned path is anchored to the
/// repo root discovered via `SystemGitRepo::discover`, so this function works
/// correctly regardless of which subdirectory the process is invoked from.
///
/// Returns `None` when:
/// - Not inside a git repository
/// - Not on a `track/*` or `plan/*` branch (including detached HEAD / main)
/// - The resolved `track/items/<id>` directory does not exist on disk
fn resolve_active_track_dir() -> Option<std::path::PathBuf> {
    use infrastructure::git_cli::GitRepository as _;
    let repo = infrastructure::git_cli::SystemGitRepo::discover().ok()?;
    let branch = repo.current_branch().ok()??;
    let track_id =
        usecase::track_resolution::resolve_track_or_plan_id_from_branch(Some(&branch)).ok()?;
    let track_dir = repo.root().join("track/items").join(&track_id);
    if track_dir.is_dir() { Some(track_dir) } else { None }
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
                assert_eq!(args.spec_path.to_str().unwrap(), "spec.md");
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
        let outcome = VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::error(
            "something broke",
        )]);
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_print_outcome_returns_success_for_warnings_only() {
        let outcome =
            VerifyOutcome::from_findings(vec![domain::verify::VerifyFinding::warning("note this")]);
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
        let exit =
            execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: spec, strict: false }));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_spec_states_subcommand_returns_failure_for_missing_section() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        // Spec with frontmatter but no ## Domain States section.
        std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Overview\n\nNo states here.\n")
            .unwrap();
        let exit =
            execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: spec, strict: false }));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    /// Writes a minimal `architecture-rules.json` with only `domain` TDDD-enabled
    /// into the given tmp dir. Shared by the two Yellow-signal CLI tests below,
    /// which both need the new T007 multilayer loop to find exactly one enabled
    /// layer that points at `domain-types.json`.
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
    /// inline `signals` array (raw JSON). This bypasses `catalogue_codec`
    /// which, after T007, drops inline signals during decode.
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
            r#"{"schema_version":2,"type_definitions":[{"name":"MyType","kind":"value_object","description":"d","approved":true}],"signals":[{"type_name":"MyType","kind_tag":"value_object","signal":"yellow","found_type":false}]}"#,
        )
        .unwrap();
        write_matching_signal_file(tmp.path(), "domain-types.json", "domain-type-signals.json");
        let exit =
            execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: spec, strict: false }));
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
            r#"{"schema_version":2,"type_definitions":[{"name":"MyType","kind":"value_object","description":"d","approved":true}],"signals":[{"type_name":"MyType","kind_tag":"value_object","signal":"yellow","found_type":false}]}"#,
        )
        .unwrap();
        write_matching_signal_file(tmp.path(), "domain-types.json", "domain-type-signals.json");
        let exit =
            execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: spec, strict: true }));
        assert_eq!(exit, ExitCode::FAILURE, "yellow signal must fail in strict (merge-gate) mode");
    }

    // --- spec-coverage CLI wiring ---

    #[test]
    fn test_spec_coverage_subcommand_returns_success_with_no_track_dir() {
        let exit = execute(VerifyCommand::SpecCoverage(SpecCoverageArgs { track_dir: None }));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_spec_coverage_subcommand_returns_success_with_covered_track() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("track/items/test-track");
        std::fs::create_dir_all(&dir).unwrap();
        write_file(
            tmp.path(),
            "track/items/test-track/metadata.json",
            r#"{"schema_version":3,"id":"test-track","title":"T","status":"in_progress","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch":"track/test-track","tasks":[{"id":"T001","description":"task","status":"todo"}],"plan":{"summary":[],"sections":[{"id":"S1","title":"S","description":[],"task_ids":["T001"]}]}}"#,
        );
        // schema v2: no task_refs / sources; empty in_scope so evaluate_coverage stub returns pass.
        write_file(
            tmp.path(),
            "track/items/test-track/spec.json",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]}}"#,
        );
        let exit = execute(VerifyCommand::SpecCoverage(SpecCoverageArgs { track_dir: Some(dir) }));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_spec_coverage_subcommand_returns_failure_for_uncovered() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("track/items/test-track");
        std::fs::create_dir_all(&dir).unwrap();
        write_file(
            tmp.path(),
            "track/items/test-track/metadata.json",
            r#"{"schema_version":3,"id":"test-track","title":"T","status":"in_progress","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch":"track/test-track","tasks":[{"id":"T001","description":"task","status":"todo"}],"plan":{"summary":[],"sections":[{"id":"S1","title":"S","description":[],"task_ids":["T001"]}]}}"#,
        );
        // schema v2: in_scope item with no task_refs → evaluate_coverage stub reports uncovered → fail.
        write_file(
            tmp.path(),
            "track/items/test-track/spec.json",
            r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[{"id":"IN-01","text":"uncovered"}],"out_of_scope":[]}}"#,
        );
        // task-coverage.json must exist to trigger coverage check (T003 transition guard).
        write_file(tmp.path(), "track/items/test-track/task-coverage.json", "{}");
        let exit = execute(VerifyCommand::SpecCoverage(SpecCoverageArgs { track_dir: Some(dir) }));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- consistency_report_to_findings tests ---

    fn make_entry_for_test(name: &str, action: domain::TypeAction) -> domain::TypeCatalogueEntry {
        domain::TypeCatalogueEntry::new(
            name,
            "desc",
            domain::TypeDefinitionKind::ValueObject,
            action,
            true,
        )
        .unwrap()
    }

    fn empty_graph_for_test() -> domain::TypeGraph {
        domain::TypeGraph::new(std::collections::HashMap::new(), std::collections::HashMap::new())
    }

    fn empty_baseline_for_test() -> domain::TypeBaseline {
        domain::TypeBaseline::new(
            1,
            domain::Timestamp::new("2026-01-01T00:00:00Z").unwrap(),
            std::collections::HashMap::new(),
            std::collections::HashMap::new(),
        )
    }

    #[test]
    fn test_consistency_report_to_findings_with_delete_not_in_baseline_produces_single_error() {
        // action=delete for a type not in baseline → delete_errors fires.
        // The domain layer also patches the forward signal to Red.
        // consistency_report_to_findings must emit exactly ONE finding (the specific
        // delete diagnostic) and suppress the duplicate generic Red finding.
        let entry = make_entry_for_test("Ghost", domain::TypeAction::Delete);
        let report = domain::check_consistency(
            &[entry],
            &empty_graph_for_test(),
            &empty_baseline_for_test(),
        );

        assert!(!report.delete_errors().is_empty(), "delete_errors must be non-empty");

        let findings = consistency_report_to_findings(&report);
        // Exactly one finding: the specific delete_errors message (no duplicate generic Red).
        assert_eq!(findings.len(), 1, "expected exactly 1 finding, got: {findings:?}");
        let msg = findings[0].message();
        assert!(
            msg.contains("action=delete") && msg.contains("Ghost"),
            "finding must be the specific delete diagnostic, got: {msg}"
        );
        assert_eq!(
            findings[0].severity(),
            domain::verify::Severity::Error,
            "delete error must be reported as error severity"
        );
    }

    #[test]
    fn test_consistency_report_to_findings_with_empty_report_produces_no_findings() {
        // No entries at all — empty report must produce no findings.
        let report =
            domain::check_consistency(&[], &empty_graph_for_test(), &empty_baseline_for_test());
        let findings = consistency_report_to_findings(&report);
        assert!(findings.is_empty(), "clean report must produce no findings: {findings:?}");
    }

    #[test]
    fn test_consistency_report_to_findings_with_add_in_baseline_produces_warning() {
        // action=add (default) when type is already in baseline → contradiction warning, not error.
        // This tests that contradiction findings are emitted as warnings (advisory).
        use std::collections::HashMap;
        let entry = make_entry_for_test("Existing", domain::TypeAction::Add);
        let baseline = domain::TypeBaseline::new(
            1,
            domain::Timestamp::new("2026-01-01T00:00:00Z").unwrap(),
            HashMap::from([(
                "Existing".to_string(),
                domain::TypeBaselineEntry::new(domain::schema::TypeKind::Struct, vec![], vec![]),
            )]),
            HashMap::new(),
        );
        let report = domain::check_consistency(&[entry], &empty_graph_for_test(), &baseline);

        let findings = consistency_report_to_findings(&report);
        // Contradiction should produce exactly one warning finding.
        let warnings: Vec<_> =
            findings.iter().filter(|f| f.severity() == domain::verify::Severity::Warning).collect();
        assert!(!warnings.is_empty(), "contradiction must produce at least one warning");
        // Must not produce any errors for a contradiction-only report.
        let errors: Vec<_> =
            findings.iter().filter(|f| f.severity() == domain::verify::Severity::Error).collect();
        assert!(errors.is_empty(), "contradiction must not produce errors: {errors:?}");
    }

    #[test]
    fn test_contradiction_only_report_produces_exit_success_via_print_outcome() {
        // End-to-end contract test: contradiction warnings must not fail the CI gate.
        // This bridges consistency_report_to_findings (warning) → VerifyOutcome → print_outcome
        // → ExitCode::SUCCESS, proving the full chain executed by SpecCodeConsistency.
        use std::collections::HashMap;
        let entry = make_entry_for_test("Existing", domain::TypeAction::Add);
        let baseline = domain::TypeBaseline::new(
            1,
            domain::Timestamp::new("2026-01-01T00:00:00Z").unwrap(),
            HashMap::from([(
                "Existing".to_string(),
                domain::TypeBaselineEntry::new(domain::schema::TypeKind::Struct, vec![], vec![]),
            )]),
            HashMap::new(),
        );
        let report = domain::check_consistency(&[entry], &empty_graph_for_test(), &baseline);
        assert!(
            !report.contradictions().is_empty(),
            "precondition: must have at least one contradiction"
        );

        let findings = consistency_report_to_findings(&report);
        let outcome = if findings.is_empty() {
            domain::verify::VerifyOutcome::pass()
        } else {
            domain::verify::VerifyOutcome::from_findings(findings)
        };
        // Contradiction-only report must NOT fail the CI gate (exit code 0).
        let exit = print_outcome("test", &outcome);
        assert_eq!(
            exit,
            ExitCode::SUCCESS,
            "contradiction-only report must exit 0 (advisory, not CI-blocking)"
        );
    }

    #[test]
    fn test_delete_error_report_produces_exit_failure_via_print_outcome() {
        // End-to-end contract test: delete errors must fail the CI gate.
        // This bridges consistency_report_to_findings (error) → VerifyOutcome → print_outcome
        // → ExitCode::FAILURE, proving the full chain executed by SpecCodeConsistency.
        let entry = make_entry_for_test("Ghost", domain::TypeAction::Delete);
        let report = domain::check_consistency(
            &[entry],
            &empty_graph_for_test(),
            &empty_baseline_for_test(),
        );
        assert!(!report.delete_errors().is_empty(), "precondition: must have delete errors");

        let findings = consistency_report_to_findings(&report);
        let outcome = domain::verify::VerifyOutcome::from_findings(findings);
        // Delete errors must fail the CI gate (exit code 1).
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::FAILURE, "delete error report must exit 1 (CI-blocking)");
    }

    // --- evaluate_consistency_from_components tests (core CLI wiring, no nightly needed) ---

    fn make_doc_with_entry(entry: domain::TypeCatalogueEntry) -> domain::TypeCatalogueDocument {
        domain::TypeCatalogueDocument::new(1, vec![entry])
    }

    fn empty_doc_for_test() -> domain::TypeCatalogueDocument {
        domain::TypeCatalogueDocument::new(1, vec![])
    }

    #[test]
    fn test_evaluate_consistency_from_components_with_delete_error_returns_failure_outcome() {
        // Proves the wiring inside execute_spec_code_consistency: delete errors must
        // produce a VerifyOutcome with Error severity findings (which exits 1 via print_outcome).
        let entry = make_entry_for_test("Ghost", domain::TypeAction::Delete);
        let doc = make_doc_with_entry(entry);
        let outcome = evaluate_consistency_from_components(
            &doc,
            &empty_graph_for_test(),
            &empty_baseline_for_test(),
        );
        // Must have error-severity findings so the CLI exits 1.
        let has_errors =
            outcome.findings().iter().any(|f| f.severity() == domain::verify::Severity::Error);
        assert!(has_errors, "delete error must produce error-severity findings in VerifyOutcome");
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::FAILURE, "delete error must cause exit 1");
    }

    #[test]
    fn test_evaluate_consistency_from_components_with_contradiction_only_returns_success_outcome() {
        // Proves the wiring inside execute_spec_code_consistency: contradiction warnings
        // must produce a VerifyOutcome that exits 0 (advisory, not CI-blocking).
        use std::collections::HashMap;
        let entry = make_entry_for_test("Existing", domain::TypeAction::Add);
        let baseline = domain::TypeBaseline::new(
            1,
            domain::Timestamp::new("2026-01-01T00:00:00Z").unwrap(),
            HashMap::from([(
                "Existing".to_string(),
                domain::TypeBaselineEntry::new(domain::schema::TypeKind::Struct, vec![], vec![]),
            )]),
            HashMap::new(),
        );
        let doc = make_doc_with_entry(entry);
        let outcome =
            evaluate_consistency_from_components(&doc, &empty_graph_for_test(), &baseline);
        // Must have only warning-severity findings (no errors) — exit code 0.
        let has_errors =
            outcome.findings().iter().any(|f| f.severity() == domain::verify::Severity::Error);
        assert!(!has_errors, "contradiction must not produce error-severity findings");
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::SUCCESS, "contradiction-only must exit 0");
    }

    #[test]
    fn test_evaluate_consistency_from_components_with_empty_report_returns_pass_outcome() {
        // Empty report (no entries, no errors) must produce VerifyOutcome::pass() → exit 0.
        let outcome = evaluate_consistency_from_components(
            &empty_doc_for_test(),
            &empty_graph_for_test(),
            &empty_baseline_for_test(),
        );
        assert!(outcome.findings().is_empty(), "empty report must produce zero findings");
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::SUCCESS, "empty report must exit 0");
    }

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
        // With `--track-dir` omitted, `resolve_active_track_dir` runs.
        // This test exercises the branch-resolution path:
        // - On a `track/*` or `plan/*` branch with an existing items dir → runs verify (may pass
        //   or fail based on the real repo state, but must not panic).
        // - On any other branch / git failure → returns an error finding with ExitCode::FAILURE.
        // The important invariant is that NO panic occurs and the exit code is deterministic.
        let exit =
            execute(VerifyCommand::PlanArtifactRefs(PlanArtifactRefsArgs { track_dir: None }));
        // We accept either outcome; the key contract is no panic.
        assert!(
            exit == ExitCode::SUCCESS || exit == ExitCode::FAILURE,
            "omitted track_dir must produce a deterministic exit code without panicking"
        );
    }
}
