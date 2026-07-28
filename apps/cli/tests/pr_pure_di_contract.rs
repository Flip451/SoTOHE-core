//! End-to-end contract evidence for the PR pure-DI command path.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

fn sotp_bin(git_config: &Path) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sotp"));
    command.env("SOTP_TELEMETRY", "0");
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

fn git_output(root: &Path, git_config: &Path, args: &[&str]) -> Output {
    git_command(root, git_config).args(args).output().unwrap()
}

fn initialize_pr_push_fixture() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf, String) {
    let sandbox = tempfile::tempdir().unwrap();
    let git_config = sandbox.path().join("gitconfig");
    std::fs::write(&git_config, "").unwrap();

    let remote = sandbox.path().join("origin.git");
    let workspace = sandbox.path().join("workspace");
    let track_id = "pr-pure-di-contract";
    let branch = format!("track/{track_id}");

    let mut remote_init = Command::new("git");
    configure_git_environment(&mut remote_init, &git_config);
    let remote_output =
        remote_init.args(["init", "--bare", remote.to_str().unwrap()]).output().unwrap();
    assert!(
        remote_output.status.success(),
        "bare remote init failed: {}",
        String::from_utf8_lossy(&remote_output.stderr),
    );

    std::fs::create_dir_all(&workspace).unwrap();
    git(&workspace, &git_config, &["init", "--initial-branch", &branch]);
    git(&workspace, &git_config, &["config", "user.email", "test@example.com"]);
    git(&workspace, &git_config, &["config", "user.name", "Test User"]);
    std::fs::write(workspace.join("contract.txt"), "pure DI path\n").unwrap();
    git(&workspace, &git_config, &["add", "contract.txt"]);
    git(&workspace, &git_config, &["commit", "-m", "contract fixture"]);
    git(&workspace, &git_config, &["remote", "add", "origin", remote.to_str().unwrap()]);

    (sandbox, git_config, remote, workspace, branch)
}

#[test]
fn test_pr_push_through_pure_di_path_preserves_cli_contract_and_remote_ref() {
    let (_sandbox, git_config, remote, workspace, branch) = initialize_pr_push_fixture();

    let output =
        sotp_bin(&git_config).current_dir(&workspace).args(["pr", "push"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "pr push must succeed: {output:?}");
    assert_eq!(
        output.stdout,
        format!("Pushing {branch} to origin...\n[OK] Pushed {branch}\n").into_bytes(),
        "successful push must preserve stdout exactly",
    );
    assert!(output.stderr.is_empty(), "successful push must not write stderr: {output:?}");

    let local_head = git_output(&workspace, &git_config, &["rev-parse", "HEAD"]);
    let remote_head =
        git_output(&remote, &git_config, &["rev-parse", &format!("refs/heads/{branch}")]);
    assert_eq!(local_head.status.code(), Some(0));
    assert_eq!(remote_head.status.code(), Some(0));
    assert_eq!(local_head.stdout, remote_head.stdout, "push must persist the branch ref at origin");
}

#[test]
fn test_pr_push_through_pure_di_path_preserves_failure_contract_and_remote_ref() {
    let (sandbox, git_config, remote, workspace, branch) = initialize_pr_push_fixture();
    let rejected_push_target = sandbox.path().join("rejected-origin.git");
    git(
        &workspace,
        &git_config,
        &["remote", "set-url", "--push", "origin", rejected_push_target.to_str().unwrap()],
    );

    let output =
        sotp_bin(&git_config).current_dir(&workspace).args(["pr", "push"]).output().unwrap();
    assert!(!output.status.success(), "failed push must return a non-zero exit code: {output:?}");
    assert_eq!(
        output.stdout,
        format!("Pushing {branch} to origin...\n").into_bytes(),
        "failed push must not write a success message to stdout",
    );
    assert!(!output.stderr.is_empty(), "failed push must write its error to stderr: {output:?}");

    let remote_head =
        git_output(&remote, &git_config, &["rev-parse", &format!("refs/heads/{branch}")]);
    assert!(
        !remote_head.status.success(),
        "failed push must not persist the branch ref at origin: {remote_head:?}",
    );
}
