//! Tests for [`verify`] (split out to keep `verify.rs` under the 700-line guideline).

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
    // CommandOutcome::success maps to ExitCode::SUCCESS (exit_code=0).
    let outcome =
        cli_composition::CommandOutcome::success(Some("[OK] All checks passed.".to_owned()));
    let exit = print_outcome(&outcome);
    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn test_print_outcome_returns_failure_for_errors() {
    // CommandOutcome with exit_code=1 maps to ExitCode::FAILURE.
    let outcome = cli_composition::CommandOutcome::failure(Some("something broke".to_owned()));
    let exit = print_outcome(&outcome);
    assert_eq!(exit, ExitCode::FAILURE);
}

#[test]
fn test_print_outcome_returns_success_for_warnings_only() {
    // A CommandOutcome with exit_code=0 but non-empty stdout maps to SUCCESS.
    let outcome = cli_composition::CommandOutcome::success(Some("[WARN] note this".to_owned()));
    let exit = print_outcome(&outcome);
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
        execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: Some(spec), strict: false }));
    assert_eq!(exit, ExitCode::SUCCESS);
}

#[test]
fn test_spec_states_subcommand_returns_failure_for_missing_section() {
    let tmp = TempDir::new().unwrap();
    let spec = tmp.path().join("spec.md");
    // Spec with frontmatter but no ## Domain States section.
    std::fs::write(&spec, "---\nversion: \"1.0\"\n---\n# Overview\n\nNo states here.\n").unwrap();
    let exit =
        execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: Some(spec), strict: false }));
    assert_eq!(exit, ExitCode::FAILURE);
}

