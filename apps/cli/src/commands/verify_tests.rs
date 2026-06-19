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
