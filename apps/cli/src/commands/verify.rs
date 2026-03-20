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
    /// Check libs/domain/src/ for pub String fields (should be enums or newtypes).
    DomainStrings(VerifyArgs),
    /// Check that plan.md files are up-to-date with metadata.json renderings.
    ViewFreshness(VerifyArgs),
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
        VerifyCommand::DomainStrings(args) => (
            "verify domain strings",
            infrastructure::verify::domain_strings::verify(&args.project_root),
        ),
        VerifyCommand::ViewFreshness(args) => (
            "verify view freshness",
            infrastructure::verify::view_freshness::verify(&args.project_root),
        ),
    };

    print_outcome(label, &outcome)
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
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

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
        write_file(tmp.path(), "docs/architecture-rules.json", r#"{"version": 2}"#);
        let exit = execute(VerifyCommand::CanonicalModules(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }

    #[test]
    fn test_canonical_modules_subcommand_returns_failure_for_missing_rules_file() {
        let tmp = TempDir::new().unwrap();
        // No docs/architecture-rules.json at all → error
        let exit = execute(VerifyCommand::CanonicalModules(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::FAILURE);
    }

    // --- module-size CLI wiring ---

    #[test]
    fn test_module_size_subcommand_returns_success_for_small_files() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "docs/architecture-rules.json",
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

    // --- view-freshness CLI wiring ---

    #[test]
    fn test_view_freshness_subcommand_returns_success_with_no_tracks() {
        let tmp = TempDir::new().unwrap();
        let exit = execute(VerifyCommand::ViewFreshness(make_args(tmp.path())));
        assert_eq!(exit, ExitCode::SUCCESS);
    }
}
