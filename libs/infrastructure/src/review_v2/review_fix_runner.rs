use std::ffi::OsString;
use std::io::{BufRead, BufReader, Write};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};
use usecase::review_v2::run_review_fix::{
    ReviewFixRunner, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixOutput,
};
const REVIEW_RUNTIME_DIR: &str = "tmp/reviewer-runtime";
#[cfg(test)]
pub(crate) const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";
pub struct CodexReviewFixRunner {
    model: String,
    #[cfg(test)]
    bin_override: Option<OsString>,
}
impl CodexReviewFixRunner {
    #[must_use]
    pub fn new(
        model: String,
        _scope: String,
        _briefing_file: Option<PathBuf>,
        _scope_files: Vec<PathBuf>,
    ) -> Self {
        Self {
            model,
            #[cfg(test)]
            bin_override: None,
        }
    }
    #[cfg(test)]
    pub(crate) fn with_bin(mut self, bin: impl Into<OsString>) -> Self {
        self.bin_override = Some(bin.into());
        self
    }
    fn smoke_test_forbidden_sandbox(&self) -> Result<(), ReviewFixRunnerError> {
        let val = std::env::var("CODEX_SANDBOX").unwrap_or_default();
        if is_forbidden_sandbox_value(&val) {
            return Err(ReviewFixRunnerError::SmokeTestFailed(format!(
                "forbidden sandbox override detected in environment: \
                 CODEX_SANDBOX={val} — danger-full-access and \
                 dangerously-bypass-approvals-and-sandbox are prohibited \
                 (ADR D3/CN-03)"
            )));
        }
        Ok(())
    }
    fn smoke_test_codex_version(&self, bin: &std::ffi::OsStr) -> Result<(), ReviewFixRunnerError> {
        let mut version_cmd = Command::new(bin);
        apply_parent_asdf_env(&mut version_cmd);
        apply_extra_path_prefix(&mut version_cmd, bin_parent_dir(bin))?;
        let output = version_cmd
            .arg("--version")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| {
                ReviewFixRunnerError::SmokeTestFailed(format!(
                    "codex CLI not found in PATH or failed to execute: {e}"
                ))
            })?;
        let combined = {
            let mut s = String::from_utf8_lossy(&output.stdout).into_owned();
            s.push_str(&String::from_utf8_lossy(&output.stderr));
            s
        };
        let version_str = parse_semver_from_text(&combined).ok_or_else(|| {
            ReviewFixRunnerError::SmokeTestFailed(
                "cannot determine codex version from `codex --version` output".to_owned(),
            )
        })?;
        let (major, minor) = parse_major_minor(&version_str).ok_or_else(|| {
            ReviewFixRunnerError::SmokeTestFailed(format!(
                "cannot parse codex version components from '{version_str}'"
            ))
        })?;
        if major > 0 {
            return Err(ReviewFixRunnerError::SmokeTestFailed(format!(
                "codex version {version_str} is outside validated range (>= 0.115.0, < 1.0.0): \
                 major version upgrade requires re-validation"
            )));
        }
        if minor < 115 {
            return Err(ReviewFixRunnerError::SmokeTestFailed(format!(
                "codex version {version_str} is below minimum validated version 0.115.0"
            )));
        }
        Ok(())
    }
}
fn is_forbidden_sandbox_value(val: &str) -> bool {
    matches!(val, "danger-full-access" | "dangerously-bypass-approvals-and-sandbox")
}
fn parse_semver_from_text(text: &str) -> Option<String> {
    for word in text.split_whitespace() {
        let candidate = word.trim_matches(|c: char| !c.is_ascii_digit());
        let parts: Vec<&str> = candidate.split('.').collect();
        let valid = parts.first().and_then(|p| p.parse::<u64>().ok()).is_some()
            && parts.get(1).and_then(|p| p.parse::<u64>().ok()).is_some()
            && parts.get(2).is_some_and(|p| p.chars().take_while(char::is_ascii_digit).count() > 0);
        if parts.len() >= 3 && valid {
            return Some(candidate.to_owned());
        }
    }
    None
}
fn parse_major_minor(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse::<u64>().ok()?;
    let minor = parts.next()?.parse::<u64>().ok()?;
    Some((major, minor))
}
fn codex_bin() -> OsString {
    #[cfg(test)]
    if let Some(value) = std::env::var_os(CODEX_BIN_ENV).filter(|v| !v.is_empty()) {
        return value;
    }
    OsString::from("codex")
}
fn resolve_codex_bin_path(bin: &std::ffi::OsStr) -> Result<OsString, ReviewFixRunnerError> {
    let bin_path = std::path::Path::new(bin);
    if bin_path.is_absolute() {
        return Ok(bin.to_os_string());
    }
    let mut asdf_cmd = Command::new("asdf");
    apply_parent_asdf_env(&mut asdf_cmd);
    let asdf_out = asdf_cmd
        .args(["which", &bin.to_string_lossy()])
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .ok();
    if let Some(out) = asdf_out {
        if out.status.success() {
            let resolved = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if !resolved.is_empty() {
                let p = std::path::Path::new(&resolved);
                if p.is_absolute() && p.exists() {
                    return Ok(OsString::from(resolved));
                }
            }
        }
    }
    let which_out = Command::new("which")
        .arg(bin)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .output()
        .ok();
    if let Some(out) = which_out {
        if out.status.success() {
            let resolved = String::from_utf8_lossy(&out.stdout).trim().to_owned();
            if !resolved.is_empty() {
                return Ok(OsString::from(resolved));
            }
        }
    }
    Ok(bin.to_os_string())
}
fn apply_parent_asdf_env(command: &mut Command) {
    for (key, value) in parent_asdf_env() {
        command.env(key, value);
    }
}
fn apply_extra_path_prefix(
    command: &mut Command,
    extra_path_prefix: Option<&Path>,
) -> Result<(), ReviewFixRunnerError> {
    if let Some(path) = path_with_optional_prefix(std::env::var_os("PATH"), extra_path_prefix)? {
        command.env("PATH", path);
    }
    Ok(())
}
fn bin_parent_dir(bin: &std::ffi::OsStr) -> Option<&Path> {
    let parent = std::path::Path::new(bin).parent()?;
    if parent.as_os_str().is_empty() {
        return None;
    }
    Some(parent)
}
fn parent_asdf_env() -> Vec<(OsString, OsString)> {
    parent_asdf_env_from(
        std::env::var_os("HOME"),
        std::env::var_os("ASDF_DATA_DIR"),
        std::env::var_os("ASDF_DIR"),
    )
}
fn parent_asdf_env_from(
    parent_home: Option<OsString>,
    asdf_data_dir: Option<OsString>,
    asdf_dir: Option<OsString>,
) -> Vec<(OsString, OsString)> {
    let parent_home = parent_home.map(PathBuf::from);
    let asdf_data_dir = asdf_data_dir
        .or_else(|| parent_home.as_ref().map(|home| home.join(".asdf").into_os_string()));
    let asdf_dir =
        asdf_dir.or_else(|| parent_home.as_ref().map(|home| home.join(".asdf").into_os_string()));
    let mut env = Vec::new();
    if let Some(value) = asdf_data_dir {
        env.push((OsString::from("ASDF_DATA_DIR"), value));
    }
    if let Some(value) = asdf_dir {
        env.push((OsString::from("ASDF_DIR"), value));
    }
    env
}
fn prompt_path_string(path: &Path, label: &str) -> Result<String, ReviewFixRunnerError> {
    let raw = path.to_str().ok_or_else(|| {
        ReviewFixRunnerError::Unexpected(format!("{label} path is not valid UTF-8"))
    })?;
    if raw.is_empty()
        || raw.chars().any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err(ReviewFixRunnerError::Unexpected(format!(
            "{label} path contains characters that are unsafe in the fixer prompt"
        )));
    }
    Ok(raw.to_owned())
}
fn scope_file_prompt_path(path: &Path) -> Result<String, ReviewFixRunnerError> {
    if path.is_absolute()
        || path.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir))
    {
        return Err(ReviewFixRunnerError::Unexpected(format!(
            "scope file path must be repository-relative without parent traversal: {}",
            path.display()
        )));
    }
    prompt_path_string(path, "scope file")
}
fn shell_quote_arg(raw: &str) -> String {
    if raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        return raw.to_owned();
    }
    format!("'{}'", raw.replace('\'', "'\\''"))
}
fn build_prompt(
    scope: &str,
    briefing_file: Option<&Path>,
    scope_files: &[PathBuf],
    command: &RunReviewFixCommand,
) -> Result<String, ReviewFixRunnerError> {
    let briefing_file = briefing_file.ok_or_else(|| {
        ReviewFixRunnerError::Unexpected(
            "briefing_file is required for review-fix runner".to_owned(),
        )
    })?;
    let briefing_path = prompt_path_string(briefing_file, "briefing_file")?;
    let briefing_content = std::fs::read_to_string(briefing_file).map_err(|e| {
        ReviewFixRunnerError::Unexpected(format!(
            "failed to read briefing file {}: {e}",
            briefing_path
        ))
    })?;
    let track_id = prompt_path_string(Path::new(&command.track_id), "track_id")?;
    let scope = prompt_path_string(Path::new(scope), "scope")?;
    let round_type = prompt_path_string(Path::new(&command.round_type), "round_type")?;
    let reviewer_model = prompt_path_string(Path::new(&command.reviewer_model), "reviewer_model")?;
    let scope_files_lines = if scope_files.is_empty() {
        "- (none provided; do not modify files unless the orchestrator reruns with an explicit boundary)"
            .to_owned()
    } else {
        scope_files
            .iter()
            .map(|p| scope_file_prompt_path(p).map(|safe| format!("- {safe}")))
            .collect::<Result<Vec<_>, _>>()?
            .join("\n")
    };
    let scope_files_section = format!(
        "\n\n## Scope File List (modification boundary)\n\n\
         You may ONLY modify files within this list:\n{scope_files_lines}"
    );
    let reviewer_invocation = format!(
        "cargo make track-local-review -- --model {} --round-type {} \
         --group {} --track-id {} --briefing-file {}",
        shell_quote_arg(&reviewer_model),
        shell_quote_arg(&round_type),
        shell_quote_arg(&scope),
        shell_quote_arg(&track_id),
        shell_quote_arg(&briefing_path),
    );
    let prompt = format!(
        "$review-fix-lead\n\n\
         {briefing_content}\n\n\
         ---\n\n\
         ## Orchestrator Assignment\n\n\
         - Track ID: {track_id}\n\
         - Scope: {scope}\n\
         - Round type: {round_type}\n\
         - Reviewer model: {reviewer_model}\n\
         - Reviewer invocation: {reviewer_invocation}\
         {scope_files_section}\n\n\
         When you finish (zero_findings confirmed or unrecoverable error), \
         print EXACTLY one of these status lines as your final output line, \
         with no trailing text:\n\n\
         \x20\x20REVIEW_FIX_STATUS: completed\n\
         \x20\x20REVIEW_FIX_STATUS: blocked_cross_scope\n\
         \x20\x20REVIEW_FIX_STATUS: failed",
        briefing_content = briefing_content,
        track_id = track_id,
        scope = scope,
        round_type = round_type,
        reviewer_model = reviewer_model,
        reviewer_invocation = reviewer_invocation,
        scope_files_section = scope_files_section,
    );
    Ok(prompt)
}
fn create_safe_home() -> Result<PathBuf, ReviewFixRunnerError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    for _ in 0..16 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                ReviewFixRunnerError::Unexpected(format!("failed to compute timestamp: {e}"))
            })?
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("review-fix-codex-home-{}-{ts}-{seq}", std::process::id()));
        #[cfg(unix)]
        let create_result = {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new().mode(0o700).create(&path)
        };
        #[cfg(not(unix))]
        let create_result = std::fs::create_dir(&path);
        match create_result {
            Ok(()) => return Ok(path),
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(e) => {
                return Err(ReviewFixRunnerError::Unexpected(format!(
                    "failed to create safe HOME {}: {e}",
                    path.display()
                )));
            }
        }
    }
    Err(ReviewFixRunnerError::Unexpected(
        "failed to create a unique safe HOME after repeated attempts".to_owned(),
    ))
}
fn make_absolute(path: PathBuf) -> Result<PathBuf, ReviewFixRunnerError> {
    if path.is_absolute() {
        return Ok(path);
    }
    let cwd = std::env::current_dir().map_err(|e| {
        ReviewFixRunnerError::Unexpected(format!(
            "failed to resolve current directory while absolutizing CODEX_HOME: {e}"
        ))
    })?;
    Ok(cwd.join(path))
}
fn resolve_codex_home() -> Result<PathBuf, ReviewFixRunnerError> {
    if let Ok(explicit) = std::env::var("CODEX_HOME") {
        if !explicit.is_empty() {
            if let Some(rest) = explicit.strip_prefix("~/") {
                let home = std::env::var("HOME").map_err(|e| {
                    ReviewFixRunnerError::Unexpected(format!(
                        "CODEX_HOME starts with ~/ but HOME is not set: {e}"
                    ))
                })?;
                return make_absolute(PathBuf::from(home).join(rest));
            }
            if explicit == "~" {
                let home = std::env::var("HOME").map_err(|e| {
                    ReviewFixRunnerError::Unexpected(format!(
                        "CODEX_HOME is ~ but HOME is not set: {e}"
                    ))
                })?;
                return make_absolute(PathBuf::from(home).join(".codex"));
            }
            return make_absolute(PathBuf::from(explicit));
        }
    }
    let home = std::env::var("HOME").map_err(|e| {
        ReviewFixRunnerError::Unexpected(format!(
            "HOME env var is not set (cannot resolve default CODEX_HOME): {e}"
        ))
    })?;
    make_absolute(PathBuf::from(home).join(".codex"))
}
fn build_codex_fixer_invocation(
    model: &str,
    codex_home: &Path,
    output_last_message: &Path,
) -> Vec<OsString> {
    let codex_home_str = codex_home.to_string_lossy();
    let codex_home_config = escape_config_string(codex_home_str.as_ref());
    let writable_roots_config =
        format!("sandbox_workspace_write.writable_roots=[\"{codex_home_config}\"]");
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    args.extend([OsString::from("--sandbox"), OsString::from("workspace-write")]);
    args.extend([OsString::from("-c"), OsString::from(writable_roots_config)]);
    args.extend([
        OsString::from("-c"),
        OsString::from("sandbox_workspace_write.network_access=true"),
    ]);
    args.extend([
        OsString::from("--output-last-message"),
        output_last_message.as_os_str().to_os_string(),
    ]);
    args
}
fn escape_config_string(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}
fn build_safe_env(
    safe_home: &Path,
    codex_home: &Path,
    extra_path_prefix: Option<&Path>,
) -> Result<Vec<(OsString, OsString)>, ReviewFixRunnerError> {
    #[rustfmt::skip]
    const BLOCKED: &[&str] = &["GITHUB_TOKEN", "SSH_AUTH_SOCK", "GIT_SSH", "GIT_SSH_COMMAND", "SSH_CONNECTION", "SSH_CLIENT", "HOME", "CODEX_HOME"];
    #[rustfmt::skip]
    const SAFE_VARS: &[&str] = &["PATH", "USER", "LOGNAME", "TERM", "LANG", "LC_ALL", "TMPDIR", "TEMP", "TMP", "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_TARGET_DIR", "DOCKER_HOST", "COMPOSE_PROJECT_NAME", "CLAUDE_PROJECT_DIR", "CARGO_MAKE_CURRENT_TASK_NAME", "OPENAI_API_KEY", "OPENAI_ORG_ID", "OPENAI_BASE_URL", "CODEX_API_KEY"];
    let mut env: Vec<(OsString, OsString)> = Vec::new();
    env.push((OsString::from("GIT_SSH_COMMAND"), OsString::from("/bin/false")));
    env.push((OsString::from("HOME"), safe_home.as_os_str().to_os_string()));
    env.push((OsString::from("CODEX_HOME"), codex_home.as_os_str().to_os_string()));
    env.extend(parent_asdf_env());
    for &var in SAFE_VARS {
        if BLOCKED.contains(&var) {
            continue;
        }
        if var == "PATH" {
            if let Some(path) =
                path_with_optional_prefix(std::env::var_os("PATH"), extra_path_prefix)?
            {
                env.push((OsString::from("PATH"), path));
            }
            continue;
        }
        if let Some(val) = std::env::var_os(var) {
            env.push((OsString::from(var), val));
        }
    }
    Ok(env)
}
fn path_with_optional_prefix(
    current_path: Option<OsString>,
    extra_path_prefix: Option<&Path>,
) -> Result<Option<OsString>, ReviewFixRunnerError> {
    match (extra_path_prefix, current_path) {
        (Some(prefix), Some(path)) => {
            let mut paths = vec![prefix.to_path_buf()];
            paths.extend(std::env::split_paths(&path));
            let joined = std::env::join_paths(paths).map_err(|e| {
                ReviewFixRunnerError::Unexpected(format!(
                    "failed to build PATH for codex fixer: {e}"
                ))
            })?;
            Ok(Some(joined))
        }
        (Some(prefix), None) => Ok(Some(prefix.as_os_str().to_os_string())),
        (None, Some(path)) => Ok(Some(path)),
        (None, None) => Ok(None),
    }
}
fn fixer_runtime_path(prefix: &str, ext: &str) -> Result<PathBuf, ReviewFixRunnerError> {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| ReviewFixRunnerError::Unexpected(format!("failed to compute timestamp: {e}")))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from(REVIEW_RUNTIME_DIR)
        .join(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        ReviewFixRunnerError::Unexpected(format!(
            "runtime path must have a parent directory: {}",
            path.display()
        ))
    })?;
    std::fs::create_dir_all(parent).map_err(|e| {
        ReviewFixRunnerError::Unexpected(format!("failed to create {}: {e}", parent.display()))
    })?;
    Ok(path)
}
fn spawn_and_collect_codex(
    bin: &std::ffi::OsStr,
    args: &[OsString],
    safe_env: &[(OsString, OsString)],
    prompt: &str,
) -> Result<(String, PathBuf), ReviewFixRunnerError> {
    let log_path = fixer_runtime_path("review-fix-codex-session", "log")?;
    let mut command = Command::new(bin);
    command.args(args);
    command.env_clear();
    for (k, v) in safe_env {
        command.env(k, v);
    }
    command.stdin(Stdio::piped());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::piped());
    let mut child = command.spawn().map_err(|e| {
        ReviewFixRunnerError::SpawnFailed(format!("failed to spawn codex fixer: {e}"))
    })?;
    let stdout_pipe = child.stdout.take();
    let stdout_handle = thread::spawn(move || collect_output_pipe(stdout_pipe, false, "stdout"));
    let stderr_pipe = child.stderr.take();
    let stderr_handle = thread::spawn(move || collect_output_pipe(stderr_pipe, true, "stderr"));
    let prompt_write_result = match child.stdin.take() {
        Some(mut stdin) => stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("failed to write prompt to codex fixer stdin: {e}")),
        None => Err("failed to open codex fixer stdin pipe".to_owned()),
    };
    if let Err(message) = prompt_write_result {
        let _ = child.kill();
        let exit_status = child.wait().map_or_else(
            |e| format!("failed to wait after prompt write error: {e}"),
            |s| s.to_string(),
        );
        let (stdout, _) =
            collector_result_for_log(join_output_collector(stdout_handle, "stdout"), "stdout");
        let (stderr, _) =
            collector_result_for_log(join_output_collector(stderr_handle, "stderr"), "stderr");
        write_session_log(&log_path, bin, &exit_status, &stdout, &stderr);
        return Err(ReviewFixRunnerError::SpawnFailed(format!(
            "{message}; session log: {}",
            log_path.display()
        )));
    }
    let exit_status = child.wait().map_err(|e| {
        ReviewFixRunnerError::SpawnFailed(format!("failed to wait for codex fixer: {e}"))
    })?;
    let exit_status = exit_status.to_string();
    let (stdout, stdout_error) =
        collector_result_for_log(join_output_collector(stdout_handle, "stdout"), "stdout");
    let (stderr, stderr_error) =
        collector_result_for_log(join_output_collector(stderr_handle, "stderr"), "stderr");
    write_session_log(&log_path, bin, &exit_status, &stdout, &stderr);
    if let Some(error) = stdout_error.or(stderr_error) {
        return Err(ReviewFixRunnerError::Unexpected(format!(
            "{error}; session log: {}",
            log_path.display()
        )));
    }
    Ok((stdout, log_path))
}
/// Names of environment variables that carry authentication credentials and are
/// intentionally passed through to the nested Codex run via `build_safe_env`.
/// Any non-empty value for these vars must be redacted before writing to a
/// persistent log file (`.claude/rules/06-security.md`).
const CREDENTIAL_VARS: &[&str] =
    &["OPENAI_API_KEY", "CODEX_API_KEY", "OPENAI_ORG_ID", "OPENAI_BASE_URL"];

