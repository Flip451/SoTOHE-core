#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn review_local_defaults_to_one_hour_timeout() {
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: super::ReviewCommand,
    }

    let local = TestCli::try_parse_from([
        "sotp",
        "local",
        "--prompt",
        "review",
        "--round-type",
        "fast",
        "--group",
        "cli",
    ])
    .expect("local must parse");

    let super::ReviewCommand::Local(args) = local.command else {
        panic!("expected local");
    };
    assert_eq!(args.timeout_seconds, 3_600);
}

#[test]
fn review_command_parses_all_surviving_variants() {
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: super::ReviewCommand,
    }

    let local = TestCli::try_parse_from([
        "sotp",
        "local",
        "--prompt",
        "review",
        "--round-type",
        "fast",
        "--group",
        "cli",
    ])
    .expect("local must parse");
    assert!(matches!(local.command, super::ReviewCommand::Local(_)));

    let fix_local = TestCli::try_parse_from([
        "sotp",
        "fix-local",
        "--scope",
        "cli",
        "--briefing-file",
        "briefing.md",
        "--round-type",
        "fast",
    ])
    .expect("fix-local must parse");
    assert!(matches!(fix_local.command, super::ReviewCommand::FixLocal(_)));

    let check_approved =
        TestCli::try_parse_from(["sotp", "check-approved"]).expect("check-approved must parse");
    assert!(matches!(check_approved.command, super::ReviewCommand::CheckApproved(_)));

    let results = TestCli::try_parse_from(["sotp", "results"]).expect("results must parse");
    assert!(matches!(results.command, super::ReviewCommand::Results(_)));

    let classify =
        TestCli::try_parse_from(["sotp", "classify", "Cargo.toml"]).expect("classify must parse");
    assert!(matches!(classify.command, super::ReviewCommand::Classify(_)));

    let files =
        TestCli::try_parse_from(["sotp", "files", "--scope", "cli"]).expect("files must parse");
    assert!(matches!(files.command, super::ReviewCommand::Files(_)));
}

#[test]
fn review_rejects_retired_provider_specific_local_commands() {
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: super::ReviewCommand,
    }

    for command in ["codex-local", "claude-local"] {
        let parse_result = TestCli::try_parse_from([
            "sotp",
            command,
            "--model",
            "reviewer-model",
            "--prompt",
            "review",
            "--round-type",
            "fast",
            "--group",
            "cli",
        ]);
        let error = match parse_result {
            Ok(_) => panic!("retired review command '{command}' must not parse"),
            Err(error) => error,
        };
        assert_eq!(
            error.kind(),
            clap::error::ErrorKind::InvalidSubcommand,
            "retired review command '{command}' must be unknown"
        );
    }
}

struct CurrentDirGuard {
    original: PathBuf,
}

impl CurrentDirGuard {
    fn change_to(path: &Path) -> Self {
        let original = env::current_dir().unwrap();
        env::set_current_dir(path).unwrap();
        Self { original }
    }
}

impl Drop for CurrentDirGuard {
    fn drop(&mut self) {
        env::set_current_dir(&self.original).unwrap();
    }
}

/// Sets up a minimal git repo with v2 review-scope.json in the given directory.
///
/// Required for tests that change cwd to a tempdir and call infrastructure
/// functions that need git discovery.
fn setup_test_git_repo(root: &Path) {
    // Minimal v2 review-scope.json (empty groups — only Other scope exists)
    setup_git_repo_with_scope_json(root, r#"{"version": 2, "groups": {}}"#);
}

// ---------------------------------------------------------------------------
// check-approved: T004 verdict mapping tests
// ---------------------------------------------------------------------------

/// Writes a `.harness/config/review-scope.json` with a single "domain" group matching
/// `libs/domain/**`.
///
/// Includes a `review_operational` exclusion for `items/<track-id>/review.json` so
/// that the review.json file written by the blocked-path test does not spill into
/// the `Other` scope and cause the test to pass for the wrong reason.
fn write_domain_scope_config(root: &Path) {
    let config_dir = root.join(".harness/config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("review-scope.json"),
        r#"{
  "version": 2,
  "groups": {"domain": {"patterns": ["libs/domain/**"]}},
  "review_operational": ["items/<track-id>/review.json"],
  "other_track": []
}"#,
    )
    .unwrap();
}

