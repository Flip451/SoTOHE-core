//! End-to-end contract evidence for the PR and Signal pure-DI command paths.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::Command;

const SIGNAL_AGGREGATE_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Signal aggregate contract fixture",
  "scope": {
    "in_scope": [{
      "id": "IN-01",
      "text": "The aggregate fixture has a grounded requirement.",
      "adr_refs": [{
        "file": "knowledge/adr/signal-aggregate-contract.md",
        "anchor": "D1"
      }]
    }],
    "out_of_scope": []
  },
  "signals": { "blue": 1, "yellow": 0, "red": 0 }
}"#;

const SIGNAL_AGGREGATE_ADR: &str = r#"---
adr_id: signal-aggregate-contract
decisions:
  - id: D1
    status: accepted
    user_decision_ref: chat:signal-aggregate-contract
---
# Signal aggregate contract fixture

## Decision

### D1: Ground the aggregate fixture
"#;

fn configure_git_environment(command: &mut Command, git_config: &Path) {
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("GIT_") {
            command.env_remove(key);
        }
    }
    command
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", git_config)
        .env("GIT_TERMINAL_PROMPT", "0");
}

fn git_command(root: &Path, git_config: &Path) -> Command {
    let mut command = Command::new("git");
    command.current_dir(root);
    configure_git_environment(&mut command, git_config);
    command
}

fn git(root: &Path, git_config: &Path, args: &[&str]) {
    let output = git_command(root, git_config).args(args).output().unwrap();
    assert!(
        output.status.success(),
        "git {args:?} failed:\nstdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
}

fn sotp_bin() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sotp"));
    command.env("SOTP_TELEMETRY", "0");
    command
}

fn initialize_track_workspace(
    track_id: &str,
) -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, String) {
    let sandbox = tempfile::tempdir().unwrap();
    let git_config = sandbox.path().join("gitconfig");
    std::fs::write(&git_config, "").unwrap();
    let remote = sandbox.path().join("origin.git");
    let mut remote_init = Command::new("git");
    configure_git_environment(&mut remote_init, &git_config);
    let remote_output =
        remote_init.args(["init", "--bare", remote.to_str().unwrap()]).output().unwrap();
    assert!(
        remote_output.status.success(),
        "bare remote init failed: {}",
        String::from_utf8_lossy(&remote_output.stderr)
    );
    let workspace = sandbox.path().join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();

    let branch = format!("track/{track_id}");
    git(&workspace, &git_config, &["init", "--initial-branch", &branch]);
    git(&workspace, &git_config, &["config", "user.email", "test@example.com"]);
    git(&workspace, &git_config, &["config", "user.name", "Test User"]);
    std::fs::write(workspace.join("contract.txt"), "pure DI path\n").unwrap();
    git(&workspace, &git_config, &["add", "contract.txt"]);
    git(&workspace, &git_config, &["commit", "-m", "contract fixture"]);
    git(&workspace, &git_config, &["remote", "add", "origin", remote.to_str().unwrap()]);

    let track_dir = workspace.join("track/items").join(track_id);
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        track_dir.join("metadata.json"),
        format!(
            r#"{{
  "schema_version": 6,
  "id": "{track_id}",
  "branch": "{branch}",
  "title": "Pure DI contract fixture",
  "created_at": "2026-07-26T00:00:00Z",
  "updated_at": "2026-07-26T00:00:00Z",
  "branch_strategy_snapshot": {{
    "base_branch": "develop",
    "merge_target": "develop",
    "merge_method": "merge"
  }}
}}
"#
        ),
    )
    .unwrap();

    (sandbox, git_config, remote, workspace, branch)
}