/// Replaces every non-empty credential value found in `text` with a
/// `[REDACTED:<VAR_NAME>]` placeholder.  Empty values are never replaced —
/// replacing an empty string would corrupt the entire log.
fn redact_credentials(text: &str) -> String {
    let values =
        CREDENTIAL_VARS.iter().filter_map(|&var| std::env::var(var).ok().map(|val| (var, val)));
    redact_credential_values(text, values)
}

fn redact_credential_values<'a>(
    text: &str,
    values: impl IntoIterator<Item = (&'a str, String)>,
) -> String {
    let mut values: Vec<(&str, String)> =
        values.into_iter().filter(|(_, val)| !val.is_empty()).collect();
    values.sort_by(|(var_a, val_a), (var_b, val_b)| {
        val_b.len().cmp(&val_a.len()).then_with(|| var_a.cmp(var_b))
    });
    let mut result = text.to_owned();
    for (var, val) in values {
        let placeholder = format!("[REDACTED:{var}]");
        result = result.replace(&val, &placeholder);
    }
    result
}

fn write_session_log(
    log_path: &Path,
    bin: &std::ffi::OsStr,
    exit_status: &str,
    stdout: &str,
    stderr: &str,
) {
    let bin_display = bin.to_string_lossy();
    let redacted_stdout = redact_credentials(stdout);
    let redacted_stderr = redact_credentials(stderr);
    let log_content = format!(
        "=== codex fixer session log ===\nbin: {bin_display}\nexit_status: {exit_status}\n\n\
         === STDOUT ===\n{redacted_stdout}\n\
         === STDERR ===\n{redacted_stderr}"
    );
    if let Err(e) = std::fs::write(log_path, &log_content) {
        eprintln!(
            "[review-fix-runner] warning: failed to write session log {}: {e}",
            log_path.display()
        );
    }
}
fn collector_result_for_log(
    result: Result<String, ReviewFixRunnerError>,
    stream_name: &str,
) -> (String, Option<ReviewFixRunnerError>) {
    match result {
        Ok(output) => (output, None),
        Err(error) => (format!("[failed to collect {stream_name}: {error}]\n"), Some(error)),
    }
}
fn collect_output_pipe<R: std::io::Read>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    stream_name: &str,
) -> Result<String, String> {
    let mut collected = String::new();
    if let Some(pipe) = pipe {
        let reader = BufReader::new(pipe);
        for line in reader.lines() {
            let line =
                line.map_err(|e| format!("failed to read codex fixer {stream_name}: {e}"))?;
            if echo_to_stderr {
                eprintln!("{line}");
            }
            collected.push_str(&line);
            collected.push('\n');
        }
    }
    Ok(collected)
}
fn join_output_collector(
    handle: thread::JoinHandle<Result<String, String>>,
    stream_name: &str,
) -> Result<String, ReviewFixRunnerError> {
    handle
        .join()
        .map_err(|_| {
            ReviewFixRunnerError::Unexpected(format!(
                "codex fixer {stream_name} collector thread panicked"
            ))
        })?
        .map_err(ReviewFixRunnerError::Unexpected)
}
fn parse_sentinel(output: &str) -> Option<&'static str> {
    let last_line = output.lines().rev().find(|line| !line.trim().is_empty())?;
    match last_line {
        "REVIEW_FIX_STATUS: completed" => Some("completed"),
        "REVIEW_FIX_STATUS: blocked_cross_scope" => Some("blocked_cross_scope"),
        "REVIEW_FIX_STATUS: failed" => Some("failed"),
        _ => None,
    }
}
fn sentinel_to_exit_code(status: &str) -> i32 {
    match status {
        "completed" => 0,
        "blocked_cross_scope" => 2,
        _ => 1,
    }
}
impl ReviewFixRunner for CodexReviewFixRunner {
    fn run_fix(
        &self,
        command: RunReviewFixCommand,
    ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
        let codex_home = resolve_codex_home()?;
        #[cfg(test)]
        let bin = self.bin_override.clone().unwrap_or_else(codex_bin);
        #[cfg(not(test))]
        let bin = codex_bin();
        self.smoke_test_forbidden_sandbox()?;
        let bin = resolve_codex_bin_path(&bin)?;
        self.smoke_test_codex_version(&bin)?;
        let prompt = build_prompt(
            &command.scope,
            command.briefing_file.as_deref(),
            &command.scope_files,
            &command,
        )?;
        let output_last_message = fixer_runtime_path("review-fix-codex-last-message", "txt")?;
        std::fs::write(&output_last_message, "").map_err(|e| {
            ReviewFixRunnerError::Unexpected(format!(
                "failed to initialize output-last-message {}: {e}",
                output_last_message.display()
            ))
        })?;
        let _last_message_cleanup = OutputLastMessageCleanup(output_last_message.clone());
        let safe_home = create_safe_home()?;
        let _home_cleanup = SafeHomeCleanup(safe_home.clone());
        if let Ok(real_home) = std::env::var("HOME") {
            let tv_src = PathBuf::from(&real_home).join(".tool-versions");
            if tv_src.exists() {
                let _ = std::fs::copy(&tv_src, safe_home.join(".tool-versions"));
            }
        }
        let bin_parent = bin_parent_dir(&bin);
        let safe_env = build_safe_env(&safe_home, &codex_home, bin_parent)?;
        let args = build_codex_fixer_invocation(&self.model, &codex_home, &output_last_message);
        let (stdout, log_path) = spawn_and_collect_codex(&bin, &args, &safe_env, &prompt)?;
        // By default the guard removes the log on drop. Disarm it on failure
        // paths so the log is retained for diagnosis.
        let log_cleanup = SessionLogCleanup::new(log_path.clone());
        let last_message_content = match std::fs::read_to_string(&output_last_message) {
            Ok(content) => content,
            Err(e) => {
                log_cleanup.keep_for_diagnosis();
                return Err(ReviewFixRunnerError::Unexpected(format!(
                    "failed to read output-last-message {}: {e}; session log: {}",
                    output_last_message.display(),
                    log_path.display()
                )));
            }
        };
        let status = parse_sentinel(&last_message_content).or_else(|| parse_sentinel(&stdout));
        let status = match status {
            Some(s) => s,
            None => {
                // Disarm the cleanup guard: log must persist so the caller can diagnose.
                log_cleanup.keep_for_diagnosis();
                return Err(ReviewFixRunnerError::SentinelNotFound(format!(
                    "no REVIEW_FIX_STATUS sentinel found; session log: {}",
                    log_path.display()
                )));
            }
        };
        if status != "completed" {
            log_cleanup.keep_for_diagnosis();
        }
        let exit_code = sentinel_to_exit_code(status);
        Ok(RunReviewFixOutput { status: status.to_owned(), exit_code })
    }
}
struct SafeHomeCleanup(PathBuf);
#[rustfmt::skip]
impl Drop for SafeHomeCleanup {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
struct OutputLastMessageCleanup(PathBuf);
#[rustfmt::skip]
impl Drop for OutputLastMessageCleanup {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}
/// Deletes the session log file on drop unless told to keep it.
///
/// Default behaviour is to remove the log when the guard is dropped (clean
/// successful run).  Call [`keep_for_diagnosis`] on the failure path so the
/// log survives for the caller to inspect.
///
/// [`keep_for_diagnosis`]: SessionLogCleanup::keep_for_diagnosis
struct SessionLogCleanup {
    path: PathBuf,
    /// When `true` (the default), drop removes the file.
    /// Set to `false` via `keep_for_diagnosis` to retain the file.
    remove_on_drop: bool,
}
impl SessionLogCleanup {
    fn new(path: PathBuf) -> Self {
        Self { path, remove_on_drop: true }
    }
    /// Prevents the log from being deleted on drop so it can be used for diagnosis.
    fn keep_for_diagnosis(mut self) {
        self.remove_on_drop = false;
    }
}
impl Drop for SessionLogCleanup {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn make_command() -> RunReviewFixCommand {
        RunReviewFixCommand {
            scope: "infrastructure".to_owned(),
            briefing_file: None,
            track_id: "review-fix-codex-rustify-2026-05-31".to_owned(),
            round_type: "fast".to_owned(),
            reviewer_model: "o4-mini".to_owned(),
            model: "gpt-5.5".to_owned(),
            scope_files: vec![],
        }
    }