/// Sets up a minimal git repo with a domain scope, creates the items dir and track dir,
/// returns (items_dir, track_dir).
fn setup_check_approved_repo(root: &Path) -> (PathBuf, PathBuf) {
    use std::process::Command;

    Command::new("git").args(["init", "-b", "main"]).current_dir(root).output().unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git").args(["config", "user.name", "Test"]).current_dir(root).output().unwrap();

    write_domain_scope_config(root);
    fs::create_dir_all(root.join("track/items")).unwrap();

    let items_dir = root.join("items");
    let track_dir = items_dir.join("test-track");
    fs::create_dir_all(&track_dir).unwrap();

    // Commit metadata.json so the empty-diff approval test has no untracked fixture noise.
    fs::write(
        track_dir.join("metadata.json"),
        r#"{"schema_version":6,"id":"test-track","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{"base_branch":"main","merge_target":"main","merge_method":"squash"}}"#,
    )
    .unwrap();

    Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
    Command::new("git").args(["commit", "-m", "init"]).current_dir(root).output().unwrap();

    (items_dir, track_dir)
}

/// Case: all scopes NotRequired (empty diff) → Approved verdict → exit 0 + [OK].
#[test]
fn check_approved_approved_path_exits_success_with_ok_message() {
    let _lock = env_lock().lock().unwrap();
    use super::{CheckApprovedArgs, execute_check_approved};

    let dir = tempfile::tempdir().unwrap();
    let (items_dir, _track_dir) = setup_check_approved_repo(dir.path());
    let _cwd = CurrentDirGuard::change_to(dir.path());

    // Empty diff → "Other" scope is NotRequired(Empty) → Approved.
    let args = CheckApprovedArgs { items_dir, track_id: Some("test-track".to_string()) };
    let exit = execute_check_approved(&args);
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}

/// Case: all Required(NotStarted) and review.json absent → ApprovedWithBypass → exit 0 + [WARN].
#[test]
fn check_approved_bypass_path_exits_success_with_warn_message() {
    let _lock = env_lock().lock().unwrap();
    use super::{CheckApprovedArgs, execute_check_approved};

    let dir = tempfile::tempdir().unwrap();
    let (items_dir, _track_dir) = setup_check_approved_repo(dir.path());
    let _cwd = CurrentDirGuard::change_to(dir.path());

    // Add an untracked file in libs/domain/ so it shows up in git ls-files --others.
    // The "domain" scope matches "libs/domain/**" → Required(NotStarted).
    // No review.json exists → bypass condition met → ApprovedWithBypass.
    let domain_src = dir.path().join("libs/domain/src");
    fs::create_dir_all(&domain_src).unwrap();
    fs::write(domain_src.join("lib.rs"), "// untracked").unwrap();

    let args = CheckApprovedArgs { items_dir, track_id: Some("test-track".to_string()) };
    let exit = execute_check_approved(&args);
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}