fn initialize_signal_aggregate_workspace(
    source_root: &Path,
    track_id: &str,
) -> (tempfile::TempDir, PathBuf, PathBuf, String) {
    let sandbox = tempfile::tempdir().unwrap();
    let git_config = sandbox.path().join("gitconfig");
    std::fs::write(&git_config, "").unwrap();
    let remote = sandbox.path().join("origin.git");
    let workspace = sandbox.path().join("workspace");
    let branch = format!("track/{track_id}");

    let mut bare_clone = Command::new("git");
    configure_git_environment(&mut bare_clone, &git_config);
    let bare_clone_output = bare_clone
        .args([
            "clone",
            "--bare",
            "--no-hardlinks",
            source_root.to_str().unwrap(),
            remote.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert!(
        bare_clone_output.status.success(),
        "bare fixture clone failed: {}",
        String::from_utf8_lossy(&bare_clone_output.stderr)
    );

    let mut workspace_clone = Command::new("git");
    configure_git_environment(&mut workspace_clone, &git_config);
    let workspace_clone_output = workspace_clone
        .args(["clone", remote.to_str().unwrap(), workspace.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(
        workspace_clone_output.status.success(),
        "workspace fixture clone failed: {}",
        String::from_utf8_lossy(&workspace_clone_output.stderr)
    );
    git(&workspace, &git_config, &["checkout", "-B", &branch]);

    // The aggregate check resolves the active track from this branch, reads
    // spec.json from the working tree, and skips absent per-layer catalogues.
    // No fixture commit is needed for that read-only check path.
    let track_dir = workspace.join("track/items").join(track_id);
    std::fs::create_dir_all(&track_dir).unwrap();
    std::fs::write(
        workspace.join("knowledge/adr/signal-aggregate-contract.md"),
        SIGNAL_AGGREGATE_ADR,
    )
    .unwrap();
    std::fs::write(track_dir.join("spec.json"), SIGNAL_AGGREGATE_SPEC_JSON).unwrap();

    (sandbox, git_config, workspace, branch)
}

#[cfg(unix)]
fn fake_gh_path(sandbox: &Path) -> PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let bin_dir = sandbox.join("bin");
    std::fs::create_dir_all(&bin_dir).unwrap();
    let executable = bin_dir.join("gh");
    std::fs::write(
        &executable,
        "#!/bin/sh\n\
         if [ \"$1\" = pr ] && [ \"$2\" = list ]; then\n\
           printf '73\\n'\n\
           exit 0\n\
         fi\n\
         printf 'unexpected gh invocation: %s\\n' \"$*\" >&2\n\
         exit 1\n",
    )
    .unwrap();
    let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
    permissions.set_mode(0o755);
    std::fs::set_permissions(&executable, permissions).unwrap();
    bin_dir
}

#[cfg(unix)]
#[test]
fn test_pr_ensure_pr_with_existing_pr_preserves_cli_contract() {
    let (sandbox, git_config, _remote, workspace, branch) =
        initialize_track_workspace("pr-signal-contract");
    let bin_dir = fake_gh_path(sandbox.path());
    let inherited_path = std::env::var_os("PATH").unwrap_or_default();
    let path = std::env::join_paths(
        std::iter::once(bin_dir).chain(std::env::split_paths(&inherited_path)),
    )
    .unwrap();

    let mut command = sotp_bin();
    configure_git_environment(&mut command, &git_config);
    let output = command
        .current_dir(&workspace)
        .env("PATH", path)
        .args(["pr", "ensure-pr"])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0), "ensure-pr must succeed: {output:?}");
    assert_eq!(output.stdout, b"[OK] Reusing existing PR #73\n");
    assert!(output.stderr.is_empty(), "ensure-pr must not write stderr: {output:?}");
    assert_eq!(
        git_command(&workspace, &git_config)
            .args(["branch", "--show-current"])
            .output()
            .unwrap()
            .stdout,
        format!("{branch}\n").into_bytes(),
        "ensure-pr must not change the checked-out branch"
    );
}

#[test]
fn test_pr_push_through_pure_di_path_preserves_cli_contract_and_remote_ref() {
    let (_sandbox, git_config, remote, workspace, branch) =
        initialize_track_workspace("pr-signal-push-contract");
    let mut command = sotp_bin();
    configure_git_environment(&mut command, &git_config);
    let output = command.current_dir(&workspace).args(["pr", "push"]).output().unwrap();

    assert_eq!(output.status.code(), Some(0), "pr push must succeed: {output:?}");
    assert_eq!(
        output.stdout,
        format!("Pushing {branch} to origin...\n[OK] Pushed {branch}\n").into_bytes()
    );
    assert!(output.stderr.is_empty(), "successful push must not write stderr: {output:?}");

    let local_head =
        git_command(&workspace, &git_config).args(["rev-parse", "HEAD"]).output().unwrap();
    let remote_head = git_command(&remote, &git_config)
        .args(["rev-parse", &format!("refs/heads/{branch}")])
        .output()
        .unwrap();
    assert_eq!(local_head.status.code(), Some(0));
    assert_eq!(remote_head.status.code(), Some(0));
    assert_eq!(local_head.stdout, remote_head.stdout, "push must persist the branch ref at origin");
}

#[test]
fn test_pr_review_cycle_off_track_branch_preserves_fail_closed_cli_contract() {
    let workspace = tempfile::tempdir().unwrap();
    let git_config = workspace.path().join("gitconfig");
    std::fs::write(&git_config, "").unwrap();
    git(workspace.path(), &git_config, &["init", "--initial-branch", "develop"]);
    git(workspace.path(), &git_config, &["config", "user.email", "test@example.com"]);
    git(workspace.path(), &git_config, &["config", "user.name", "Test User"]);
    git(workspace.path(), &git_config, &["commit", "--allow-empty", "-m", "fixture"]);
    let profiles_dir = workspace.path().join(".harness/config");
    std::fs::create_dir_all(&profiles_dir).unwrap();
    std::fs::write(
        profiles_dir.join("agent-profiles.json"),
        r#"{
  "schema_version": 1,
  "providers": { "codex": { "label": "Codex", "supported_reasoning_efforts": ["low", "medium", "high", "xhigh", "max"] } },
  "capabilities": {
    "pr-reviewer": {
      "provider": "codex",
      "execution_mode": "typed-pipeline"
    }
  }
}
"#,
    )
    .unwrap();

    let mut command = sotp_bin();
    configure_git_environment(&mut command, &git_config);
    let output =
        command.current_dir(workspace.path()).args(["pr", "review-cycle"]).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stdout.is_empty());
    assert_eq!(
        String::from_utf8(output.stderr).unwrap(),
        "not on a track branch (expected track/<id>); switch to the track branch and retry.\n"
    );
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