    fn make_runner() -> CodexReviewFixRunner {
        CodexReviewFixRunner::new("gpt-5.5".to_owned(), "infrastructure".to_owned(), None, vec![])
    }

    // ── build_prompt ─────────────────────────────────────────────────────────

    #[test]
    fn test_build_prompt_accepts_empty_scope_files_as_empty_boundary() {
        let dir = tempfile::tempdir().unwrap();
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "briefing").unwrap();

        let prompt = build_prompt("infrastructure", Some(&briefing), &[], &make_command()).unwrap();

        assert!(prompt.contains("## Scope File List (modification boundary)"));
        assert!(prompt.contains("- (none provided; do not modify files"));
    }

    #[test]
    fn test_build_prompt_rejects_scope_file_path_with_newline() {
        let dir = tempfile::tempdir().unwrap();
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "briefing").unwrap();
        let scope_files = vec![PathBuf::from("libs/infrastructure/src/lib.rs\n- Cargo.toml")];

        let result = build_prompt("infrastructure", Some(&briefing), &scope_files, &make_command());

        assert!(matches!(result, Err(ReviewFixRunnerError::Unexpected(_))));
    }

    #[test]
    fn test_build_prompt_rejects_briefing_path_with_backtick() {
        let dir = tempfile::tempdir().unwrap();
        let briefing = dir.path().join("brief`ing.md");
        std::fs::write(&briefing, "briefing").unwrap();

        let result = build_prompt("infrastructure", Some(&briefing), &[], &make_command());

        assert!(matches!(result, Err(ReviewFixRunnerError::Unexpected(_))));
    }

    #[test]
    fn test_build_prompt_shell_quotes_scope_in_reviewer_invocation() {
        let dir = tempfile::tempdir().unwrap();
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "briefing").unwrap();

        let prompt = build_prompt("usecase cli", Some(&briefing), &[], &make_command()).unwrap();

        assert!(prompt.contains("--group 'usecase cli'"));
    }

    #[test]
    fn test_build_prompt_rejects_assignment_field_injection() {
        let dir = tempfile::tempdir().unwrap();
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "briefing").unwrap();
        let mut command = make_command();
        command.track_id = "review-fix\n- Scope: cli".to_owned();
        assert!(matches!(
            build_prompt("infrastructure", Some(&briefing), &[], &command),
            Err(ReviewFixRunnerError::Unexpected(_))
        ));
        let mut command = make_command();
        command.reviewer_model = "gpt-5.4-mini`".to_owned();
        assert!(matches!(
            build_prompt("infrastructure", Some(&briefing), &[], &command),
            Err(ReviewFixRunnerError::Unexpected(_))
        ));
        assert!(matches!(
            build_prompt("infra\n- Scope: cli", Some(&briefing), &[], &make_command()),
            Err(ReviewFixRunnerError::Unexpected(_))
        ));
    }

    // ── is_forbidden_sandbox_value ────────────────────────────────────────────
    //
    // The `#![forbid(unsafe_code)]` crate attribute prevents calling
    // `std::env::set_var` / `remove_var` (unsafe in Rust 2024) from tests.
    // We test the AC-07 requirement by exercising the pure helper
    // `is_forbidden_sandbox_value` directly — the same function the method
    // delegates to — rather than mutating the environment.

    #[test]
    fn test_is_forbidden_sandbox_value_danger_full_access_returns_true() {
        assert!(
            is_forbidden_sandbox_value("danger-full-access"),
            "danger-full-access must be identified as forbidden"
        );
    }

    #[test]
    fn test_is_forbidden_sandbox_value_dangerously_bypass_returns_true() {
        assert!(
            is_forbidden_sandbox_value("dangerously-bypass-approvals-and-sandbox"),
            "dangerously-bypass-approvals-and-sandbox must be identified as forbidden"
        );
    }

    #[test]
    fn test_is_forbidden_sandbox_value_workspace_write_returns_false() {
        assert!(
            !is_forbidden_sandbox_value("workspace-write"),
            "workspace-write must NOT be forbidden"
        );
    }

    #[test]
    fn test_is_forbidden_sandbox_value_read_only_returns_false() {
        assert!(!is_forbidden_sandbox_value("read-only"), "read-only must NOT be forbidden");
    }

    #[test]
    fn test_is_forbidden_sandbox_value_empty_returns_false() {
        assert!(!is_forbidden_sandbox_value(""), "empty string must NOT be forbidden");
    }

    // ── parse_semver_from_text ────────────────────────────────────────────────

    #[test]
    fn test_parse_semver_from_text_finds_version_in_typical_output() {
        let output = "codex 0.125.0 (abc123)";
        assert_eq!(parse_semver_from_text(output).as_deref(), Some("0.125.0"));
    }

    #[test]
    fn test_parse_semver_from_text_returns_none_for_empty() {
        assert!(parse_semver_from_text("").is_none());
    }

    #[test]
    fn test_parse_semver_from_text_returns_none_for_non_version_text() {
        assert!(parse_semver_from_text("no version here at all").is_none());
    }

    // ── parse_sentinel ────────────────────────────────────────────────────────

    #[test]
    fn test_parse_sentinel_completed_returns_completed() {
        let output = "some output\nREVIEW_FIX_STATUS: completed";
        assert_eq!(parse_sentinel(output), Some("completed"));
    }

    #[test]
    fn test_parse_sentinel_blocked_cross_scope_returns_blocked_cross_scope() {
        let output = "some output\nREVIEW_FIX_STATUS: blocked_cross_scope";
        assert_eq!(parse_sentinel(output), Some("blocked_cross_scope"));
    }

    #[test]
    fn test_parse_sentinel_failed_returns_failed() {
        let output = "some output\nREVIEW_FIX_STATUS: failed";
        assert_eq!(parse_sentinel(output), Some("failed"));
    }

    #[test]
    fn test_parse_sentinel_empty_output_returns_none() {
        assert_eq!(parse_sentinel(""), None);
    }

    #[test]
    fn test_parse_sentinel_whitespace_only_returns_none() {
        assert_eq!(parse_sentinel("   \n\n  "), None);
    }

    #[test]
    fn test_parse_sentinel_embedded_in_prose_not_last_line_returns_none() {
        // Sentinel embedded in prose on a non-last line must NOT match.
        let output = "REVIEW_FIX_STATUS: completed — but here is more text explaining things\n\
             followed by trailing lines that are not the sentinel";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_sentinel_with_trailing_text_does_not_match() {
        // Line has extra text after the sentinel value — must NOT match.
        let output = "REVIEW_FIX_STATUS: completed and some extra text";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_sentinel_with_trailing_space_does_not_match() {
        let output = "REVIEW_FIX_STATUS: completed ";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_sentinel_with_leading_space_does_not_match() {
        let output = " REVIEW_FIX_STATUS: completed";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_trailing_text_line_after_sentinel_returns_none() {
        let output = "REVIEW_FIX_STATUS: completed\nextra text after sentinel";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_trailing_blank_lines_do_not_mask_sentinel() {
        // Trailing blank or whitespace-only lines must not cause the sentinel to be missed.
        let output = "REVIEW_FIX_STATUS: completed\n  \n\t\n";
        assert_eq!(parse_sentinel(output), Some("completed"));
    }

    #[test]
    fn test_parse_sentinel_codex_footer_after_sentinel_returns_none() {
        let output = "some preamble\n\
             REVIEW_FIX_STATUS: completed\n\
             \n\
             [tokens: prompt=12345 completion=678 total=13023]";
        assert_eq!(parse_sentinel(output), None);
    }

    #[test]
    fn test_parse_sentinel_last_sentinel_wins_when_multiple_present() {
        // When the sentinel appears multiple times, the last occurrence wins.
        let output = "REVIEW_FIX_STATUS: failed\nsome text\nREVIEW_FIX_STATUS: completed";
        assert_eq!(parse_sentinel(output), Some("completed"));
    }

    // ── sentinel_to_exit_code ─────────────────────────────────────────────────

    #[test]
    fn test_sentinel_to_exit_code_completed_is_zero() {
        assert_eq!(sentinel_to_exit_code("completed"), 0);
    }

    #[test]
    fn test_sentinel_to_exit_code_blocked_cross_scope_is_two() {
        assert_eq!(sentinel_to_exit_code("blocked_cross_scope"), 2);
    }

    #[test]
    fn test_sentinel_to_exit_code_failed_is_one() {
        assert_eq!(sentinel_to_exit_code("failed"), 1);
    }

    // ── smoke_test_codex_version via fake binary ──────────────────────────────

    #[cfg(unix)]
    fn make_executable(script: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(script, perms).unwrap();
    }

    #[cfg(unix)]
    fn write_fake_codex(dir: &std::path::Path, version_output: &str) -> PathBuf {
        let script = dir.join("fake-codex.sh");
        let script_content = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"{version_output}\"; exit 0; fi\nexit 0\n"
        );
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    fn write_fake_codex_requiring_path_parent(dir: &std::path::Path) -> PathBuf {
        let script = dir.join("fake-codex-needs-path.sh");
        let script_content = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  parent="$(dirname "$0")"
  case ":$PATH:" in
    *":$parent:"*) echo "codex 0.125.0"; exit 0 ;;
    *) echo "missing binary parent on PATH" >&2; exit 8 ;;
  esac
fi
exit 0
"#;
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    fn write_fake_codex_runner(dir: &std::path::Path) -> PathBuf {
        let script = dir.join("fake-codex-runner.sh");
        let script_content = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "codex 0.125.0"
  exit 0
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
if [ -z "$out" ]; then
  echo "missing output-last-message" >&2
  exit 9
fi
prompt_file="${out}.prompt"
cat > "$prompt_file"
if [ ! -s "$prompt_file" ]; then
  echo "missing stdin prompt" >&2
  exit 8
fi
printf 'REVIEW_FIX_STATUS: completed\n' > "$out"
printf 'fake stdout\n'
exit 0
"#;
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    fn write_fake_codex_runner_without_sentinel(dir: &std::path::Path) -> PathBuf {
        let script = dir.join("fake-codex-no-sentinel.sh");
        let script_content = r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "codex 0.125.0"
  exit 0
fi
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    out="$2"
    shift 2
  else
    shift
  fi
done
cat >/dev/null
printf 'not a sentinel\n' > "$out"
printf 'fake stdout without sentinel\n'
exit 0
"#;
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    fn write_fake_codex_runner_with_status(
        dir: &std::path::Path,
        status: &str,
        marker: &str,
    ) -> PathBuf {
        let script = dir.join("fake-codex-status-runner.sh");
        let script_content = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "codex 0.125.0"
  exit 0
fi
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    out="$2"
    shift 2
  else
    shift
  fi
done
cat >/dev/null
printf 'REVIEW_FIX_STATUS: {status}\n' > "$out"
printf '{marker}\n'
exit 0
"#
        );
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    fn write_fake_codex_runner_removing_last_message(
        dir: &std::path::Path,
        marker: &str,
    ) -> PathBuf {
        let script = dir.join("fake-codex-removes-last-message.sh");
        let script_content = format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  echo "codex 0.125.0"
  exit 0
fi
out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    out="$2"
    shift 2
  else
    shift
  fi
done
cat >/dev/null
printf '{marker}\n'
rm -f "$out"
exit 0
"#
        );
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    fn retained_session_log_containing(marker: &str) -> Option<PathBuf> {
        let entries = std::fs::read_dir(REVIEW_RUNTIME_DIR).ok()?;
        for entry in entries.flatten() {
            let path = entry.path();
            let is_session_log = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("review-fix-codex-session-"))
                && path.extension().and_then(|ext| ext.to_str()) == Some("log");
            if is_session_log
                && std::fs::read_to_string(&path).is_ok_and(|content| content.contains(marker))
            {
                return Some(path);
            }
        }
        None
    }

    #[cfg(unix)]
    #[test]
    fn test_smoke_test_codex_version_valid_passes() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex(dir.path(), "codex 0.125.0");
        let runner = make_runner().with_bin(&fake);
        let result = runner.smoke_test_codex_version(fake.as_os_str());
        assert!(result.is_ok(), "expected Ok for valid version 0.125.0, got: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_smoke_test_codex_version_uses_binary_parent_path_prefix() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_requiring_path_parent(dir.path());
        let runner = make_runner().with_bin(&fake);

        let result = runner.smoke_test_codex_version(fake.as_os_str());

        assert!(
            result.is_ok(),
            "expected version smoke test to put binary parent on PATH, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_smoke_test_codex_version_too_old_returns_smoke_test_failed() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex(dir.path(), "codex 0.114.9");
        let runner = make_runner().with_bin(&fake);
        let result = runner.smoke_test_codex_version(fake.as_os_str());
        assert!(
            matches!(result, Err(ReviewFixRunnerError::SmokeTestFailed(_))),
            "expected SmokeTestFailed for version 0.114.9, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_smoke_test_codex_version_major_bump_returns_smoke_test_failed() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex(dir.path(), "codex 1.0.0");
        let runner = make_runner().with_bin(&fake);
        let result = runner.smoke_test_codex_version(fake.as_os_str());
        assert!(
            matches!(result, Err(ReviewFixRunnerError::SmokeTestFailed(_))),
            "expected SmokeTestFailed for major version 1.0.0, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_fake_codex_completed_returns_completed() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_runner(dir.path());
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "# Briefing\n").unwrap();
        let mut command = make_command();
        command.briefing_file = Some(briefing);
        command.scope_files =
            vec![PathBuf::from("libs/infrastructure/src/review_v2/review_fix_runner.rs")];
        let runner = make_runner().with_bin(&fake);

        let output = runner.run_fix(command).unwrap();

        assert_eq!(output.status, "completed");
        assert_eq!(output.exit_code, 0);
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_fake_codex_without_sentinel_returns_sentinel_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_runner_without_sentinel(dir.path());
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "# Briefing\n").unwrap();
        let mut command = make_command();
        command.briefing_file = Some(briefing);
        command.scope_files =
            vec![PathBuf::from("libs/infrastructure/src/review_v2/review_fix_runner.rs")];
        let runner = make_runner().with_bin(&fake);

        let result = runner.run_fix(command);

        match result {
            Err(ReviewFixRunnerError::SentinelNotFound(_)) => {}
            Err(other) => panic!("expected SentinelNotFound, got error: {other:?}"),
            Ok(output) => panic!("expected SentinelNotFound, got status: {}", output.status),
        }
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_failed_status_retains_session_log_for_diagnosis() {
        let dir = tempfile::tempdir().unwrap();
        let marker =
            format!("failed-status-marker-{}", dir.path().file_name().unwrap().to_string_lossy());
        let fake = write_fake_codex_runner_with_status(dir.path(), "failed", &marker);
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "# Briefing\n").unwrap();
        let mut command = make_command();
        command.briefing_file = Some(briefing);
        command.scope_files =
            vec![PathBuf::from("libs/infrastructure/src/review_v2/review_fix_runner.rs")];
        let runner = make_runner().with_bin(&fake);

        let output = runner.run_fix(command).unwrap();

        assert_eq!(output.status, "failed");
        assert_eq!(output.exit_code, 1);
        let log_path = retained_session_log_containing(&marker)
            .expect("failed status must retain the session log for diagnosis");
        std::fs::remove_file(log_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_last_message_read_error_retains_session_log_for_diagnosis() {
        let dir = tempfile::tempdir().unwrap();
        let marker = format!(
            "missing-last-message-marker-{}",
            dir.path().file_name().unwrap().to_string_lossy()
        );
        let fake = write_fake_codex_runner_removing_last_message(dir.path(), &marker);
        let briefing = dir.path().join("briefing.md");
        std::fs::write(&briefing, "# Briefing\n").unwrap();
        let mut command = make_command();
        command.briefing_file = Some(briefing);
        command.scope_files =
            vec![PathBuf::from("libs/infrastructure/src/review_v2/review_fix_runner.rs")];
        let runner = make_runner().with_bin(&fake);

        let result = runner.run_fix(command);

        match result {
            Err(ReviewFixRunnerError::Unexpected(message)) => {
                assert!(message.contains("failed to read output-last-message"));
            }
            Err(other) => panic!("expected Unexpected read error, got error: {other:?}"),
            Ok(output) => panic!("expected read error, got status: {}", output.status),
        }
        let log_path = retained_session_log_containing(&marker)
            .expect("read error must retain the session log for diagnosis");
        std::fs::remove_file(log_path).unwrap();
    }

    // ── build_codex_fixer_invocation ──────────────────────────────────────────

    fn dummy_output_last_message() -> PathBuf {
        PathBuf::from("/tmp/review-fix-codex-last-message-test.txt")
    }

    #[test]
    fn test_build_codex_fixer_invocation_contains_workspace_write_sandbox() {
        let codex_home = PathBuf::from("/home/user/.codex");
        let olm = dummy_output_last_message();
        let args = build_codex_fixer_invocation("gpt-5.5", &codex_home, &olm);
        let args_str: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let sandbox_pos = args_str.iter().position(|a| a == "--sandbox");
        assert!(sandbox_pos.is_some(), "--sandbox flag must be present");
        let sandbox_val = sandbox_pos
            .and_then(|pos| args_str.get(pos + 1))
            .expect("--sandbox flag must be followed by a value");
        assert_eq!(sandbox_val, "workspace-write", "sandbox must be workspace-write");
    }

    #[test]
    fn test_build_codex_fixer_invocation_contains_writable_roots_config() {
        let codex_home = PathBuf::from("/home/user/.codex");
        let olm = dummy_output_last_message();
        let args = build_codex_fixer_invocation("gpt-5.5", &codex_home, &olm);
        let args_str: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let has_writable_roots =
            args_str.iter().any(|a| a.contains("sandbox_workspace_write.writable_roots"));
        assert!(has_writable_roots, "writable_roots config must be present in args");
    }

    #[test]
    fn test_build_codex_fixer_invocation_escapes_writable_roots_config() {
        let codex_home = PathBuf::from("/tmp/a\"b\\c");
        let olm = dummy_output_last_message();
        let args = build_codex_fixer_invocation("gpt-5.5", &codex_home, &olm);
        let args_str: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let config = args_str
            .iter()
            .find(|a| a.contains("sandbox_workspace_write.writable_roots"))
            .expect("writable_roots config must be present");

        assert_eq!(config, "sandbox_workspace_write.writable_roots=[\"/tmp/a\\\"b\\\\c\"]");
    }

    #[test]
    fn test_build_codex_fixer_invocation_contains_network_access_config() {
        let codex_home = PathBuf::from("/home/user/.codex");
        let olm = dummy_output_last_message();
        let args = build_codex_fixer_invocation("gpt-5.5", &codex_home, &olm);
        let args_str: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let has_network =
            args_str.iter().any(|a| a.contains("sandbox_workspace_write.network_access=true"));
        assert!(has_network, "network_access=true config must be present in args");
    }

    #[test]
    fn test_build_codex_fixer_invocation_contains_output_last_message_flag() {
        let codex_home = PathBuf::from("/home/user/.codex");
        let olm = PathBuf::from("/tmp/review-fix-test-last-message.txt");
        let args = build_codex_fixer_invocation("gpt-5.5", &codex_home, &olm);
        let args_str: Vec<String> = args.iter().map(|a| a.to_string_lossy().into_owned()).collect();
        let olm_pos = args_str.iter().position(|a| a == "--output-last-message");
        assert!(olm_pos.is_some(), "--output-last-message flag must be present");
        let olm_val = olm_pos
            .and_then(|pos| args_str.get(pos + 1))
            .expect("--output-last-message flag must be followed by a value");
        assert_eq!(
            olm_val, "/tmp/review-fix-test-last-message.txt",
            "--output-last-message must point to the provided path"
        );
    }

    #[test]
    fn test_build_codex_fixer_invocation_has_no_prompt_positional_argument() {
        let codex_home = PathBuf::from("/home/user/.codex");
        let olm = dummy_output_last_message();
        let args = build_codex_fixer_invocation("gpt-5.5", &codex_home, &olm);
        let olm_pos = args.iter().position(|a| a == "--output-last-message");
        let expected_len = olm_pos.map_or(0, |pos| pos + 2);
        assert_eq!(
            args.len(),
            expected_len,
            "prompt must be delivered through stdin, not appended to argv"
        );
    }

    #[test]
    fn test_create_safe_home_returns_unique_directories() {
        let first = create_safe_home().expect("first safe HOME should be created");
        let second = create_safe_home().expect("second safe HOME should be created");

        assert_ne!(first, second, "safe HOME directories must be unique per run");
        assert!(first.is_dir(), "first safe HOME should exist");
        assert!(second.is_dir(), "second safe HOME should exist");

        let _ = std::fs::remove_dir_all(first);
        let _ = std::fs::remove_dir_all(second);
    }

    // ── build_safe_env ────────────────────────────────────────────────────────

    #[test]
    fn test_build_safe_env_excludes_github_token() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let keys: Vec<String> = env.iter().map(|(k, _)| k.to_string_lossy().into_owned()).collect();
        assert!(!keys.contains(&"GITHUB_TOKEN".to_owned()), "GITHUB_TOKEN must be excluded");
    }

    #[test]
    fn test_build_safe_env_excludes_ssh_auth_sock() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let keys: Vec<String> = env.iter().map(|(k, _)| k.to_string_lossy().into_owned()).collect();
        assert!(!keys.contains(&"SSH_AUTH_SOCK".to_owned()), "SSH_AUTH_SOCK must be excluded");
    }

    #[test]
    fn test_build_safe_env_sets_git_ssh_command_to_false() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let git_ssh_cmd = env
            .iter()
            .find(|(k, _)| k == "GIT_SSH_COMMAND")
            .map(|(_, v)| v.to_string_lossy().into_owned());
        assert_eq!(
            git_ssh_cmd.as_deref(),
            Some("/bin/false"),
            "GIT_SSH_COMMAND must be /bin/false"
        );
    }

    #[test]
    fn test_build_safe_env_sets_home_to_safe_home() {
        let safe_home = PathBuf::from("/tmp/my-safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let home_val =
            env.iter().find(|(k, _)| k == "HOME").map(|(_, v)| v.to_string_lossy().into_owned());
        assert_eq!(
            home_val.as_deref(),
            Some("/tmp/my-safe-home"),
            "HOME must be the safe home dir"
        );
    }

    #[test]
    fn test_build_safe_env_sets_codex_home_to_real_codex_home() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let codex_home_val = env
            .iter()
            .find(|(k, _)| k == "CODEX_HOME")
            .map(|(_, v)| v.to_string_lossy().into_owned());
        assert_eq!(
            codex_home_val.as_deref(),
            Some("/home/user/.codex"),
            "CODEX_HOME must be real codex home"
        );
    }

    #[test]
    fn test_path_with_optional_prefix_uses_prefix_without_parent_path() {
        let actual = path_with_optional_prefix(None, Some(Path::new("/opt/codex/bin"))).unwrap();

        assert_eq!(actual, Some(std::ffi::OsString::from("/opt/codex/bin")));
    }

    #[test]
    fn test_path_with_optional_prefix_prepends_prefix_to_parent_path() {
        let actual = path_with_optional_prefix(
            Some(std::ffi::OsString::from("/usr/bin")),
            Some(Path::new("/opt/codex/bin")),
        )
        .unwrap();
        let expected =
            std::env::join_paths([PathBuf::from("/opt/codex/bin"), PathBuf::from("/usr/bin")])
                .unwrap();

        assert_eq!(actual, Some(expected));
    }

    #[test]
    fn test_parent_asdf_env_from_defaults_to_parent_home_asdf() {
        let env = parent_asdf_env_from(Some(OsString::from("/home/parent")), None, None);

        assert_eq!(
            env,
            vec![
                (OsString::from("ASDF_DATA_DIR"), OsString::from("/home/parent/.asdf")),
                (OsString::from("ASDF_DIR"), OsString::from("/home/parent/.asdf")),
            ]
        );
    }

    #[test]
    fn test_parent_asdf_env_from_preserves_explicit_asdf_values() {
        let env = parent_asdf_env_from(
            Some(OsString::from("/home/parent")),
            Some(OsString::from("/custom/data")),
            Some(OsString::from("/custom/dir")),
        );

        assert_eq!(
            env,
            vec![
                (OsString::from("ASDF_DATA_DIR"), OsString::from("/custom/data")),
                (OsString::from("ASDF_DIR"), OsString::from("/custom/dir")),
            ]
        );
    }

    #[test]
    fn test_build_safe_env_sets_asdf_data_dir_with_parent_home_default() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let actual = env.iter().find(|(k, _)| k == "ASDF_DATA_DIR").map(|(_, v)| v.clone());
        let expected = std::env::var_os("ASDF_DATA_DIR").or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".asdf").into_os_string())
        });

        assert_eq!(actual, expected);
    }

    #[test]
    fn test_build_safe_env_sets_asdf_dir_with_parent_home_default() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let actual = env.iter().find(|(k, _)| k == "ASDF_DIR").map(|(_, v)| v.clone());
        let expected = std::env::var_os("ASDF_DIR").or_else(|| {
            std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".asdf").into_os_string())
        });

        assert_eq!(actual, expected);
    }

    // ── redact_credentials ────────────────────────────────────────────────────

    /// Run `redact_credentials` in isolation without mutating the real env
    /// (Rust 2024 forbids `std::env::set_var` inside tests due to
    /// `#![forbid(unsafe_code)]`).  We test the pure helper directly by
    /// constructing a list of `(name, value)` pairs and applying the same
    /// replacement logic.
    fn apply_redaction_with(text: &str, pairs: &[(&str, &str)]) -> String {
        redact_credential_values(text, pairs.iter().map(|(var, val)| (*var, (*val).to_owned())))
    }

    #[test]
    fn test_redact_credentials_replaces_non_empty_value_in_captured_output() {
        let fake_key = "sk-FAKE-SECRET-123456";
        let captured = format!("Running codex...\nAuthorization: Bearer {fake_key}\nDone.");

        let redacted = apply_redaction_with(&captured, &[("OPENAI_API_KEY", fake_key)]);

        assert!(
            !redacted.contains(fake_key),
            "redacted output must not contain the original secret value"
        );
        assert!(
            redacted.contains("[REDACTED:OPENAI_API_KEY]"),
            "redacted output must contain the placeholder"
        );
    }

    #[test]
    fn test_redact_credentials_with_empty_value_does_not_corrupt_output() {
        let captured = "Running codex...\nNo secret here.\nDone.";

        // Empty value: must be skipped to avoid replacing every empty-string match.
        let redacted = apply_redaction_with(captured, &[("OPENAI_API_KEY", "")]);

        assert_eq!(
            redacted, captured,
            "empty credential value must leave the output completely unchanged"
        );
    }

    #[test]
    fn test_redact_credentials_handles_multiple_vars_independently() {
        let key_val = "sk-FAKE-OPENAI-KEY";
        let codex_val = "ck-FAKE-CODEX-KEY";
        let org_val = "org-FAKE-ORG";
        let base_url_val = "https://token@example.invalid/v1";
        let captured = format!(
            "key={key_val} codex={codex_val} org={org_val} base={base_url_val} other=plaintext"
        );

        let redacted = apply_redaction_with(
            &captured,
            &[
                ("OPENAI_API_KEY", key_val),
                ("CODEX_API_KEY", codex_val),
                ("OPENAI_ORG_ID", org_val),
                ("OPENAI_BASE_URL", base_url_val),
            ],
        );

        assert!(!redacted.contains(key_val), "OPENAI_API_KEY value must be redacted");
        assert!(!redacted.contains(codex_val), "CODEX_API_KEY value must be redacted");
        assert!(!redacted.contains(org_val), "OPENAI_ORG_ID value must be redacted");
        assert!(!redacted.contains(base_url_val), "OPENAI_BASE_URL value must be redacted");
        assert!(redacted.contains("other=plaintext"), "non-credential content must be preserved");
        assert!(redacted.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(redacted.contains("[REDACTED:CODEX_API_KEY]"));
        assert!(redacted.contains("[REDACTED:OPENAI_ORG_ID]"));
        assert!(redacted.contains("[REDACTED:OPENAI_BASE_URL]"));
    }

    #[test]
    fn test_redact_credentials_replaces_longest_overlapping_value_first() {
        let short_val = "sk-overlap";
        let long_val = "sk-overlap-secret";
        let captured = format!("short={short_val} long={long_val}");

        let redacted = apply_redaction_with(
            &captured,
            &[("OPENAI_API_KEY", short_val), ("CODEX_API_KEY", long_val)],
        );

        assert!(!redacted.contains(short_val), "short credential value must be redacted");
        assert!(!redacted.contains(long_val), "long credential value must be redacted");
        assert!(
            !redacted.contains("-secret"),
            "suffix of overlapping credential value must not leak"
        );
        assert!(redacted.contains("[REDACTED:OPENAI_API_KEY]"));
        assert!(redacted.contains("[REDACTED:CODEX_API_KEY]"));
    }

    #[test]
    fn test_credential_vars_include_all_auth_safe_vars() {
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_API_KEY"));
        assert!(CREDENTIAL_VARS.contains(&"CODEX_API_KEY"));
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_ORG_ID"));
        assert!(CREDENTIAL_VARS.contains(&"OPENAI_BASE_URL"));
    }

    // ── make_command and make_runner are needed for unused-variable lint ──────

    #[test]
    fn test_make_command_and_runner_compile() {
        let _cmd = make_command();
        let _runner = make_runner();
    }
}
