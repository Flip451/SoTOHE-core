#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{
    CODEX_BIN_ENV, CodexInvocation, PlanCodexLocalArgs,
    codex_local::{build_codex_invocation, build_prompt, run_codex_local_invocation},
};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

fn env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct EnvVarGuard {
    key: &'static str,
    original: Option<OsString>,
}

impl EnvVarGuard {
    fn set(key: &'static str, value: &std::ffi::OsStr) -> Self {
        let original = std::env::var_os(key);
        // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
        unsafe { std::env::set_var(key, value) };
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => {
                // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
                unsafe { std::env::set_var(self.key, value) };
            }
            None => {
                // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
                unsafe { std::env::remove_var(self.key) };
            }
        }
    }
}

fn fake_args(prompt: Option<String>, briefing_file: Option<PathBuf>) -> PlanCodexLocalArgs {
    PlanCodexLocalArgs { model: "gpt-5.4".to_owned(), timeout_seconds: 600, briefing_file, prompt }
}

// ---------------------------------------------------------------------------
// build_prompt tests
// ---------------------------------------------------------------------------

#[test]
fn build_prompt_with_briefing_file_returns_read_instruction() {
    let dir = tempfile::tempdir().unwrap();
    let briefing = dir.path().join("briefing.md");
    fs::write(&briefing, "# Task\n").unwrap();
    let args = fake_args(None, Some(briefing.clone()));

    let prompt = build_prompt(&args).unwrap();

    assert_eq!(
        prompt,
        format!("Read {} and perform the task described there.", briefing.display())
    );
}

#[test]
fn build_prompt_with_inline_prompt_returns_prompt_string() {
    let args = fake_args(Some("Analyze this design.".to_owned()), None);

    let prompt = build_prompt(&args).unwrap();

    assert_eq!(prompt, "Analyze this design.");
}

#[test]
fn build_prompt_with_missing_briefing_file_returns_error() {
    let args = fake_args(None, Some(PathBuf::from("/nonexistent/briefing.md")));

    let result = build_prompt(&args);

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("briefing file not found"));
}

// ---------------------------------------------------------------------------
// build_codex_invocation tests
// ---------------------------------------------------------------------------

#[test]
fn build_codex_invocation_without_full_auto_omits_full_auto_flag() {
    let invocation = build_codex_invocation("gpt-5.3-codex-spark", "Design this module.", false);
    let rendered: Vec<String> =
        invocation.args.iter().map(|arg| arg.to_string_lossy().to_string()).collect();

    assert_eq!(rendered.first().map(String::as_str), Some("exec"));
    assert!(rendered.windows(2).any(|pair| pair == ["--sandbox", "read-only"]));
    assert!(rendered.windows(2).any(|pair| pair == ["--model", "gpt-5.3-codex-spark"]));
    assert!(!rendered.iter().any(|arg| arg == "--full-auto"));
    // No --output-schema or --output-last-message (planner is simpler than reviewer)
    assert!(!rendered.iter().any(|arg| arg == "--output-schema"));
    assert!(!rendered.iter().any(|arg| arg == "--output-last-message"));
}

#[test]
fn build_codex_invocation_with_full_auto_includes_full_auto_before_read_only() {
    let invocation = build_codex_invocation("gpt-5.4", "Design this module.", true);
    let rendered: Vec<String> =
        invocation.args.iter().map(|arg| arg.to_string_lossy().to_string()).collect();

    assert!(rendered.iter().any(|arg| arg == "--full-auto"));
    // --sandbox read-only must appear AFTER --full-auto (last-wins semantics)
    let full_auto_pos = rendered.iter().position(|arg| arg == "--full-auto").unwrap();
    let sandbox_pos = rendered.iter().position(|arg| arg == "read-only").unwrap();
    assert!(full_auto_pos < sandbox_pos, "--full-auto must come before read-only");
}

