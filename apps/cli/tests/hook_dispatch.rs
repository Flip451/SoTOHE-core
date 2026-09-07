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
    let temporary_directory = tempfile::TempDir::new().unwrap();
    let mut command = hook_command(args, guarded_git_token);
    command.current_dir(temporary_directory.path());
    command.stdin(Stdio::null());
    command.output().unwrap()
}

fn run_hook_with_stdin(args: &[&str], guarded_git_token: Option<&str>, stdin: &[u8]) -> Output {
    let temporary_directory = tempfile::TempDir::new().unwrap();
    let mut command = hook_command(args, guarded_git_token);
    command.current_dir(temporary_directory.path());
    command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());

    let mut child = command.spawn().unwrap();
    child.stdin.as_mut().unwrap().write_all(stdin).unwrap();
    child.wait_with_output().unwrap()
}

fn run_agent_hook(host: &str, hook: &str, stdin: &[u8]) -> Output {
    run_hook_with_stdin(&["hook", "dispatch", "--host", host, hook], None, stdin)
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
    let output = run_hook(&["hook", "dispatch", "--host", "claude", "block-direct-git-ops"], None);

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_block_direct_git_ops_with_malformed_stdin_returns_exit_2() {
    let output = run_hook_with_stdin(
        &["hook", "dispatch", "--host", "claude", "block-direct-git-ops"],
        None,
        b"not json",
    );

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_block_test_file_deletion_with_empty_stdin_returns_exit_2() {
    let output =
        run_hook(&["hook", "dispatch", "--host", "claude", "block-test-file-deletion"], None);

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_block_test_file_deletion_with_malformed_stdin_returns_exit_2() {
    let output = run_hook_with_stdin(
        &["hook", "dispatch", "--host", "claude", "block-test-file-deletion"],
        None,
        b"not json",
    );

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

#[test]
fn test_hook_dispatch_agent_without_host_returns_exit_2() {
    for hook in ["block-direct-git-ops", "skill-compliance"] {
        let output = run_hook(&["hook", "dispatch", hook], None);

        assert_exit_code(&output, 2);
        assert!(String::from_utf8_lossy(&output.stderr).contains("--host"));
    }
}

#[test]
fn test_hook_dispatch_git_with_host_returns_exit_2() {
    let output = run_hook(
        &["hook", "dispatch", "git-ref-update", "--host", "claude", "committed"],
        Some("1"),
    );

    assert_exit_code(&output, 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("must not specify --host"));
}

#[test]
fn test_hook_dispatch_git_with_host_after_argument_returns_exit_2() {
    let output = run_hook(
        &["hook", "dispatch", "git-ref-update", "committed", "--host", "claude"],
        Some("1"),
    );

    assert_exit_code(&output, 2);
    assert!(String::from_utf8_lossy(&output.stderr).contains("must not specify --host"));
}

#[test]
fn test_hook_dispatch_git_arguments_after_explicit_delimiter_remain_git_argv() {
    let output = run_hook(
        &["hook", "dispatch", "git-ref-update", "committed", "--", "--host", "claude"],
        Some("1"),
    );

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_invalid_host_returns_exit_2() {
    for hook in ["block-direct-git-ops", "skill-compliance"] {
        let output = run_hook(&["hook", "dispatch", "--host", "invalid", hook], None);

        assert_exit_code(&output, 2);
    }
}

#[test]
fn test_hook_dispatch_grok_dual_field_terminal_uses_grok_values() {
    let output = run_agent_hook(
        "grok",
        "block-direct-git-ops",
        br#"{"toolName":"run_terminal_command","toolInput":{"command":"git add safe.txt"},"tool_name":"Write","tool_input":{"file_path":"safe.txt","content":"not a git command"}}"#,
    );

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_grok_dual_field_search_replace_uses_grok_values() {
    let output = run_agent_hook(
        "grok",
        "block-test-file-deletion",
        br#"{"toolName":"search_replace","toolInput":{"file_path":"tests/example.rs","old_string":"old","new_string":"new"},"tool_name":"Bash","tool_input":{"command":"rm tests/example.rs"}}"#,
    );

    assert_exit_code(&output, 0);
}

#[test]
fn test_hook_dispatch_grok_malformed_or_wrong_format_fails_closed() {
    let envelopes = [
        br#"{"toolName":"run_terminal_command"}"#.as_slice(),
        br#"{"toolName":"unknown_tool","toolInput":{"command":"git status"}}"#.as_slice(),
        br#"{"toolName":"run_terminal_command","toolInput":{"command":42}}"#.as_slice(),
        br#"{"tool_name":"Bash","tool_input":{"command":"git add safe.txt"}}"#.as_slice(),
    ];

    for envelope in envelopes {
        let output = run_agent_hook("grok", "block-direct-git-ops", envelope);
        assert_exit_code(&output, 2);
    }
}

#[test]
fn test_hook_dispatch_claude_and_codex_use_snake_case_pretool_use_values() {
    let envelope = br#"{"tool_name":"Bash","tool_input":{"command":"git add safe.txt"}}"#;

    let outputs = [
        run_agent_hook("claude", "block-direct-git-ops", envelope),
        run_agent_hook("codex", "block-direct-git-ops", envelope),
    ];

    for output in &outputs {
        assert_exit_code(output, 2);
    }

    assert_eq!(outputs[0].stdout, outputs[1].stdout);
    assert_eq!(outputs[0].stderr, outputs[1].stderr);
}

#[test]
fn test_hook_dispatch_codex_apply_patch_guard_remains_fail_closed() {
    let output = run_agent_hook(
        "codex",
        "block-test-file-deletion",
        b"{\"tool_name\":\"apply_patch\",\"tool_input\":{\"command\":\"*** Begin Patch\\n*** Delete File: tests/example.rs\\n*** End Patch\"}}",
    );

    assert_exit_code(&output, 2);
}

#[test]
fn test_hook_dispatch_skill_compliance_is_consistent_across_hosts() {
    let normal_prompt = br#"{"prompt":"hello"}"#;
    let guidance_prompt = br#"{"prompt":"/track:review"}"#;

    let mut normal_outputs = Vec::new();
    let mut guidance_outputs = Vec::new();
    for host in ["claude", "codex", "grok"] {
        let normal = run_agent_hook(host, "skill-compliance", normal_prompt);
        assert_exit_code(&normal, 0);
        normal_outputs.push(normal.stdout);

        let guidance = run_agent_hook(host, "skill-compliance", guidance_prompt);
        assert_exit_code(&guidance, 0);
        guidance_outputs.push(guidance.stdout);
    }

    assert!(normal_outputs.iter().all(Vec::is_empty));
    assert!(guidance_outputs.iter().all(|stdout| !stdout.is_empty()));
    assert!(guidance_outputs.windows(2).all(|pair| { pair.first() == pair.get(1) }));

    let rendered: serde_json::Value =
        serde_json::from_slice(guidance_outputs.first().expect("guidance output must exist"))
            .unwrap();
    let hook_output = rendered
        .get("hookSpecificOutput")
        .and_then(serde_json::Value::as_object)
        .expect("skill-compliance output must contain hookSpecificOutput");
    assert_eq!(
        hook_output.get("hookEventName").and_then(serde_json::Value::as_str),
        Some("UserPromptSubmit")
    );
    assert!(
        hook_output
            .get("additionalContext")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|context| context.contains("/track:review"))
    );
}

#[test]
fn test_hook_dispatch_skill_compliance_malformed_input_warns_without_blocking_direct_dispatch() {
    for host in ["claude", "codex", "grok"] {
        let output = run_agent_hook(host, "skill-compliance", b"not json");

        assert_exit_code(&output, 0);
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr)
                .contains("warning: failed to parse prompt JSON")
        );
    }
}
