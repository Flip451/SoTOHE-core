#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::{
    CODEX_BIN_ENV, CodexLocalArgs, ReviewRunResult, codex_local::build_codex_invocation,
    codex_local::build_prompt, codex_local::render_codex_local_result,
    codex_local::run_codex_local,
};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
#[cfg(unix)]
use std::time::Duration;
use usecase::review_workflow::ReviewVerdict;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

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
        let original = env::var_os(key);
        // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
        unsafe { env::set_var(key, value) };
        Self { key, original }
    }
}

impl Drop for EnvVarGuard {
    fn drop(&mut self) {
        match &self.original {
            Some(value) => {
                // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
                unsafe { env::set_var(self.key, value) };
            }
            None => {
                // SAFETY: tests serialize access via env_lock(), so mutating process env here is safe.
                unsafe { env::remove_var(self.key) };
            }
        }
    }
}

fn fake_args(
    prompt: Option<String>,
    briefing_file: Option<PathBuf>,
    output_last_message: PathBuf,
    timeout_seconds: u64,
) -> CodexLocalArgs {
    CodexLocalArgs {
        model: "gpt-5.4".to_owned(),
        timeout_seconds,
        briefing_file,
        prompt,
        output_last_message: Some(output_last_message),
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

#[cfg(unix)]
fn process_is_gone_or_zombie(pid: i32) -> bool {
    // Safety: kill with signal 0 only probes whether the process still exists.
    let status = unsafe { libc::kill(pid, 0) };
    let err = std::io::Error::last_os_error();
    if status != 0 && err.raw_os_error() == Some(libc::ESRCH) {
        return true;
    }

    let stat_path = PathBuf::from(format!("/proc/{pid}/stat"));
    let Ok(stat) = fs::read_to_string(stat_path) else {
        return false;
    };
    stat.split_whitespace().nth(2) == Some("Z")
}

fn write_fake_codex_script(root: &Path) -> PathBuf {
    let script = root.join("fake-codex.sh");
    fs::write(
        &script,
        r#"#!/bin/sh
set -eu
args_file="${SOTP_FAKE_CODEX_ARGS_FILE:-}"
if [ -n "$args_file" ]; then
  printf '%s\n' "$@" > "$args_file"
fi
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --output-last-message)
      out="$2"
      shift 2
      ;;
    *)
      shift
      ;;
  esac
done
sleep_seconds="${SOTP_FAKE_CODEX_SLEEP_SECONDS:-0}"
if [ "$sleep_seconds" != "0" ]; then
  sleep "$sleep_seconds"
fi
message="${SOTP_FAKE_CODEX_MESSAGE:-}"
if [ -n "$message" ] && [ -n "$out" ]; then
  printf '%s\n' "$message" > "$out"
fi
exit "${SOTP_FAKE_CODEX_EXIT_CODE:-0}"
"#,
    )
    .unwrap();
    #[cfg(unix)]
    {
        let mut perms = fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&script, perms).unwrap();
    }
    script
}

#[test]
fn build_prompt_uses_briefing_file_reference() {
    let dir = tempfile::tempdir().unwrap();
    let briefing = dir.path().join("briefing.md");
    fs::write(&briefing, "# Task\n").unwrap();
    let args = fake_args(None, Some(briefing.clone()), dir.path().join("out.txt"), 1);

    let prompt = build_prompt(&args).unwrap();

    assert_eq!(
        prompt,
        format!("Read {} and perform the task described there.", briefing.display())
    );
}

#[test]
fn build_codex_invocation_omits_full_auto_when_false() {
    let invocation = build_codex_invocation(
        "gpt-5.3-codex-spark",
        "Review this change.",
        Path::new("tmp/reviewer-runtime/out.txt"),
        Path::new("tmp/reviewer-runtime/schema.json"),
        false,
    );
    let rendered =
        invocation.args.iter().map(|arg| arg.to_string_lossy().to_string()).collect::<Vec<_>>();

    assert_eq!(rendered.first().map(String::as_str), Some("exec"));
    assert!(rendered.windows(2).any(|pair| pair == ["--sandbox", "read-only"]));
    assert!(
        rendered
            .windows(2)
            .any(|pair| pair == ["--output-schema", "tmp/reviewer-runtime/schema.json"])
    );
    assert!(rendered.windows(2).any(|pair| pair == ["--model", "gpt-5.3-codex-spark"]));
    assert!(!rendered.iter().any(|arg| arg == "--full-auto"));
}