#[test]
fn build_codex_invocation_includes_prompt_as_last_arg() {
    let prompt = "Please analyze this trait design.";
    let invocation = build_codex_invocation("gpt-5.4", prompt, false);
    let rendered: Vec<String> =
        invocation.args.iter().map(|arg| arg.to_string_lossy().to_string()).collect();

    assert_eq!(rendered.last().map(String::as_str), Some(prompt));
}

#[test]
fn build_codex_invocation_uses_codex_as_default_bin() {
    let _lock = env_lock().lock().unwrap();
    let _guard = EnvVarGuard::set(CODEX_BIN_ENV, std::ffi::OsStr::new(""));

    let invocation = build_codex_invocation("gpt-5.4", "prompt", false);

    assert_eq!(invocation.bin, OsString::from("codex"));
}

#[test]
fn build_codex_invocation_uses_env_override_for_bin() {
    let _lock = env_lock().lock().unwrap();
    let _guard = EnvVarGuard::set(CODEX_BIN_ENV, std::ffi::OsStr::new("/custom/codex"));

    let invocation = build_codex_invocation("gpt-5.4", "prompt", false);

    assert_eq!(invocation.bin, OsString::from("/custom/codex"));
}

// ---------------------------------------------------------------------------
// Integration tests: subprocess lifecycle
// ---------------------------------------------------------------------------

#[cfg(unix)]
fn write_fake_codex_script(root: &Path) -> PathBuf {
    let script = root.join("fake-codex.sh");
    fs::write(
        &script,
        r#"#!/bin/sh
set -eu
sleep_seconds="${SOTP_FAKE_CODEX_SLEEP_SECONDS:-0}"
if [ "$sleep_seconds" != "0" ]; then
  sleep "$sleep_seconds"
fi
exit "${SOTP_FAKE_CODEX_EXIT_CODE:-0}"
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    use std::os::unix::fs::PermissionsExt;
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();
    script
}

#[cfg(unix)]
#[test]
fn run_codex_local_invocation_happy_path_returns_exit_code_zero() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _exit = EnvVarGuard::set("SOTP_FAKE_CODEX_EXIT_CODE", std::ffi::OsStr::new("0"));

    let invocation = build_codex_invocation("gpt-5.4", "Plan this feature.", true);
    let result = run_codex_local_invocation(&invocation, Duration::from_secs(10)).unwrap();

    assert_eq!(result.exit_code, 0);
}

#[cfg(unix)]
#[test]
fn run_codex_local_invocation_propagates_nonzero_exit_code() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _exit = EnvVarGuard::set("SOTP_FAKE_CODEX_EXIT_CODE", std::ffi::OsStr::new("42"));

    let invocation = build_codex_invocation("gpt-5.4", "Plan this feature.", true);
    let result = run_codex_local_invocation(&invocation, Duration::from_secs(10)).unwrap();

    assert_eq!(result.exit_code, 42);
}

#[cfg(unix)]
#[test]
fn run_codex_local_invocation_returns_exit_code_1_on_timeout() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _sleep = EnvVarGuard::set("SOTP_FAKE_CODEX_SLEEP_SECONDS", std::ffi::OsStr::new("30"));

    let invocation = build_codex_invocation("gpt-5.4", "Plan this feature.", true);
    // Timeout of 0 seconds triggers immediate timeout
    let result = run_codex_local_invocation(&invocation, Duration::from_secs(0)).unwrap();

    assert_eq!(result.exit_code, 1, "timeout should return exit code 1");
}

#[test]
fn run_codex_local_invocation_spawn_failure_returns_error() {
    let _lock = env_lock().lock().unwrap();
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, std::ffi::OsStr::new("/nonexistent/codex-binary"));

    let invocation = CodexInvocation {
        bin: OsString::from("/nonexistent/codex-binary"),
        args: vec![OsString::from("exec"), OsString::from("prompt")],
    };
    let result = run_codex_local_invocation(&invocation, Duration::from_secs(10));

    assert!(result.is_err());
    assert!(result.unwrap_err().contains("failed to spawn"));
}