/// Writes a minimal `architecture-rules.json` with only `domain` TDDD-enabled
/// into the given tmp dir. Shared by the two Yellow-signal CLI tests below,
/// which both need the multilayer loop to find exactly one enabled layer
/// that points at `domain-types.json`.
fn write_minimal_arch_rules(dir: &std::path::Path) {
    std::fs::create_dir_all(dir.join(".git")).unwrap();
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
    let exit =
        execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: Some(spec), strict: false }));
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
    let exit =
        execute(VerifyCommand::SpecStates(SpecStatesArgs { spec_path: Some(spec), strict: true }));
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
    let exit = execute(VerifyCommand::PlanArtifactRefs(PlanArtifactRefsArgs { track_dir: None }));
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
    // v4-native format required by CatalogueDocumentCodec::decode.
    let catalogue = serde_json::json!({
        "schema_version": 4,
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
            { "type_name": "MyType", "signal": "yellow", "entry_hash": "a".repeat(64) }
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
            { "type_name": "MyType", "signal": "yellow", "entry_hash": "a".repeat(64) }
        ]
    });
    std::fs::write(
        track_dir.join("domain-catalogue-spec-signals.json"),
        serde_json::to_string_pretty(&signals_json).unwrap(),
    )
    .unwrap();

    let outcome = execute_catalogue_spec_signals(items_dir, track_id.to_owned(), ws, true);
    assert!(outcome.has_errors(), "Yellow signal with strict=true must be an error: {outcome:?}");
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
    assert!(msg.contains("red=1"), "error finding must include the red count summary, got: {msg}");
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
    fn current_branch(&self) -> Result<Option<String>, usecase::track_resolution::BranchReadError> {
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
    let result = ci_verify_resolve(StubVerifyBranchReader::returning_read_error("git not found"));
    assert!(result.is_err(), "BranchRead error must return Err so callers fail-closed: {result:?}");
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
// These tests exercise the skip-detection dispatch path via the #[cfg(test)]
// dispatch helpers. A regression in skip detection would fail the corresponding test.

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
    // When --track-id is given explicitly, the resolver is NOT called and the
    // arm must return None (fall through to the full verify path).
    let result =
        dispatch_catalogue_spec_refs_skip_with_resolver(Some("my-track".to_owned()), || {
            panic!("resolver must not be called when track_id is explicit")
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

// ── spec-states skip-gate: missing spec artifacts (D1 / T001) ──────────────
//
// These tests exercise the new spec-artifact-absent skip path in
// `dispatch_spec_states_with_resolver_and_repo_root`. The repo-root reader is
// injected so fixtures live in an isolated tempdir, not in the real checkout.
fn dispatch_spec_states_with_track_fixture(
    repo_root: &std::path::Path,
    track_id: &str,
    strict: bool,
) -> ExitCode {
    let id = track_id.to_owned();
    let root = repo_root.to_path_buf();
    dispatch_spec_states_with_resolver_and_repo_root(
        SpecStatesArgs { spec_path: None, strict },
        move || Ok(Some(id.clone())),
        move || Ok(root.clone()),
    )
}

fn create_temp_track_dir(repo_root: &std::path::Path, track_id: &str) -> std::path::PathBuf {
    let path = repo_root.join("track/items").join(track_id);
    std::fs::create_dir_all(&path).unwrap();
    path
}

// (a) track-resolution path + BOTH spec artifacts absent → exit 0 + SKIP output.
#[test]
fn test_dispatch_spec_states_skips_when_both_spec_artifacts_absent_on_track_branch() {
    let tmp = tempfile::TempDir::new().unwrap();
    let track_id = "test-skip-gate-no-spec-artifacts-t001a";
    let _track_dir = create_temp_track_dir(tmp.path(), track_id);
    // The track directory now exists but has neither spec.json nor spec.md.
    let exit = dispatch_spec_states_with_track_fixture(tmp.path(), track_id, false);
    assert_eq!(
        exit,
        ExitCode::SUCCESS,
        "spec-states must return SUCCESS (skip) when neither spec.json nor spec.md exists"
    );
}

// (b) track-resolution path + `spec.md` present → evaluates normally (delegates to infra verify).
#[test]
fn test_dispatch_spec_states_evaluates_when_spec_md_is_present() {
    let tmp = tempfile::TempDir::new().unwrap();
    let track_id = "test-skip-gate-spec-md-present-t001b";
    let track_dir = create_temp_track_dir(tmp.path(), track_id);
    // Write an intentionally invalid spec.md. A real verification run fails;
    // an accidental skip would return SUCCESS.
    std::fs::write(track_dir.join("spec.md"), "# Spec Without Domain States\n").unwrap();
    // spec.json is absent — only spec.md present.
    assert!(!track_dir.join("spec.json").exists());
    let exit = dispatch_spec_states_with_track_fixture(tmp.path(), track_id, false);
    assert_eq!(
        exit,
        ExitCode::FAILURE,
        "spec-states must evaluate (not skip) when spec.md is present"
    );
}

// (c) track-resolution path + `spec.json` present, `spec.md` absent → evaluates normally (NOT skipped).
#[test]
fn test_dispatch_spec_states_evaluates_when_only_spec_json_is_present() {
    let tmp = tempfile::TempDir::new().unwrap();
    let track_id = "test-skip-gate-spec-json-only-t001c";
    let track_dir = create_temp_track_dir(tmp.path(), track_id);
    // Write a minimal spec.json (the infrastructure layer reads this if spec.md is absent).
    std::fs::write(
        track_dir.join("spec.json"),
        r#"{"schema_version":2,"version":"1.0","title":"T","scope":{"in_scope":[],"out_of_scope":[]},"signals":{"blue":0,"yellow":0,"red":0}}"#,
    )
    .unwrap();
    // spec.md is absent — only spec.json present.
    assert!(!track_dir.join("spec.md").exists());
    let exit = dispatch_spec_states_with_track_fixture(tmp.path(), track_id, false);
    assert_eq!(
        exit,
        ExitCode::FAILURE,
        "spec-states must evaluate (not skip) when spec.json is present without spec.md"
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