#[test]
fn build_codex_invocation_includes_full_auto_then_read_only_when_true() {
    let invocation = build_codex_invocation(
        "gpt-5.4",
        "Review this change.",
        Path::new("tmp/reviewer-runtime/out.txt"),
        Path::new("tmp/reviewer-runtime/schema.json"),
        true,
    );
    let rendered =
        invocation.args.iter().map(|arg| arg.to_string_lossy().to_string()).collect::<Vec<_>>();

    assert_eq!(rendered.first().map(String::as_str), Some("exec"));
    assert!(rendered.iter().any(|arg| arg == "--full-auto"));
    // --sandbox read-only must appear AFTER --full-auto to override workspace-write
    let full_auto_pos = rendered.iter().position(|a| a == "--full-auto").unwrap();
    let sandbox_pos = rendered.windows(2).position(|p| p == ["--sandbox", "read-only"]).unwrap();
    assert!(
        sandbox_pos > full_auto_pos,
        "--sandbox read-only must come after --full-auto to override its implicit workspace-write"
    );
    assert!(rendered.windows(2).any(|pair| pair == ["--model", "gpt-5.4"]));
}

#[test]
fn render_codex_local_result_emits_zero_findings_json_to_stdout() {
    let rendered = render_codex_local_result(
        &fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            PathBuf::from("tmp/reviewer-runtime/out.txt"),
            1,
        ),
        Ok(ReviewRunResult {
            verdict: ReviewVerdict::ZeroFindings,
            final_message: Some("{\"verdict\":\"zero_findings\",\"findings\":[]}".to_owned()),
            output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
            output_last_message_auto_managed: false,
            verdict_detail: None,
        }),
    );

    assert_eq!(rendered.exit_code, 0);
    assert_eq!(
        rendered.stdout_lines,
        vec!["{\"verdict\":\"zero_findings\",\"findings\":[]}".to_owned()]
    );
    assert!(rendered.stderr_lines.is_empty());
}

#[test]
fn render_codex_local_result_emits_findings_json_to_stdout() {
    let rendered = render_codex_local_result(
        &fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            PathBuf::from("tmp/reviewer-runtime/out.txt"),
            1,
        ),
        Ok(ReviewRunResult {
            verdict: ReviewVerdict::FindingsRemain,
            final_message: Some(
                "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}".to_owned(),
            ),
            output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
            output_last_message_auto_managed: false,
            verdict_detail: None,
        }),
    );

    assert_eq!(rendered.exit_code, 2);
    assert_eq!(
        rendered.stdout_lines,
        vec![
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}"
                .to_owned(),
        ]
    );
    assert!(rendered.stderr_lines.is_empty());
}

#[test]
fn render_codex_local_result_hides_auto_managed_path_for_timeout() {
    let rendered = render_codex_local_result(
        &fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            PathBuf::from("tmp/reviewer-runtime/out.txt"),
            1,
        ),
        Ok(ReviewRunResult {
            verdict: ReviewVerdict::Timeout,
            final_message: None,
            output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
            output_last_message_auto_managed: true,
            verdict_detail: None,
        }),
    );

    assert_eq!(rendered.exit_code, 1);
    assert_eq!(rendered.stderr_lines, vec!["[TIMEOUT] Local reviewer exceeded 1s".to_owned()]);
}

#[test]
fn render_codex_local_result_keeps_explicit_path_for_missing_message() {
    let rendered = render_codex_local_result(
        &fake_args(
            Some("Review this implementation.".to_owned()),
            None,
            PathBuf::from("tmp/reviewer-runtime/out.txt"),
            1,
        ),
        Ok(ReviewRunResult {
            verdict: ReviewVerdict::LastMessageMissing,
            final_message: None,
            output_last_message: PathBuf::from("tmp/reviewer-runtime/out.txt"),
            output_last_message_auto_managed: false,
            verdict_detail: None,
        }),
    );

    assert_eq!(rendered.exit_code, 1);
    assert_eq!(
        rendered.stderr_lines,
        vec![
            "[ERROR] Local reviewer finished without a final message: tmp/reviewer-runtime/out.txt"
                .to_owned()
        ]
    );
}