/// Case: Required scope + review.json present → bypass blocked → Blocked → exit 1 + [BLOCKED].
///
/// The review-scope.json has `review_operational: ["items/<track-id>/review.json"]` so the
/// review.json file written to the track dir is excluded from scope classification and does not
/// create a spurious `Other` required scope that could make this test pass for the wrong reason.
#[test]
fn check_approved_blocked_path_exits_failure_with_blocked_message() {
    let _lock = env_lock().lock().unwrap();
    use super::{CheckApprovedArgs, execute_check_approved};

    let dir = tempfile::tempdir().unwrap();
    let (items_dir, track_dir) = setup_check_approved_repo(dir.path());
    let _cwd = CurrentDirGuard::change_to(dir.path());

    // Add an untracked file in libs/domain/ → Required(NotStarted) for domain scope.
    let domain_src = dir.path().join("libs/domain/src");
    fs::create_dir_all(&domain_src).unwrap();
    fs::write(domain_src.join("lib.rs"), "// untracked").unwrap();

    // Write an empty review.json to disable the NotStarted bypass.
    // review_operational in the scope config excludes this file from scope classification.
    fs::write(track_dir.join("review.json"), r#"{"schema_version":2,"scopes":{}}"#).unwrap();

    let args = CheckApprovedArgs { items_dir, track_id: Some("test-track".to_string()) };
    let exit = execute_check_approved(&args);
    assert_eq!(exit, std::process::ExitCode::FAILURE);
}

// ---------------------------------------------------------------------------
// format_approval_verdict: AC-10 observable surface (message prefix) tests
// ---------------------------------------------------------------------------
//
// These tests verify the `[OK]` / `[WARN]` / `[BLOCKED]` prefix contract (AC-10)
// directly against the pure `format_approval_verdict` function, which avoids the
// need to redirect the real stderr in the integration tests above.

#[test]
fn format_approval_verdict_approved_has_ok_prefix() {
    use super::format_approval_verdict;
    use usecase::review_v2::{ReviewApprovalDecision, ReviewApprovalOutput};

    let output = ReviewApprovalOutput {
        decision: ReviewApprovalDecision::Approved,
        bypass_scope_count: None,
        blocked_scopes: vec![],
    };
    let (msg, code) = format_approval_verdict(output);
    assert!(
        msg.starts_with("[OK]"),
        "Approved message must start with [OK] prefix (AC-10); got: {msg:?}"
    );
    assert_eq!(code, std::process::ExitCode::SUCCESS);
}

#[test]
fn format_approval_verdict_approved_with_bypass_has_warn_prefix() {
    use super::format_approval_verdict;
    use usecase::review_v2::{ReviewApprovalDecision, ReviewApprovalOutput};

    let output = ReviewApprovalOutput {
        decision: ReviewApprovalDecision::ApprovedWithBypass,
        bypass_scope_count: Some(2),
        blocked_scopes: vec![],
    };
    let (msg, code) = format_approval_verdict(output);
    assert!(
        msg.starts_with("[WARN]"),
        "ApprovedWithBypass message must start with [WARN] prefix (AC-10); got: {msg:?}"
    );
    assert!(
        msg.contains("2 scope(s)"),
        "ApprovedWithBypass message must include scope count; got: {msg:?}"
    );
    assert_eq!(code, std::process::ExitCode::SUCCESS);
}

#[test]
fn format_approval_verdict_blocked_has_blocked_prefix_and_lists_scopes() {
    use super::format_approval_verdict;
    use usecase::review_v2::{ReviewApprovalDecision, ReviewApprovalOutput};

    let output = ReviewApprovalOutput {
        decision: ReviewApprovalDecision::Blocked,
        bypass_scope_count: None,
        blocked_scopes: vec!["cli".to_owned(), "domain".to_owned()],
    };
    let (msg, code) = format_approval_verdict(output);
    assert!(
        msg.starts_with("[BLOCKED]"),
        "Blocked message must start with [BLOCKED] prefix (AC-10); got: {msg:?}"
    );
    assert!(
        msg.contains("  cli") && msg.contains("  domain"),
        "Blocked message must list required scope names; got: {msg:?}"
    );
    assert_eq!(code, std::process::ExitCode::FAILURE);
}

// ---------------------------------------------------------------------------
// build_review_v2 items_dir path traversal guard tests
// ---------------------------------------------------------------------------

#[test]
fn build_review_v2_rejects_items_dir_outside_repo_root() {
    // Serialize with env_lock because build_review_v2_str uses SystemGitRepo::discover()
    // (via infrastructure::review_v2::build_review_v2_str) which depends on cwd — other tests may change cwd concurrently.
    let _lock = env_lock().lock().unwrap();
    // Use /tmp as items_dir — this should always be outside the repo root.
    let result =
        cli_composition::review_v2::build_review_v2_str("test-track", std::path::Path::new("/tmp"));
    assert!(result.is_err(), "build_review_v2_str should reject items_dir outside repo root");
    let err = result.err().expect("checked is_err above").to_string();
    assert!(
        err.contains("outside the repository root") || err.contains("git discover"),
        "error should mention path traversal guard: {err}"
    );
}

#[test]
fn build_review_v2_rejects_traversal_items_dir_outside_repo_root() {
    // A relative path with ".." that resolves outside the repo root should be
    // rejected by the canonicalize + starts_with containment check.
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    setup_test_git_repo(dir.path());
    let _cwd = CurrentDirGuard::change_to(dir.path());

    // "items/../../../tmp" — resolves outside repo root
    let traversal_path = PathBuf::from("items/../../../tmp");
    let result = cli_composition::review_v2::build_review_v2_str("test-track", &traversal_path);
    assert!(result.is_err(), "items_dir outside repo should be rejected");
    let err = result.err().expect("checked is_err above").to_string();
    assert!(
        err.contains("outside the repository root"),
        "error should mention containment violation: {err}"
    );
}

/// Sets up a minimal git repo with a custom `.harness/config/review-scope.json` content.
///
/// Unlike `setup_test_git_repo` (which writes `{"version": 2, "groups": {}}`), this
/// helper writes arbitrary JSON so tests can configure specific scope/briefing combos.
fn setup_git_repo_with_scope_json(root: &Path, scope_json: &str) {
    use std::process::Command;
    Command::new("git").args(["init", "-b", "main"]).current_dir(root).output().unwrap();
    Command::new("git")
        .args(["config", "user.email", "test@test.com"])
        .current_dir(root)
        .output()
        .unwrap();
    Command::new("git").args(["config", "user.name", "Test"]).current_dir(root).output().unwrap();

    let config_dir = root.join(".harness/config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(config_dir.join("review-scope.json"), scope_json).unwrap();
    fs::create_dir_all(root.join("track/items")).unwrap();

    Command::new("git").args(["add", "."]).current_dir(root).output().unwrap();
    Command::new("git").args(["commit", "-m", "init"]).current_dir(root).output().unwrap();
}

// ---------------------------------------------------------------------------
// resolve_reviewer_for_test: CN-03 fail-closed provider resolution tests
// ---------------------------------------------------------------------------

use super::local::resolve_reviewer_for_test;

/// Writes an agent-profiles.json at the given path with the provided content.
fn write_profiles_json(dir: &Path, content: &str) -> PathBuf {
    use std::io::Write;
    let config_dir = dir.join(".harness").join("config");
    fs::create_dir_all(&config_dir).unwrap();
    let path = config_dir.join("agent-profiles.json");
    let mut f = fs::File::create(&path).unwrap();
    f.write_all(content.as_bytes()).unwrap();
    path
}

#[test]
fn resolve_reviewer_fails_closed_when_reviewer_capability_missing() {
    // CN-03: resolve_execution("reviewer", round_type) returning None → fail-closed error.
    let dir = tempfile::tempdir().unwrap();
    // agent-profiles.json has no "reviewer" capability.
    let path = write_profiles_json(
        dir.path(),
        r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex CLI" } },
  "capabilities": {}
}"#,
    );
    let result = resolve_reviewer_for_test(dir.path(), &path, super::CodexRoundTypeArg::Fast);
    assert!(result.is_err(), "expected error when reviewer capability is missing");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("reviewer capability not defined"),
        "error must explain that reviewer capability is missing; got: {err}"
    );
}

