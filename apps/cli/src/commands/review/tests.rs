#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use crate::commands::track::test_support::process_env_lock;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

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

    let check_zero_findings = TestCli::try_parse_from([
        "sotp",
        "check-zero-findings",
        "--scope",
        "usecase",
        "--round",
        "final",
        "--track-id",
        "check-track",
    ])
    .expect("check-zero-findings must parse through the review command tree");
    let super::ReviewCommand::CheckZeroFindings(args) = check_zero_findings.command else {
        panic!("expected check-zero-findings command");
    };
    assert_eq!(args.round, super::ReviewCheckRoundArg::Final);

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
    let _lock = process_env_lock().lock().unwrap();
    use super::{ReviewCheckApprovedArgs, ReviewCommand, execute};

    let dir = tempfile::tempdir().unwrap();
    let (items_dir, _track_dir) = setup_check_approved_repo(dir.path());
    let _cwd = CurrentDirGuard::change_to(dir.path());

    // Empty diff → "Other" scope is NotRequired(Empty) → Approved.
    let args = ReviewCheckApprovedArgs { items_dir, track_id: Some("test-track".to_string()) };
    let exit = execute(ReviewCommand::CheckApproved(args));
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}

/// Case: all Required(NotStarted) and review.json absent → ApprovedWithBypass → exit 0 + [WARN].
#[test]
fn check_approved_bypass_path_exits_success_with_warn_message() {
    let _lock = process_env_lock().lock().unwrap();
    use super::{ReviewCheckApprovedArgs, ReviewCommand, execute};

    let dir = tempfile::tempdir().unwrap();
    let (items_dir, _track_dir) = setup_check_approved_repo(dir.path());
    let _cwd = CurrentDirGuard::change_to(dir.path());

    // Add an untracked file in libs/domain/ so it shows up in git ls-files --others.
    // The "domain" scope matches "libs/domain/**" → Required(NotStarted).
    // No review.json exists → bypass condition met → ApprovedWithBypass.
    let domain_src = dir.path().join("libs/domain/src");
    fs::create_dir_all(&domain_src).unwrap();
    fs::write(domain_src.join("lib.rs"), "// untracked").unwrap();

    let args = ReviewCheckApprovedArgs { items_dir, track_id: Some("test-track".to_string()) };
    let exit = execute(ReviewCommand::CheckApproved(args));
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}

/// Case: Required scope + review.json present → bypass blocked → Blocked → exit 1 + [BLOCKED].
///
/// The review-scope.json has `review_operational: ["items/<track-id>/review.json"]` so the
/// review.json file written to the track dir is excluded from scope classification and does not
/// create a spurious `Other` required scope that could make this test pass for the wrong reason.
#[test]
fn check_approved_blocked_path_exits_failure_with_blocked_message() {
    let _lock = process_env_lock().lock().unwrap();
    use super::{ReviewCheckApprovedArgs, ReviewCommand, execute};

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

    let args = ReviewCheckApprovedArgs { items_dir, track_id: Some("test-track".to_string()) };
    let exit = execute(ReviewCommand::CheckApproved(args));
    assert_eq!(exit, std::process::ExitCode::FAILURE);
}

#[cfg(unix)]
struct ReviewCommandRouteRepo {
    _dir: tempfile::TempDir,
    root: PathBuf,
    items_dir: PathBuf,
    track_id: String,
    fake_bin_dir: PathBuf,
}

#[cfg(unix)]
fn run_git(root: &Path, args: &[&str]) -> String {
    use std::process::Command;

    let output = Command::new("git").args(args).current_dir(root).output().unwrap();
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_owned()
}

