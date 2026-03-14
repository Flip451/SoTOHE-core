//! CLI subcommands for local reviewer workflow wrappers.

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use clap::{ArgGroup, Args, Subcommand};
use usecase::review_workflow::{
    REVIEW_OUTPUT_SCHEMA_JSON, ReviewFinalMessageState, ReviewVerdict, classify_review_verdict,
    normalize_final_message, parse_review_final_message, render_review_payload,
};

#[cfg(unix)]
use std::os::unix::process::CommandExt;

const DEFAULT_TIMEOUT_SECONDS: u64 = 600;
const REVIEW_RUNTIME_DIR: &str = "tmp/reviewer-runtime";
const POLL_INTERVAL: Duration = Duration::from_millis(50);
#[cfg(test)]
const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

#[derive(Debug, Subcommand)]
pub enum ReviewCommand {
    /// Run the local Codex-backed reviewer through a repo-owned wrapper.
    CodexLocal(CodexLocalArgs),
}

#[derive(Debug, Args)]
#[command(group(
    ArgGroup::new("review_input")
        .required(true)
        .args(["briefing_file", "prompt"])
))]
pub struct CodexLocalArgs {
    /// Model name resolved from `.claude/agent-profiles.json`.
    #[arg(long)]
    model: String,

    /// Timeout for the reviewer subprocess in seconds.
    #[arg(long, default_value_t = DEFAULT_TIMEOUT_SECONDS)]
    timeout_seconds: u64,

    /// Path to a briefing file that the reviewer should read.
    #[arg(long)]
    briefing_file: Option<PathBuf>,

    /// Inline prompt for the reviewer.
    #[arg(long)]
    prompt: Option<String>,