#[test]
fn resolve_reviewer_fails_closed_when_provider_is_unsupported() {
    // CN-03: an unknown/unsupported provider → fail-closed error (never run a review
    // with an unknown provider).
    let dir = tempfile::tempdir().unwrap();
    let path = write_profiles_json(
        dir.path(),
        r#"{
  "schema_version": 1,
  "providers": { "gemini": { "label": "Gemini CLI" } },
  "capabilities": {
    "reviewer": { "provider": "gemini", "model": "gemini-2.5-pro", "reasoning_effort": "high", "execution_mode": "typed-pipeline" }
  }
}"#,
    );
    let result = resolve_reviewer_for_test(dir.path(), &path, super::CodexRoundTypeArg::Final);
    assert!(result.is_err(), "expected error for unsupported provider");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("unsupported reviewer provider") && err.contains("gemini"),
        "error must name the unsupported provider; got: {err}"
    );
}

#[test]
fn resolve_reviewer_succeeds_for_codex_provider() {
    // CN-03: known provider "codex" → no error.
    let dir = tempfile::tempdir().unwrap();
    let path = write_profiles_json(
        dir.path(),
        r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex CLI" } },
  "capabilities": {
    "reviewer": { "provider": "codex", "model": "gpt-5.4", "reasoning_effort": "high", "execution_mode": "typed-pipeline" }
  }
}"#,
    );
    let result = resolve_reviewer_for_test(dir.path(), &path, super::CodexRoundTypeArg::Final);
    assert!(result.is_ok(), "expected Ok for codex provider; got: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.provider, "codex");
    assert_eq!(resolved.model.as_deref(), Some("gpt-5.4"));
}