/// Creates a real track repository whose final review verdict can be queried
/// through the CLI command enum.
#[cfg(unix)]
fn setup_review_command_route_repo(track_id: &str) -> ReviewCommandRouteRepo {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();
    run_git(&root, &["init", "-b", "main"]);
    run_git(&root, &["config", "user.email", "test@example.invalid"]);
    run_git(&root, &["config", "user.name", "Test"]);

    let config_dir = root.join(".harness/config");
    fs::create_dir_all(&config_dir).unwrap();
    fs::write(
        config_dir.join("review-scope.json"),
        r#"{"version":2,"groups":{"cli":{"patterns":["src/**"]}},"review_operational":["track/items/<track-id>/**","tmp/reviewer-runtime/**"]}"#,
    )
    .unwrap();
    fs::write(root.join("README.md"), "base\n").unwrap();
    run_git(&root, &["add", "."]);
    run_git(&root, &["commit", "-m", "base"]);
    let base_commit = run_git(&root, &["rev-parse", "HEAD"]);

    let track_branch = format!("track/{track_id}");
    run_git(&root, &["checkout", "-b", &track_branch]);
    fs::create_dir_all(root.join("src")).unwrap();
    fs::write(root.join("src/lib.rs"), "pub fn reviewed() {}\n").unwrap();
    fs::create_dir_all(root.join("other")).unwrap();
    fs::write(root.join("other/review_target.rs"), "pub fn reviewed_other() {}\n").unwrap();
    run_git(&root, &["add", "src/lib.rs", "other/review_target.rs"]);
    run_git(&root, &["commit", "-m", "review target"]);

    let items_dir = root.join("track/items");
    let track_dir = items_dir.join(track_id);
    fs::create_dir_all(&track_dir).unwrap();
    fs::write(track_dir.join(".commit_hash"), base_commit).unwrap();
    fs::write(
        track_dir.join("metadata.json"),
        format!(
            r#"{{"schema_version":6,"id":"{track_id}","title":"Test Track","created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z","branch_strategy_snapshot":{{"base_branch":"main","merge_target":"main","merge_method":"squash"}}}}"#
        ),
    )
    .unwrap();

    let fake_bin_dir = root.join("fake-bin");
    fs::create_dir_all(&fake_bin_dir).unwrap();
    let codex = fake_bin_dir.join("codex");
    fs::write(
        &codex,
        r#"#!/bin/sh
case "$1" in
  --version) echo "codex 0.125.0"; exit 0 ;;
esac
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '{"verdict":"zero_findings","findings":[]}\n' > "$out"
"#,
    )
    .unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&codex, fs::Permissions::from_mode(0o755)).unwrap();

    ReviewCommandRouteRepo {
        _dir: dir,
        root,
        items_dir,
        track_id: track_id.to_owned(),
        fake_bin_dir,
    }
}

#[cfg(unix)]
fn with_fake_codex_on_path<T>(bin_dir: &Path, action: impl FnOnce() -> T) -> T {
    let mut path = bin_dir.as_os_str().to_os_string();
    path.push(":");
    path.push(env::var_os("PATH").unwrap_or_default());
    temp_env::with_var("PATH", Some(path), action)
}

fn parse_review_command(args: &[&str]) -> super::ReviewCommand {
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: super::ReviewCommand,
    }

    TestCli::try_parse_from(args).expect("review command must parse").command
}

/// The `Local` enum route must select the configured pre-review workflow
/// before the provider-resolved reviewer can be launched.
#[cfg(unix)]
#[test]
fn review_local_enum_route_runs_selected_pre_review_gate_before_reviewer() {
    let _lock = process_env_lock().lock().unwrap();
    let repo = setup_review_command_route_repo("review-local-enum-route");
    fs::write(
        repo.root.join(".harness/config/pre-review-gates.json"),
        r#"{
  "schema_version": 1,
  "scopes": [{
    "scope": "cli",
    "commands": [{"argv": ["sh", "-c", "touch .pre-review-command-ran; exit 1"], "timeout_seconds": null}]
  }]
}"#,
    )
    .unwrap();
    let _cwd = CurrentDirGuard::change_to(&repo.root);
    let items_dir = repo.items_dir.to_str().unwrap();
    let command = parse_review_command(&[
        "sotp",
        "local",
        "--prompt",
        "review",
        "--round-type",
        "fast",
        "--group",
        "cli",
        "--track-id",
        &repo.track_id,
        "--items-dir",
        items_dir,
    ]);

    let exit = super::execute(command);

    assert_eq!(exit, std::process::ExitCode::FAILURE);
    assert!(
        repo.root.join(".pre-review-command-ran").exists(),
        "the Local enum route must enter the selected pre-review command workflow"
    );
}