    /// Test-only explicit path where the wrapper should ask Codex to write the final message.
    #[cfg(test)]
    #[arg(long, hide = true)]
    output_last_message: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ReviewRunResult {
    verdict: ReviewVerdict,
    final_message: Option<String>,
    output_last_message: PathBuf,
    output_last_message_auto_managed: bool,
    verdict_detail: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CodexInvocation {
    bin: OsString,
    args: Vec<OsString>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RenderedCommandResult {
    exit_code: u8,
    stdout_lines: Vec<String>,
    stderr_lines: Vec<String>,
}

pub fn execute(cmd: ReviewCommand) -> ExitCode {
    match cmd {
        ReviewCommand::CodexLocal(args) => execute_codex_local(&args),
    }
}

fn execute_codex_local(args: &CodexLocalArgs) -> ExitCode {
    let rendered = render_codex_local_result(args, run_codex_local(args));
    for line in rendered.stdout_lines {
        println!("{line}");
    }
    for line in rendered.stderr_lines {
        eprintln!("{line}");
    }
    ExitCode::from(rendered.exit_code)
}

fn render_codex_local_result(
    args: &CodexLocalArgs,
    outcome: Result<ReviewRunResult, String>,
) -> RenderedCommandResult {
    match outcome {
        Ok(result) => match result.verdict {
            ReviewVerdict::ZeroFindings => render_final_json_or_failure(
                result,
                0,
                "[ERROR] Local reviewer reported zero findings without a final JSON payload",
            ),
            ReviewVerdict::FindingsRemain => render_final_json_or_failure(
                result,
                2,
                "[ERROR] Local reviewer reported findings without a final JSON payload",
            ),
            ReviewVerdict::Timeout => RenderedCommandResult {
                exit_code: 1,
                stdout_lines: Vec::new(),
                stderr_lines: vec![render_missing_message_failure(
                    &format!("[TIMEOUT] Local reviewer exceeded {}s", args.timeout_seconds),
                    &result,
                )],
            },
            ReviewVerdict::ProcessFailed => {
                let mut stderr_lines = vec!["[ERROR] Local reviewer process failed".to_owned()];
                if let Some(detail) = result.verdict_detail {
                    stderr_lines.push(detail);
                }
                if let Some(message) = result.final_message {
                    stderr_lines.push(message);
                }
                RenderedCommandResult { exit_code: 1, stdout_lines: Vec::new(), stderr_lines }
            }
            ReviewVerdict::LastMessageMissing => RenderedCommandResult {
                exit_code: 1,
                stdout_lines: Vec::new(),
                stderr_lines: vec![render_missing_message_failure(
                    "[ERROR] Local reviewer finished without a final message",
                    &result,
                )],
            },
        },
        Err(err) => RenderedCommandResult {
            exit_code: 1,
            stdout_lines: Vec::new(),
            stderr_lines: vec![format!("local reviewer failed: {err}")],
        },
    }
}

fn render_final_json_or_failure(
    result: ReviewRunResult,
    success_exit_code: u8,
    missing_payload_message: &str,
) -> RenderedCommandResult {
    match result.final_message {
        Some(message) => RenderedCommandResult {
            exit_code: success_exit_code,
            stdout_lines: vec![message],
            stderr_lines: Vec::new(),
        },
        None => RenderedCommandResult {
            exit_code: 1,
            stdout_lines: Vec::new(),
            stderr_lines: vec![render_missing_message_failure(missing_payload_message, &result)],
        },
    }
}

fn render_missing_message_failure(prefix: &str, result: &ReviewRunResult) -> String {
    if result.output_last_message_auto_managed {
        prefix.to_owned()
    } else {
        format!("{prefix}: {}", result.output_last_message.display())
    }
}

fn run_codex_local(args: &CodexLocalArgs) -> Result<ReviewRunResult, String> {
    let prompt = build_prompt(args)?;
    #[cfg(test)]
    let explicit_output_last_message = args.output_last_message.as_deref();
    #[cfg(not(test))]
    let explicit_output_last_message: Option<&Path> = None;

    let output_last_message = prepare_output_last_message_path(explicit_output_last_message)?;
    let output_schema = prepare_output_schema_path()?;
    let _cleanup = AutoManagedArtifacts::new([&output_last_message, &output_schema]);
    reset_output_last_message(&output_last_message.path)?;
    write_output_schema(&output_schema.path)?;
    let invocation = build_codex_invocation(
        &args.model,
        &prompt,
        &output_last_message.path,
        &output_schema.path,
    );
    run_codex_invocation(
        &invocation,
        Duration::from_secs(args.timeout_seconds),
        output_last_message,
    )
}

fn build_prompt(args: &CodexLocalArgs) -> Result<String, String> {
    let prompt = if let Some(path) = &args.briefing_file {
        if !path.is_file() {
            return Err(format!("briefing file not found: {}", path.display()));
        }
        format!("Read {} and perform the task described there.", path.display())
    } else {
        args.prompt
            .clone()
            .ok_or_else(|| "either --briefing-file or --prompt is required".to_owned())?
    };

    Ok(prompt)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct OutputLastMessagePath {
    path: PathBuf,
    auto_managed: bool,
}

fn prepare_output_last_message_path(path: Option<&Path>) -> Result<OutputLastMessagePath, String> {
    let (path, auto_managed) = match path {
        Some(path) => (path.to_path_buf(), false),
        None => {
            let timestamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|err| format!("failed to compute timestamp: {err}"))?
                .as_millis();
            (
                PathBuf::from(REVIEW_RUNTIME_DIR)
                    .join(format!("codex-last-message-{}-{timestamp}.txt", std::process::id())),
                true,
            )
        }
    };

    let parent = path.parent().ok_or_else(|| {
        format!("output-last-message path must have a parent directory: {}", path.display())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;

    Ok(OutputLastMessagePath { path, auto_managed })
}

fn reset_output_last_message(path: &Path) -> Result<(), String> {
    std::fs::write(path, "").map_err(|err| {
        format!("failed to initialize reviewer final message {}: {err}", path.display())
    })
}

fn prepare_output_schema_path() -> Result<OutputLastMessagePath, String> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("failed to compute timestamp: {err}"))?
        .as_millis();
    let path = PathBuf::from(REVIEW_RUNTIME_DIR)
        .join(format!("codex-output-schema-{}-{timestamp}.json", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        format!("output-schema path must have a parent directory: {}", path.display())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|err| format!("failed to create {}: {err}", parent.display()))?;

    Ok(OutputLastMessagePath { path, auto_managed: true })
}

fn write_output_schema(path: &Path) -> Result<(), String> {
    std::fs::write(path, REVIEW_OUTPUT_SCHEMA_JSON)
        .map_err(|err| format!("failed to write reviewer output schema {}: {err}", path.display()))
}

#[derive(Debug)]
struct AutoManagedArtifacts {
    paths: Vec<PathBuf>,
}

impl AutoManagedArtifacts {
    fn new<'a>(artifacts: impl IntoIterator<Item = &'a OutputLastMessagePath>) -> Self {
        Self {
            paths: artifacts
                .into_iter()
                .filter(|artifact| artifact.auto_managed)
                .map(|artifact| artifact.path.clone())
                .collect(),
        }
    }
}

impl Drop for AutoManagedArtifacts {
    fn drop(&mut self) {
        for path in &self.paths {
            let _ = std::fs::remove_file(path);
        }
    }
}

fn build_codex_invocation(
    model: &str,
    prompt: &str,
    output_last_message: &Path,
    output_schema: &Path,
) -> CodexInvocation {
    let args = vec![
        OsString::from("exec"),
        OsString::from("--model"),
        OsString::from(model),
        OsString::from("--sandbox"),
        OsString::from("read-only"),
        OsString::from("--output-schema"),
        output_schema.as_os_str().to_os_string(),
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_os_string(),
        OsString::from(prompt),
    ];

    CodexInvocation { bin: codex_bin(), args }
}

fn codex_bin() -> OsString {
    #[cfg(test)]
    if let Some(value) = std::env::var_os(CODEX_BIN_ENV).filter(|value| !value.is_empty()) {
        return value;
    }

    OsString::from("codex")
}

fn spawn_codex(invocation: &CodexInvocation) -> Result<Child, String> {
    let mut command = Command::new(&invocation.bin);
    command
        .args(&invocation.args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());
    configure_child_process_group(&mut command);
    command
        .spawn()
        .map_err(|err| format!("failed to spawn {}: {err}", invocation.bin.to_string_lossy()))
}

fn run_codex_invocation(
    invocation: &CodexInvocation,
    timeout: Duration,
    output_last_message: OutputLastMessagePath,
) -> Result<ReviewRunResult, String> {
    let child = spawn_codex(invocation)?;
    run_codex_child(child, timeout, output_last_message)
}

fn run_codex_child(
    mut child: Child,
    timeout: Duration,
    output_last_message: OutputLastMessagePath,
) -> Result<ReviewRunResult, String> {
    let start = Instant::now();
    let mut timed_out = false;
    let mut exit_success = false;

    loop {
        match child.try_wait().map_err(|err| format!("failed to poll reviewer child: {err}"))? {
            Some(status) => {
                exit_success = status.success();
                break;
            }
            None => {
                if start.elapsed() >= timeout {
                    timed_out = true;
                    terminate_reviewer_child(&mut child)?;
                    child.wait().map_err(|err| format!("failed to reap reviewer child: {err}"))?;
                    break;
                }
                thread::sleep(POLL_INTERVAL);
            }
        }
    }

    let raw_final_message = read_final_message(&output_last_message.path)?;
    let final_message_state = parse_review_final_message(raw_final_message.as_deref());
    let final_message = match &final_message_state {
        ReviewFinalMessageState::Parsed(payload) => Some(render_review_payload(payload)?),
        _ => raw_final_message,
    };
    let verdict = classify_review_verdict(timed_out, exit_success, &final_message_state);
    let verdict_detail = match &final_message_state {
        ReviewFinalMessageState::Invalid { reason } => {
            Some(format!("invalid reviewer final payload: {reason}"))
        }
        _ => None,
    };

    Ok(ReviewRunResult {
        verdict,
        final_message,
        output_last_message: output_last_message.path,
        output_last_message_auto_managed: output_last_message.auto_managed,
        verdict_detail,
    })
}

fn read_final_message(path: &Path) -> Result<Option<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(content) => Ok(normalize_final_message(&content)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(format!("failed to read reviewer final message {}: {err}", path.display())),
    }
}

#[cfg(unix)]
fn configure_child_process_group(command: &mut Command) {
    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_child_process_group(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    let process_group = i32::try_from(child.id())
        .map_err(|_| format!("reviewer child pid does not fit into i32: {}", child.id()))?;
    // Safety: `killpg` is called with the child process group id created by `process_group(0)`.
    let result = unsafe { libc::killpg(process_group, libc::SIGKILL) };
    if result == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        if err.raw_os_error() == Some(libc::ESRCH) {
            Ok(())
        } else {
            Err(format!("failed to terminate reviewer child process group {process_group}: {err}"))
        }
    }
}

#[cfg(windows)]
fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    if child.try_wait().map_err(|err| format!("failed to poll reviewer child: {err}"))?.is_some() {
        return Ok(());
    }

    let status = Command::new("taskkill")
        .args(["/PID", &child.id().to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map_err(|err| {
            format!("failed to spawn taskkill for reviewer child {}: {err}", child.id())
        })?;
    if status.success() {
        Ok(())
    } else if child
        .try_wait()
        .map_err(|err| format!("failed to poll reviewer child after taskkill: {err}"))?
        .is_some()
    {
        Ok(())
    } else {
        Err(format!("failed to terminate reviewer child process tree {} via taskkill", child.id()))
    }
}

#[cfg(all(not(unix), not(windows)))]
fn terminate_reviewer_child(child: &mut Child) -> Result<(), String> {
    child.kill().map_err(|err| format!("failed to terminate reviewer child: {err}"))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::{
        CODEX_BIN_ENV, CodexLocalArgs, ReviewRunResult, build_codex_invocation, build_prompt,
        render_codex_local_result, run_codex_local,
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
    fn build_codex_invocation_uses_read_only_output_schema_without_full_auto() {
        let invocation = build_codex_invocation(
            "gpt-5.4",
            "Review this change.",
            Path::new("tmp/reviewer-runtime/out.txt"),
            Path::new("tmp/reviewer-runtime/schema.json"),
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
        assert!(rendered.windows(2).any(|pair| pair == ["--model", "gpt-5.4"]));
        assert!(!rendered.iter().any(|arg| arg == "--full-auto"));
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
        let _bin =
            EnvVarGuard::set(CODEX_BIN_ENV, std::ffi::OsStr::new("definitely-missing-codex"));

        let args = CodexLocalArgs {
            model: "gpt-5.4".to_owned(),
            timeout_seconds: 1,
            briefing_file: None,
            prompt: Some("Review this implementation.".to_owned()),
            output_last_message: None,
        };

        let err = run_codex_local(&args).unwrap_err();
        assert!(err.contains("failed to spawn"));

        let runtime_dir = dir.path().join("tmp/reviewer-runtime");
        let entries =
            if runtime_dir.is_dir() { fs::read_dir(runtime_dir).unwrap().count() } else { 0 };
        assert_eq!(entries, 0);
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
        let _message =
            EnvVarGuard::set("SOTP_FAKE_CODEX_MESSAGE", std::ffi::OsStr::new("NO_FINDINGS"));

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
        let _pid_file =
            EnvVarGuard::set("SOTP_FAKE_CODEX_CHILD_PID_FILE", child_pid_file.as_os_str());
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
        assert!(
            child_gone,
            "expected timed-out reviewer descendant {child_pid} to be gone or zombie"
        );
    }
}