#[test]
fn run_codex_local_reports_zero_findings() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _message = EnvVarGuard::set(
        "SOTP_FAKE_CODEX_MESSAGE",
        std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
    );

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output.clone(),
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
    assert_eq!(
        result.final_message.as_deref(),
        Some("{\"verdict\":\"zero_findings\",\"findings\":[]}")
    );
    assert_eq!(result.output_last_message, output);
}

#[test]
fn run_codex_local_reports_findings_when_final_message_is_present() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _message = EnvVarGuard::set(
        "SOTP_FAKE_CODEX_MESSAGE",
        std::ffi::OsStr::new(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: review finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}",
        ),
    );

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::FindingsRemain);
    assert_eq!(
        result.final_message.as_deref(),
        Some(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: review finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}"
        )
    );
}

#[test]
fn run_codex_local_reports_process_failed_when_findings_payload_has_nonzero_exit() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _message = EnvVarGuard::set(
        "SOTP_FAKE_CODEX_MESSAGE",
        std::ffi::OsStr::new(
            "{\"verdict\":\"findings_remain\",\"findings\":[{\"message\":\"P1: review finding\",\"severity\":\"P1\",\"file\":null,\"line\":null}]}",
        ),
    );
    let _code = EnvVarGuard::set("SOTP_FAKE_CODEX_EXIT_CODE", std::ffi::OsStr::new("1"));

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::ProcessFailed);
}

#[test]
fn run_codex_local_canonicalizes_pretty_printed_final_json() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _message = EnvVarGuard::set(
        "SOTP_FAKE_CODEX_MESSAGE",
        std::ffi::OsStr::new("{\n  \"verdict\": \"zero_findings\",\n  \"findings\": []\n}"),
    );

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
    assert_eq!(
        result.final_message.as_deref(),
        Some("{\"verdict\":\"zero_findings\",\"findings\":[]}")
    );
}

#[test]
fn run_codex_local_clears_stale_explicit_output_file_before_invocation() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    fs::write(&output, "{\"verdict\":\"zero_findings\",\"findings\":[]}").unwrap();
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output.clone(),
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::LastMessageMissing);
    assert_eq!(result.final_message, None);
    assert_eq!(fs::read_to_string(output).unwrap(), "");
}

#[test]
fn run_codex_local_cleans_auto_managed_artifacts_when_spawn_fails() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _cwd = CurrentDirGuard::change_to(dir.path());
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, std::ffi::OsStr::new("definitely-missing-codex"));

    let args = CodexLocalArgs {
        model: "gpt-5.4".to_owned(),
        timeout_seconds: 1,
        briefing_file: None,
        prompt: Some("Review this implementation.".to_owned()),
        output_last_message: None,
    };

    let err = run_codex_local(&args).unwrap_err();
    assert!(err.contains("failed to spawn"));

    // Auto-managed artifacts (output-last-message, output-schema) should be cleaned up.
    // Session log persists intentionally for post-run debugging.
    let runtime_dir = dir.path().join("tmp/reviewer-runtime");
    if runtime_dir.is_dir() {
        let remaining: Vec<_> = fs::read_dir(&runtime_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.file_name().to_string_lossy().to_string())
            .collect();
        // Only session log files should remain
        assert!(
            remaining.iter().all(|name| name.starts_with("codex-session-")),
            "unexpected non-session-log artifacts remain: {remaining:?}"
        );
    }
}

#[test]
fn run_codex_local_reports_last_message_missing_on_success() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::LastMessageMissing);
    assert_eq!(result.final_message, None);
}

#[test]
fn run_codex_local_reports_process_failed_for_invalid_json_payload() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _message = EnvVarGuard::set("SOTP_FAKE_CODEX_MESSAGE", std::ffi::OsStr::new("NO_FINDINGS"));

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::ProcessFailed);
    assert!(
        result
            .verdict_detail
            .as_deref()
            .is_some_and(|detail| detail.contains("invalid reviewer final payload"))
    );
}

#[test]
fn run_codex_local_reports_timeout() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _sleep = EnvVarGuard::set("SOTP_FAKE_CODEX_SLEEP_SECONDS", std::ffi::OsStr::new("1"));

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        0,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::Timeout);
    assert_eq!(result.final_message, None);
}

