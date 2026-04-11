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
}

/// Arguments for spec-coverage verify subcommand.
#[derive(Args)]
pub struct SpecCoverageArgs {
    /// Path to the track directory (e.g., track/items/<id>).
    /// If not provided, the command is a no-op (pass).
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
        VerifyCommand::SpecStates(args) => (
            "verify spec states",
            infrastructure::verify::spec_states::verify(&args.spec_path, args.strict),
        ),
        VerifyCommand::SpecCoverage(args) => {
            let outcome = match &args.track_dir {
                Some(dir) if dir.is_dir() => infrastructure::verify::spec_coverage::verify(dir),
                Some(dir) => VerifyOutcome::from_findings(vec![domain::verify::Finding::error(
                    format!("Track directory does not exist: {}", dir.display()),
                )]),
                None => VerifyOutcome::pass(),
            };
            ("verify spec coverage", outcome)
        }
        VerifyCommand::SpecCodeConsistency(args) => {
            ("verify spec-code consistency", execute_spec_code_consistency(args))
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
            return VerifyOutcome::from_findings(vec![domain::verify::Finding::error(format!(
                "invalid track ID: {e}"
            ))]);
        }
    };

    let track_dir = args.project_root.join("track/items").join(track_id.as_ref());
    let domain_types_path = track_dir.join("domain-types.json");

    // Read and decode domain-types.json.
    let json = match std::fs::read_to_string(&domain_types_path) {
        Ok(s) => s,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![domain::verify::Finding::error(format!(
                "cannot read {}: {e}",
                domain_types_path.display()
            ))]);
        }
    };

    let doc = match infrastructure::tddd::catalogue_codec::decode(&json) {
        Ok(d) => d,
        Err(e) => {
            return VerifyOutcome::from_findings(vec![domain::verify::Finding::error(format!(
                "domain-types.json decode error: {e}"
            ))]);
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
            return VerifyOutcome::from_findings(vec![domain::verify::Finding::error(format!(
                "schema export failed: {e}{hint}"
            ))]);
        }
    };

    // Collect typestate names and build TypeGraph.
    let typestate_names: std::collections::HashSet<String> = doc
        .entries()
        .iter()
        .filter(|e| matches!(e.kind(), domain::DomainTypeKind::Typestate { .. }))
        .map(|e| e.name().to_string())
        .collect();
    let graph = infrastructure::code_profile_builder::build_type_graph(&schema, &typestate_names);

    // Load baseline for 4-group evaluation.
    let baseline_path = track_dir.join("domain-types-baseline.json");
    let baseline = match std::fs::read_to_string(&baseline_path) {
        Ok(bl_json) => match infrastructure::tddd::baseline_codec::decode(&bl_json) {
            Ok(bl) => bl,
            Err(e) => {
                return VerifyOutcome::from_findings(vec![domain::verify::Finding::error(
                    format!("baseline decode error: {e}"),
                )]);
            }
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return VerifyOutcome::from_findings(vec![domain::verify::Finding::error(format!(
                "domain-types-baseline.json not found — run `sotp track baseline-capture {}`",
                args.track_id
            ))]);
        }
        Err(e) => {
            return VerifyOutcome::from_findings(vec![domain::verify::Finding::error(format!(
                "cannot read {}: {e}",
                baseline_path.display()
            ))]);
        }
    };

    // Run bidirectional consistency check with baseline-aware 4-group evaluation.
    let report = domain::check_consistency(doc.entries(), &graph, &baseline);

    // Convert to VerifyOutcome.
    let mut findings = Vec::new();

    // Forward: red signals become errors.
    for sig in report.forward_signals() {
        if sig.signal() == domain::ConfidenceSignal::Red {
            findings.push(domain::verify::Finding::error(format!(
                "{} ({}): Red — missing={:?}, extra={:?}",
                sig.type_name(),
                sig.kind_tag(),
                sig.missing_items(),
                sig.extra_items(),
            )));
        }
    }

    // Group 4: undeclared types/traits (not in baseline, not declared) → Red.
    for name in report.undeclared_types() {
        findings.push(domain::verify::Finding::error(format!(
            "undeclared new type in code: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.undeclared_traits() {
        findings.push(domain::verify::Finding::error(format!(
            "undeclared new trait in code: `{name}` — add to domain-types.json"
        )));
    }

    // Group 3: baseline structural changes or deletions → Red.
    for name in report.baseline_red_types() {
        findings.push(domain::verify::Finding::error(format!(
            "undeclared structural change to baseline type: `{name}` — add to domain-types.json"
        )));
    }
    for name in report.baseline_red_traits() {
        findings.push(domain::verify::Finding::error(format!(
            "undeclared structural change to baseline trait: `{name}` — add to domain-types.json"
        )));
    }

    print_consistency_report_json(&report);

    if findings.is_empty() { VerifyOutcome::pass() } else { VerifyOutcome::from_findings(findings) }
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
    let output = serde_json::json!({
        "forward_signals": forward,
        "undeclared_types": report.undeclared_types(),
        "undeclared_traits": report.undeclared_traits(),
        "skipped_count": report.skipped_count(),
        "baseline_red_types": report.baseline_red_types(),
        "baseline_red_traits": report.baseline_red_traits(),
    });
    println!("{output}");
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
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
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
        let outcome =
            VerifyOutcome::from_findings(vec![domain::verify::Finding::error("something broke")]);
        let exit = print_outcome("test", &outcome);
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_print_outcome_returns_success_for_warnings_only() {
        let outcome =
            VerifyOutcome::from_findings(vec![domain::verify::Finding::warning("note this")]);
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
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n---\n# Spec\n").unwrap();
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
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Scope\n- item [source: PRD §1]\n",
        )
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
            "---\nstatus: draft\nversion: \"1.0\"\n---\n## Domain States\n\n\
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
        std::fs::write(
            &spec,
            "---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n\nNo states here.\n",
        )
        .unwrap();
        let exit =
            execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: spec, strict: false }));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    #[test]
    fn test_spec_states_strict_false_passes_with_yellow_signal() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        // spec.md without ## Domain States (will delegate to spec.json).
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n").unwrap();
        // spec.json: signals have yellow=1, red=0 (Stage 1 prerequisite satisfied).
        std::fs::write(
            tmp.path().join("spec.json"),
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":{"blue":0,"yellow":1,"red":0}}"#,
        )
        .unwrap();
        // domain-types.json: one entry with a yellow signal.
        std::fs::write(
            tmp.path().join("domain-types.json"),
            r#"{"schema_version":1,"domain_types":[{"name":"MyType","kind":"value_object","description":"d","approved":true}],"signals":[{"type_name":"MyType","kind_tag":"value_object","signal":"yellow","found_type":false}]}"#,
        )
        .unwrap();
        let exit =
            execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: spec, strict: false }));
        assert_eq!(exit, ExitCode::SUCCESS, "yellow signal must pass in default (non-strict) mode");
    }

    #[test]
    fn test_spec_states_strict_true_fails_with_yellow_signal() {
        let tmp = TempDir::new().unwrap();
        let spec = tmp.path().join("spec.md");
        // spec.md without ## Domain States (will delegate to spec.json).
        std::fs::write(&spec, "---\nstatus: draft\nversion: \"1.0\"\n---\n# Overview\n").unwrap();
        // spec.json: signals have yellow=1, red=0 (Stage 1 prerequisite satisfied).
        std::fs::write(
            tmp.path().join("spec.json"),
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":{"blue":0,"yellow":1,"red":0}}"#,
        )
        .unwrap();
        // domain-types.json: one entry with a yellow signal.
        std::fs::write(
            tmp.path().join("domain-types.json"),
            r#"{"schema_version":1,"domain_types":[{"name":"MyType","kind":"value_object","description":"d","approved":true}],"signals":[{"type_name":"MyType","kind_tag":"value_object","signal":"yellow","found_type":false}]}"#,
        )
        .unwrap();
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
        write_file(
            tmp.path(),
            "track/items/test-track/spec.json",
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"item","sources":["PRD"],"task_refs":["T001"]}],"out_of_scope":[]}}"#,
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
        write_file(
            tmp.path(),
            "track/items/test-track/spec.json",
            r#"{"schema_version":1,"status":"draft","version":"1.0","title":"T","scope":{"in_scope":[{"text":"uncovered","sources":["PRD"]}],"out_of_scope":[]}}"#,
        );
        let exit = execute(VerifyCommand::SpecCoverage(SpecCoverageArgs { track_dir: Some(dir) }));
        assert_eq!(exit, ExitCode::FAILURE);
    }
}