fn isolated_spec_fixture() -> (tempfile::TempDir, PathBuf) {
    let sandbox = tempfile::tempdir().unwrap();
    let source_spec = workspace_root().join("track/items/pr-signal-pure-di-2026-07-26/spec.json");
    let mut document: serde_json::Value =
        serde_json::from_slice(&std::fs::read(source_spec).unwrap()).unwrap();
    document.as_object_mut().unwrap().remove("signals");

    let spec_json = sandbox.path().join("spec.json");
    std::fs::write(&spec_json, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
    (sandbox, spec_json)
}

#[test]
fn test_signal_check_spec_adr_with_explicit_path_short_circuits_repository_discovery() {
    let root = workspace_root();
    let spec_json = root.join("track/items/pr-signal-pure-di-2026-07-26/spec.json");
    let non_repository = tempfile::tempdir().unwrap();
    let output = sotp_bin()
        .current_dir(non_repository.path())
        .args(["signal", "check-spec-adr", "--strict", "--spec-json", spec_json.to_str().unwrap()])
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(0), "explicit spec path must succeed: {output:?}");
    assert_eq!(
        output.stdout,
        b"--- signal check-spec-adr ---\n[OK] All checks passed.\n--- signal check-spec-adr PASSED ---\n"
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn test_signal_calc_then_check_spec_adr_preserves_isolated_persisted_artifact() {
    let (sandbox, spec_json) = isolated_spec_fixture();
    let persisted_before_calc = std::fs::read(&spec_json).unwrap();

    let calc = sotp_bin()
        .current_dir(sandbox.path())
        .args(["signal", "calc-spec-adr", "--spec-json", spec_json.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(calc.status.success(), "calc must succeed: {calc:?}");
    assert_eq!(
        calc.stdout,
        b"--- signal calc-spec-adr ---\n[OK] All checks passed.\n--- signal calc-spec-adr PASSED ---\n"
    );
    assert!(calc.stderr.is_empty(), "calc must not write stderr: {calc:?}");

    let persisted_after_calc = std::fs::read(&spec_json).unwrap();
    assert_ne!(persisted_after_calc, persisted_before_calc, "calc must persist signal results");
    let document: serde_json::Value = serde_json::from_slice(&persisted_after_calc).unwrap();
    assert!(document.get("signals").is_some(), "calc must persist the signals field");

    let check = sotp_bin()
        .current_dir(sandbox.path())
        .args(["signal", "check-spec-adr", "--strict", "--spec-json", spec_json.to_str().unwrap()])
        .output()
        .unwrap();
    assert!(check.status.success(), "check must succeed: {check:?}");
    assert_eq!(
        check.stdout,
        b"--- signal check-spec-adr ---\n[OK] All checks passed.\n--- signal check-spec-adr PASSED ---\n"
    );
    assert!(check.stderr.is_empty(), "check must not write stderr: {check:?}");
    assert_eq!(
        std::fs::read(spec_json).unwrap(),
        persisted_after_calc,
        "check must read, rather than mutate, the calc-persisted artifact"
    );
}

#[test]
fn test_signal_check_aggregate_preserves_chain_order_and_repository_discovery_contract() {
    let root = workspace_root();
    let (_sandbox, _git_config, workspace, _branch) =
        initialize_signal_aggregate_workspace(&root, "signal-aggregate-contract");
    let spec_json = workspace.join("track/items/signal-aggregate-contract/spec.json");
    let output = sotp_bin()
        .current_dir(&workspace)
        .args([
            "signal",
            "check",
            "--gate",
            "commit",
            "--workspace-root",
            workspace.to_str().unwrap(),
            "--project-root",
            workspace.to_str().unwrap(),
            "--spec-json",
            spec_json.to_str().unwrap(),
        ])
        .output()
        .unwrap();

    assert!(output.status.success(), "aggregate signal check must succeed: {output:?}");
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).unwrap();
    let order = [
        "--- signal check-adr-user ---",
        "--- signal check-spec-adr ---",
        "--- signal check-catalog-spec ---",
        "--- signal check-impl-catalog ---",
    ];
    let mut previous = 0;
    for marker in order {
        let position =
            stdout.find(marker).unwrap_or_else(|| panic!("missing {marker:?}:\n{stdout}"));
        assert!(position >= previous, "chains must run in declared order:\n{stdout}");
        assert_eq!(stdout.matches(marker).count(), 1, "{marker} must run exactly once:\n{stdout}");
        previous = position + marker.len();
    }
    assert!(stdout.ends_with("--- signal check --gate commit PASSED ---\n"));

    let non_repository = tempfile::tempdir().unwrap();
    let discovery_failure = sotp_bin()
        .current_dir(non_repository.path())
        .args(["signal", "check", "--gate", "commit"])
        .output()
        .unwrap();
    assert_eq!(discovery_failure.status.code(), Some(1));
    assert!(discovery_failure.stdout.is_empty());
    let stderr = String::from_utf8(discovery_failure.stderr).unwrap();
    assert!(stderr.starts_with("cannot discover git repository: "), "unexpected stderr: {stderr}");
    assert!(!stderr.contains("[ERROR]"), "repository discovery must not be command-labeled");
    assert!(
        !stderr.contains("pass --workspace-root or --spec-json explicitly"),
        "the aggregate gate must not use the spec-path remediation suffix"
    );

    let command_failure = sotp_bin()
        .current_dir(non_repository.path())
        .args(["signal", "check-catalog-spec", "--strict"])
        .output()
        .unwrap();
    assert_eq!(command_failure.status.code(), Some(1));
    assert!(command_failure.stdout.is_empty());
    let command_stderr = String::from_utf8(command_failure.stderr).unwrap();
    assert!(
        command_stderr.starts_with("[ERROR] signal check-catalog-spec: "),
        "command-scoped failure must retain its label: {command_stderr}"
    );
}