#[cfg(unix)]
#[test]
fn run_codex_local_kills_reviewer_process_group_on_timeout() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let script = dir.path().join("fake-codex-tree.sh");
    let child_pid_file = dir.path().join("child.pid");
    fs::write(
        &script,
        r#"#!/bin/sh
set -eu
pid_file="${SOTP_FAKE_CODEX_CHILD_PID_FILE:-}"
if [ -n "$pid_file" ]; then
  sleep 30 &
  echo "$!" > "$pid_file"
fi
sleep "${SOTP_FAKE_CODEX_SLEEP_SECONDS:-30}"
"#,
    )
    .unwrap();
    let mut perms = fs::metadata(&script).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script, perms).unwrap();

    let output = dir.path().join("last.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _pid_file = EnvVarGuard::set("SOTP_FAKE_CODEX_CHILD_PID_FILE", child_pid_file.as_os_str());
    let _sleep = EnvVarGuard::set("SOTP_FAKE_CODEX_SLEEP_SECONDS", std::ffi::OsStr::new("30"));

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::Timeout);

    for _ in 0..20 {
        if child_pid_file.is_file() {
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    let child_pid = fs::read_to_string(&child_pid_file).unwrap();
    let child_pid = child_pid.trim().parse::<i32>().unwrap();
    let mut child_gone = false;
    for _ in 0..40 {
        if process_is_gone_or_zombie(child_pid) {
            child_gone = true;
            break;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    assert!(child_gone, "expected timed-out reviewer descendant {child_pid} to be gone or zombie");
}

fn write_agent_profiles(root: &Path, model_profiles_json: &str) {
    let claude_dir = root.join(".claude");
    fs::create_dir_all(&claude_dir).unwrap();
    fs::write(
        claude_dir.join("agent-profiles.json"),
        format!(
            r#"{{
  "version": 1,
  "providers": {{
    "codex": {{
      "default_model": "gpt-5.4",
      "model_profiles": {model_profiles_json}
    }}
  }},
  "profiles": {{
    "default": {{ "reviewer": "codex" }}
  }}
}}"#
        ),
    )
    .unwrap();
}

#[test]
fn run_codex_local_passes_full_auto_for_full_model() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _cwd = CurrentDirGuard::change_to(dir.path());
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let args_file = dir.path().join("codex-args.txt");
    write_agent_profiles(
        dir.path(),
        r#"{"gpt-5.4": {"full_auto": true}, "gpt-5.3-codex-spark": {"full_auto": false}}"#,
    );
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _args_env = EnvVarGuard::set("SOTP_FAKE_CODEX_ARGS_FILE", args_file.as_os_str());
    let _message = EnvVarGuard::set(
        "SOTP_FAKE_CODEX_MESSAGE",
        std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
    );

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
    let args_content = fs::read_to_string(&args_file).unwrap();
    assert!(
        args_content.contains("--full-auto"),
        "expected --full-auto in args for full model, got: {args_content}"
    );
}

#[test]
fn run_codex_local_omits_full_auto_for_spark_model() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _cwd = CurrentDirGuard::change_to(dir.path());
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let args_file = dir.path().join("codex-args.txt");
    write_agent_profiles(
        dir.path(),
        r#"{"gpt-5.4": {"full_auto": true}, "gpt-5.3-codex-spark": {"full_auto": false}}"#,
    );
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _args_env = EnvVarGuard::set("SOTP_FAKE_CODEX_ARGS_FILE", args_file.as_os_str());
    let _message = EnvVarGuard::set(
        "SOTP_FAKE_CODEX_MESSAGE",
        std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
    );

    let mut args = fake_args(Some("Review this implementation.".to_owned()), None, output, 1);
    args.model = "gpt-5.3-codex-spark".to_owned();

    let result = run_codex_local(&args).unwrap();

    assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
    let args_content = fs::read_to_string(&args_file).unwrap();
    assert!(
        !args_content.contains("--full-auto"),
        "expected no --full-auto in args for spark model, got: {args_content}"
    );
}