/// A parsed `check-zero-findings --scope cli --round final` command must
/// select the persisted final verdict for that scope at the CLI enum route.
#[cfg(unix)]
#[test]
fn review_check_zero_findings_enum_route_selects_final_scope_state_for_exit_code() {
    let _lock = process_env_lock().lock().unwrap();
    let repo = setup_review_command_route_repo("review-check-zero-enum-route");
    let _cwd = CurrentDirGuard::change_to(&repo.root);

    with_fake_codex_on_path(&repo.fake_bin_dir, || {
        let review = cli_composition::ReviewCompositionRoot::new()
            .review_run_codex(cli_composition::review_v2::ReviewRunCodexInput {
                model: "test-codex".to_owned(),
                timeout_seconds: 10,
                briefing_file: None,
                prompt: Some("review".to_owned()),
                track_id: Some(repo.track_id.clone()),
                round_type: "final".to_owned(),
                group: "cli".to_owned(),
                items_dir: repo.items_dir.clone(),
            })
            .unwrap();
        assert_eq!(review.exit_code, 0, "fixture must persist a final zero-findings verdict");

        let items_dir = repo.items_dir.to_str().unwrap();
        let args = [
            "sotp",
            "check-zero-findings",
            "--scope",
            "cli",
            "--round",
            "final",
            "--track-id",
            repo.track_id.as_str(),
            "--items-dir",
            items_dir,
        ];
        assert_eq!(super::execute(parse_review_command(&args)), std::process::ExitCode::SUCCESS);

        fs::write(repo.root.join("src/lib.rs"), "pub fn changed_after_review() {}\n").unwrap();
        assert_eq!(super::execute(parse_review_command(&args)), std::process::ExitCode::FAILURE);
    });
}

/// A parsed `check-zero-findings --scope other --round final` command must
/// select the persisted `Other` verdict rather than a named scope's verdict.
#[cfg(unix)]
#[test]
fn review_check_zero_findings_enum_route_selects_other_scope_state_for_exit_code() {
    let _lock = process_env_lock().lock().unwrap();
    let repo = setup_review_command_route_repo("review-check-zero-other-enum-route");
    let _cwd = CurrentDirGuard::change_to(&repo.root);

    with_fake_codex_on_path(&repo.fake_bin_dir, || {
        let review = cli_composition::ReviewCompositionRoot::new()
            .review_run_codex(cli_composition::review_v2::ReviewRunCodexInput {
                model: "test-codex".to_owned(),
                timeout_seconds: 10,
                briefing_file: None,
                prompt: Some("review".to_owned()),
                track_id: Some(repo.track_id.clone()),
                round_type: "final".to_owned(),
                group: "other".to_owned(),
                items_dir: repo.items_dir.clone(),
            })
            .unwrap();
        assert_eq!(
            review.exit_code, 0,
            "fixture must persist an Other final zero-findings verdict"
        );

        let items_dir = repo.items_dir.to_str().unwrap();
        let args = [
            "sotp",
            "check-zero-findings",
            "--scope",
            "other",
            "--round",
            "final",
            "--track-id",
            repo.track_id.as_str(),
            "--items-dir",
            items_dir,
        ];
        assert_eq!(super::execute(parse_review_command(&args)), std::process::ExitCode::SUCCESS);

        fs::write(
            repo.root.join("other/review_target.rs"),
            "pub fn changed_after_other_review() {}\n",
        )
        .unwrap();
        assert_eq!(super::execute(parse_review_command(&args)), std::process::ExitCode::FAILURE);
    });
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
    let _lock = process_env_lock().lock().unwrap();
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
    let _lock = process_env_lock().lock().unwrap();
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
#[test]
fn test_review_command_rejects_non_final_or_missing_check_zero_findings_round() {
    use clap::Parser;

    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        command: super::ReviewCommand,
    }

    assert!(
        TestCli::try_parse_from([
            "sotp",
            "check-zero-findings",
            "--scope",
            "usecase",
            "--round",
            "fast",
        ])
        .is_err()
    );
    assert!(
        TestCli::try_parse_from(["sotp", "check-zero-findings", "--scope", "usecase"]).is_err()
    );
}