#[test]
fn resolve_reviewer_succeeds_for_claude_provider() {
    // CN-03: known provider "claude" → no error.
    let dir = tempfile::tempdir().unwrap();
    let path = write_profiles_json(
        dir.path(),
        r#"{
  "schema_version": 1,
  "providers": { "claude": { "label": "Claude Code" } },
  "capabilities": {
    "reviewer": { "provider": "claude", "model": "claude-sonnet-4-6", "reasoning_effort": "high", "execution_mode": "typed-pipeline" }
  }
}"#,
    );
    let result = resolve_reviewer_for_test(dir.path(), &path, super::CodexRoundTypeArg::Final);
    assert!(result.is_ok(), "expected Ok for claude provider; got: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.provider, "claude");
    assert_eq!(resolved.model.as_deref(), Some("claude-sonnet-4-6"));
}

#[test]
fn resolve_reviewer_fast_round_uses_fast_model_from_codex_provider() {
    // AC-04: round_type is passed straight to resolve_execution, so fast_model
    // is selected automatically for fast rounds.
    let dir = tempfile::tempdir().unwrap();
    let path = write_profiles_json(
        dir.path(),
        r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex CLI" } },
  "capabilities": {
    "reviewer": { "provider": "codex", "model": "gpt-5.4", "fast_model": "gpt-5.4-mini", "reasoning_effort": "xhigh", "fast_reasoning_effort": "low", "execution_mode": "typed-pipeline" }
  }
}"#,
    );
    let result = resolve_reviewer_for_test(dir.path(), &path, super::CodexRoundTypeArg::Fast);
    assert!(result.is_ok(), "expected Ok for fast round; got: {:?}", result.err());
    let resolved = result.unwrap();
    assert_eq!(resolved.provider, "codex");
    assert_eq!(
        resolved.model.as_deref(),
        Some("gpt-5.4-mini"),
        "fast round must select fast_model"
    );
}

#[test]
fn resolve_reviewer_fast_round_mixed_provider_selects_fast_provider() {
    // AC-04: fast_provider overrides the base provider for fast rounds.
    let dir = tempfile::tempdir().unwrap();
    let path = write_profiles_json(
        dir.path(),
        r#"{
  "schema_version": 1,
  "providers": {
    "claude": { "label": "Claude Code" },
    "codex": { "label": "Codex CLI" }
  },
  "capabilities": {
    "reviewer": {
      "provider": "claude",
      "model": "claude-opus-4-7",
      "fast_provider": "codex",
      "fast_model": "gpt-5.4-mini",
      "reasoning_effort": "max",
      "fast_reasoning_effort": "low",
      "execution_mode": "typed-pipeline"
    }
  }
}"#,
    );
    let result = resolve_reviewer_for_test(dir.path(), &path, super::CodexRoundTypeArg::Fast);
    assert!(
        result.is_ok(),
        "expected Ok for fast round with fast_provider; got: {:?}",
        result.err()
    );
    let resolved = result.unwrap();
    assert_eq!(resolved.provider, "codex", "fast round must use fast_provider");
    assert_eq!(resolved.model.as_deref(), Some("gpt-5.4-mini"));
}