#[test]
fn run_codex_local_defaults_to_full_auto_when_profiles_missing() {
    let _lock = env_lock().lock().unwrap();
    let dir = tempfile::tempdir().unwrap();
    let _cwd = CurrentDirGuard::change_to(dir.path());
    // No agent-profiles.json written — file read should fail, fall back to full_auto=true
    let script = write_fake_codex_script(dir.path());
    let output = dir.path().join("last.txt");
    let args_file = dir.path().join("codex-args.txt");
    let _bin = EnvVarGuard::set(CODEX_BIN_ENV, script.as_os_str());
    let _args_env = EnvVarGuard::set("SOTP_FAKE_CODEX_ARGS_FILE", args_file.as_os_str());
    let _message = EnvVarGuard::set(
        "SOTP_FAKE_CODEX_MESSAGE",
        std::ffi::OsStr::new("{\"verdict\":\"zero_findings\",\"findings\":[]}"),
    );

    let result = run_codex_local(&fake_args(
        Some("Review this implementation.".to_owned()),
        None,
        output,
        1,
    ))
    .unwrap();

    assert_eq!(result.verdict, ReviewVerdict::ZeroFindings);
    let args_content = fs::read_to_string(&args_file).unwrap();
    assert!(
        args_content.contains("--full-auto"),
        "expected --full-auto (fail-closed) when profiles missing, got: {args_content}"
    );
}

// ---------------------------------------------------------------------------
// is_planning_only_path unit tests
// ---------------------------------------------------------------------------

#[test]
fn planning_only_path_accepts_track_files() {
    use super::is_planning_only_path;

    assert!(is_planning_only_path("track/items/my-track/metadata.json"));
    assert!(is_planning_only_path("track/registry.md"));
    assert!(is_planning_only_path("track/tech-stack.md"));
    assert!(is_planning_only_path("track/workflow.md"));
}

#[test]
fn planning_only_path_accepts_doc_and_config_files() {
    use super::is_planning_only_path;

    assert!(is_planning_only_path(".claude/docs/DESIGN.md"));
    assert!(is_planning_only_path(".claude/commands/track/review.md"));
    assert!(is_planning_only_path(".claude/rules/04-coding-principles.md"));
    assert!(is_planning_only_path(".claude/agent-profiles.json"));
    assert!(is_planning_only_path(".claude/settings.json"));
    assert!(is_planning_only_path("project-docs/conventions/hexagonal-architecture.md"));
    assert!(is_planning_only_path("docs/architecture-rules.json"));
    assert!(is_planning_only_path("knowledge/adr/2026-03-11-0000-foo.md"));
    assert!(is_planning_only_path("CLAUDE.md"));
    assert!(is_planning_only_path("DEVELOPER_AI_WORKFLOW.md"));
    assert!(is_planning_only_path("TRACK_TRACEABILITY.md"));
    assert!(is_planning_only_path("README.md"));
    // Root-level .md files are planning-only
    assert!(is_planning_only_path("AGENTS.md"));
    assert!(is_planning_only_path("CHANGELOG.md"));
}

#[test]
fn planning_only_path_rejects_code_files() {
    use super::is_planning_only_path;

    assert!(!is_planning_only_path("libs/domain/src/review/state.rs"));
    assert!(!is_planning_only_path("libs/usecase/src/review_workflow/usecases.rs"));
    assert!(!is_planning_only_path("libs/infrastructure/src/review_adapters.rs"));
    assert!(!is_planning_only_path("apps/cli/src/commands/review/mod.rs"));
    assert!(!is_planning_only_path("Cargo.toml"));
    assert!(!is_planning_only_path("Cargo.lock"));
    // Executable code under .claude/ and tmp/ must NOT be planning-only
    assert!(!is_planning_only_path(".claude/hooks/check-codex-before-write.py"));
    assert!(!is_planning_only_path(".claude/skills/track-plan/SKILL.md"));
    assert!(!is_planning_only_path("tmp/reviewer-runtime/briefing.md"));
    assert!(!is_planning_only_path("tmp/some-script.sh"));
    assert!(!is_planning_only_path("Makefile.toml"));
    assert!(!is_planning_only_path("Dockerfile"));
    assert!(!is_planning_only_path("deny.toml"));
    assert!(!is_planning_only_path("rustfmt.toml"));
    assert!(!is_planning_only_path("scripts/check_layers.py"));
    assert!(!is_planning_only_path("vendor/conch-parser/src/lib.rs"));
    // Code/unknown extensions in planning-only directories are rejected (fail-closed)
    assert!(!is_planning_only_path("docs/exploit.rs"));
    assert!(!is_planning_only_path("track/items/my-track/helper.py"));
    assert!(!is_planning_only_path("knowledge/script.sh"));
    assert!(!is_planning_only_path("docs/exploit.js"));
    assert!(!is_planning_only_path("docs/tool.rb"));
    assert!(!is_planning_only_path("track/items/my-track/Dockerfile"));
}

