//! Integration tests for `sotp hook dispatch`.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::io::Write as _;
use std::process::{Command, Output, Stdio};

fn hook_command(args: &[&str], guarded_git_token: Option<&str>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_sotp"));
    command.args(args).env_remove("SOTP_GUARDED_GIT");
    if let Some(value) = guarded_git_token {
        command.env("SOTP_GUARDED_GIT", value);
    }
    command
}

fn run_hook(args: &[&str], guarded_git_token: Option<&str>) -> Output {
    let mut command = hook_command(args, guarded_git_token);
    command.stdin(Stdio::null());
    command.output().unwrap()
}

fn run_hook_with_stdin(args: &[&str], guarded_git_token: Option<&str>, stdin: &[u8]) -> Output {
    let mut command = hook_command(args, guarded_git_token);
    command.stdin(Stdio::piped());

    let mut child = command.spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn assert_exit_code(output: &Output, expected: i32) {
    assert_eq!(
        output.status.code(),
        Some(expected),
        "stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn test_hook_dispatch_block_direct_git_ops_with_empty_stdin_returns_exit_2() {
    let output = run_hook(&["hook", "dispatch", "block-direct-git-ops"], None);

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_block_direct_git_ops_with_malformed_stdin_returns_exit_2() {
    let output =
        run_hook_with_stdin(&["hook", "dispatch", "block-direct-git-ops"], None, b"not json");

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_block_test_file_deletion_with_empty_stdin_returns_exit_2() {
    let output = run_hook(&["hook", "dispatch", "block-test-file-deletion"], None);

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_block_test_file_deletion_with_malformed_stdin_returns_exit_2() {
    let output =
        run_hook_with_stdin(&["hook", "dispatch", "block-test-file-deletion"], None, b"not json");

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_git_ref_update_prepared_without_token_blocks() {
    let output = run_hook(&["hook", "dispatch", "git-ref-update", "prepared"], None);

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_git_ref_update_prepared_with_token_allows() {
    let output = run_hook(&["hook", "dispatch", "git-ref-update", "prepared"], Some("1"));

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_git_ref_update_committed_without_token_allows() {
    let output = run_hook(&["hook", "dispatch", "git-ref-update", "committed"], None);

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_git_ref_update_committed_with_token_allows() {
    let output = run_hook(&["hook", "dispatch", "git-ref-update", "committed"], Some("1"));

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_git_ref_update_aborted_without_token_allows() {
    let output = run_hook(&["hook", "dispatch", "git-ref-update", "aborted"], None);

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_git_ref_update_aborted_with_token_allows() {
    let output = run_hook(&["hook", "dispatch", "git-ref-update", "aborted"], Some("1"));

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_git_pre_push_with_remote_args_and_token_allows() {
    let output =
        run_hook(&["hook", "dispatch", "git-pre-push", "origin", "https://example.com"], Some("1"));

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_git_pre_push_with_remote_args_without_token_blocks() {
    let output =
        run_hook(&["hook", "dispatch", "git-pre-push", "origin", "https://example.com"], None);

    assert_exit_code(&output, 2);
    assert!(output.stdout.is_empty(), "stdout must be empty for blocking hook verdict");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Direct git ref updates"), "stderr must include block reason");
}