// ---------------------------------------------------------------------------
// is_planning_only_path: boundary cases and composition tests
// ---------------------------------------------------------------------------

#[test]
fn planning_only_path_empty_staged_is_planning_only() {
    // When no files are staged, all(is_planning_only) returns true (vacuous truth).
    use super::is_planning_only_path;

    let staged: [&str; 0] = [];
    assert!(staged.iter().all(|f| is_planning_only_path(f)));
}

#[test]
fn planning_only_path_mixed_staged_is_not_planning_only() {
    // When a mix of planning-only and code files are staged, result is false.
    use super::is_planning_only_path;

    let staged = ["track/items/my-track/metadata.json", "libs/domain/src/review/state.rs"];
    assert!(!staged.iter().all(|f| is_planning_only_path(f)));
}

#[test]
fn planning_only_path_all_planning_staged_is_planning_only() {
    use super::is_planning_only_path;

    let staged = [
        "track/items/my-track/metadata.json",
        "track/registry.md",
        ".claude/docs/DESIGN.md",
        "CLAUDE.md",
    ];
    assert!(staged.iter().all(|f| is_planning_only_path(f)));
}

#[test]
fn planning_only_path_all_code_staged_is_not_planning_only() {
    use super::is_planning_only_path;

    let staged =
        ["libs/usecase/src/review_workflow/usecases.rs", "apps/cli/src/commands/review/mod.rs"];
    assert!(!staged.iter().all(|f| is_planning_only_path(f)));
}

// ---------------------------------------------------------------------------
// extract_paths_from_name_status unit tests
// ---------------------------------------------------------------------------

#[test]
fn extract_paths_handles_normal_status_lines() {
    use super::extract_paths_from_name_status;

    let output = "A\tlibs/domain/src/new.rs\nM\tapps/cli/src/main.rs\n";
    let paths = extract_paths_from_name_status(output);
    assert_eq!(paths, ["libs/domain/src/new.rs", "apps/cli/src/main.rs"]);
}

#[test]
fn extract_paths_includes_both_sides_of_rename() {
    use super::extract_paths_from_name_status;

    let output = "R100\tlibs/domain/src/old.rs\tdocs/old.rs\n";
    let paths = extract_paths_from_name_status(output);
    assert_eq!(paths, ["libs/domain/src/old.rs", "docs/old.rs"]);
}

#[test]
fn extract_paths_mixed_add_and_rename() {
    use super::extract_paths_from_name_status;

    let output = "A\ttrack/registry.md\nR095\tsrc/lib.rs\ttrack/lib.rs\nM\tCLAUDE.md\n";
    let paths = extract_paths_from_name_status(output);
    assert_eq!(paths, ["track/registry.md", "src/lib.rs", "track/lib.rs", "CLAUDE.md"]);
}

#[test]
fn extract_paths_rename_code_into_planning_dir_is_not_planning_only() {
    use super::{extract_paths_from_name_status, is_planning_only_path};

    // A code file renamed into docs/ — source path is still code
    let output = "R100\tlibs/domain/src/review.rs\tdocs/review.rs\n";
    let paths = extract_paths_from_name_status(output);
    assert!(!paths.iter().all(|p| is_planning_only_path(p)));
}

#[test]
fn extract_paths_empty_output() {
    use super::extract_paths_from_name_status;

    let paths = extract_paths_from_name_status("");
    assert!(paths.is_empty());
}

// ---------------------------------------------------------------------------
// review status: integration tests via execute_status
// ---------------------------------------------------------------------------

#[test]
fn status_succeeds_with_valid_track() {
    use super::{StatusArgs, execute_status};

    let dir = tempfile::tempdir().unwrap();
    let items_dir = dir.path().join("items");
    let track_dir = items_dir.join("test-track");
    fs::create_dir_all(&track_dir).unwrap();

    // Write a minimal metadata.json with review section
    let metadata = r#"{
  "schema_version": 3,
  "id": "test-track",
  "branch": "track/test-track",
  "title": "Test track",
  "status": "planned",
  "created_at": "2026-03-24T00:00:00Z",
  "updated_at": "2026-03-24T00:00:00Z",
  "tasks": [{"id": "T1", "description": "task", "status": "todo", "commit_hash": null}],
  "plan": {"summary": ["s"], "sections": [{"id": "S1", "title": "sec", "description": [], "task_ids": ["T1"]}]},
  "review": {"status": "not_started", "code_hash": null, "groups": {}}
}"#;
    fs::write(track_dir.join("metadata.json"), metadata).unwrap();

    let args = StatusArgs { items_dir: items_dir.clone(), track_id: "test-track".to_string() };
    let exit = execute_status(&args);
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}

#[test]
fn status_fails_for_missing_track() {
    use super::{StatusArgs, execute_status};

    let dir = tempfile::tempdir().unwrap();
    let items_dir = dir.path().join("items");
    fs::create_dir_all(&items_dir).unwrap();

    let args = StatusArgs { items_dir, track_id: "nonexistent".to_string() };
    let exit = execute_status(&args);
    assert_ne!(exit, std::process::ExitCode::SUCCESS);
}

#[test]
fn status_succeeds_without_review_section() {
    use super::{StatusArgs, execute_status};

    let dir = tempfile::tempdir().unwrap();
    let items_dir = dir.path().join("items");
    let track_dir = items_dir.join("test-track");
    fs::create_dir_all(&track_dir).unwrap();

    // metadata.json without review section (older track format)
    let metadata = r#"{
  "schema_version": 3,
  "id": "test-track",
  "branch": "track/test-track",
  "title": "Test track",
  "status": "planned",
  "created_at": "2026-03-24T00:00:00Z",
  "updated_at": "2026-03-24T00:00:00Z",
  "tasks": [{"id": "T1", "description": "task", "status": "todo", "commit_hash": null}],
  "plan": {"summary": ["s"], "sections": [{"id": "S1", "title": "sec", "description": [], "task_ids": ["T1"]}]}
}"#;
    fs::write(track_dir.join("metadata.json"), metadata).unwrap();

    let args = StatusArgs { items_dir, track_id: "test-track".to_string() };
    let exit = execute_status(&args);
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}

#[test]
fn status_displays_per_group_fast_final_state() {
    use super::{StatusArgs, execute_status};

    let dir = tempfile::tempdir().unwrap();
    let items_dir = dir.path().join("items");
    let track_dir = items_dir.join("test-track");
    fs::create_dir_all(&track_dir).unwrap();

    let metadata = r#"{
  "schema_version": 3,
  "id": "test-track",
  "branch": "track/test-track",
  "title": "Test track",
  "status": "in_progress",
  "created_at": "2026-03-24T00:00:00Z",
  "updated_at": "2026-03-24T00:00:00Z",
  "tasks": [{"id": "T1", "description": "task", "status": "in_progress", "commit_hash": null}],
  "plan": {"summary": ["s"], "sections": [{"id": "S1", "title": "sec", "description": [], "task_ids": ["T1"]}]},
  "review": {
    "status": "fast_passed",
    "code_hash": "abc123def456",
    "groups": {
      "usecase": {
        "fast": {"round": 2, "verdict": "zero_findings", "timestamp": "2026-03-24T01:00:00Z"}
      },
      "cli": {
        "fast": {"round": 1, "verdict": "findings_remain", "timestamp": "2026-03-24T00:30:00Z", "concerns": ["security"]},
        "final": {"round": 1, "verdict": "zero_findings", "timestamp": "2026-03-24T01:30:00Z"}
      }
    }
  }
}"#;
    fs::write(track_dir.join("metadata.json"), metadata).unwrap();

    let args = StatusArgs { items_dir, track_id: "test-track".to_string() };
    let exit = execute_status(&args);
    assert_eq!(exit, std::process::ExitCode::SUCCESS);
}
