use crate::dry::RunDryFixLocalInput;
use crate::{CommandOutcome, error::CompositionError};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

// ── Per-context composition root ──────────────────────────────────────────────

/// Composition root for the `dry_fix_runner` command family.
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct DryFixRunnerCompositionRoot;

impl DryFixRunnerCompositionRoot {
    /// Create a new `DryFixRunnerCompositionRoot`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for DryFixRunnerCompositionRoot {
    fn default() -> Self {
        Self::new()
    }
}

impl DryFixRunnerCompositionRoot {
    pub fn dry_run_fix_local(
        &self,
        input: RunDryFixLocalInput,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
        use infrastructure::git_cli::{GitRepository, SystemGitRepo};
        let repo = SystemGitRepo::discover().map_err(|e| {
            CompositionError::AdapterInit(format!(
                "[ERROR] failed to discover git repository root: {e}"
            ))
        })?;
        let profiles_path = repo.root().join(AGENT_PROFILES_PATH);
        let profiles = AgentProfiles::load(&profiles_path).map_err(|e| {
            CompositionError::ConfigLoad(format!("[ERROR] failed to load agent-profiles.json: {e}"))
        })?;
        let track_id = ::domain::TrackId::try_new(input.track_id.trim())
            .map_err(|e| CompositionError::WiringFailed(format!("invalid --track-id: {e}")))?;
        let resolved =
            profiles.resolve_execution("dry-fix-lead", RoundType::Final).ok_or_else(|| {
                CompositionError::WiringFailed(
                    "[ERROR] dry-fix-lead capability not defined in agent-profiles.json".to_owned(),
                )
            })?;
        let model = input.model.clone().or_else(|| resolved.model.clone()).ok_or_else(|| {
            CompositionError::WiringFailed(
                "[ERROR] no model specified: pass --model or set model in agent-profiles.json \
             dry-fix-lead capability"
                    .to_owned(),
            )
        })?;
        eprintln!("[sotp dry fix-local] provider={} model={}", resolved.provider, &model);
        match resolved.provider.as_str() {
            "codex" => run_dry_fix_codex(&model, track_id.as_ref(), &input.briefing_file),
            other => Err(CompositionError::WiringFailed(format!(
                "[ERROR] unsupported dry-fix-lead provider '{other}' (supported: 'codex')"
            ))),
        }
    }
}

const DRY_FIX_SENTINEL_PREFIX: &str = "DRY_FIX_STATUS: ";
const DRY_FIX_REDACTED_ENV_VARS: &[&str] =
    &["OPENAI_API_KEY", "OPENAI_ORG_ID", "OPENAI_BASE_URL", "CODEX_API_KEY"];
pub(crate) fn run_dry_fix_codex(
    model: &str,
    track_id: &str,
    briefing_file: &Path,
) -> Result<CommandOutcome, CompositionError> {
    let codex_bin = resolve_codex_bin();
    let extra_path = bin_parent_dir(&codex_bin);
    dry_fix_smoke_test_forbidden_sandbox()?;
    let codex_home = dry_fix_resolve_codex_home()?;
    let safe_home = dry_fix_create_safe_home()?;
    let _home_cleanup = DryFixSafeHomeCleanup(safe_home.clone());
    let safe_env = dry_fix_build_safe_env(&safe_home, &codex_home, extra_path.as_deref())?;
    let smoke_env = dry_fix_build_smoke_env(&safe_env);
    dry_fix_smoke_test_codex_version(&codex_bin, &smoke_env)?;
    let prompt = build_dry_fix_prompt(track_id, briefing_file)?;
    let output_last_message = dry_fix_runtime_path("dry-fix-codex-last-message", "txt")?;
    std::fs::write(&output_last_message, "").map_err(|e| {
        CompositionError::Infrastructure(format!("failed to initialize last-message file: {e}"))
    })?;
    let _last_message_cleanup = DryFixLastMessageCleanup(output_last_message.clone());
    let args = build_dry_fix_invocation(model, &codex_home, &safe_home, &output_last_message);
    let (stdout, log_path) = dry_fix_spawn_and_collect(&codex_bin, &args, &safe_env, &prompt)?;
    let log_cleanup = DryFixSessionLogCleanup::new(log_path.clone());
    let last_message_content = match std::fs::read_to_string(&output_last_message) {
        Ok(content) => content,
        Err(e) => {
            log_cleanup.keep_for_diagnosis();
            return Err(CompositionError::Infrastructure(format!(
                "failed to read last-message file: {e}; log: {}",
                log_path.display()
            )));
        }
    };
    let status =
        parse_dry_fix_sentinel(&last_message_content).or_else(|| parse_dry_fix_sentinel(&stdout));
    let status = match status {
        Some(s) => s,
        None => {
            log_cleanup.keep_for_diagnosis();
            return Err(CompositionError::Infrastructure(format!(
                "no DRY_FIX_STATUS sentinel found in fixer output; log: {}",
                log_path.display()
            )));
        }
    };
    if status != "completed" {
        log_cleanup.keep_for_diagnosis();
    }
    let exit_code = dry_fix_sentinel_to_exit_code(status);
    Ok(CommandOutcome {
        stdout: Some(format!("DRY_FIX_STATUS: {status}")),
        stderr: None,
        exit_code: u8::try_from(exit_code).unwrap_or(1),
    })
}
pub(crate) fn resolve_codex_bin() -> OsString {
    if let Some(val) = std::env::var_os("CODEX_BIN").filter(|val| !val.is_empty()) {
        return resolve_codex_bin_candidate(val);
    }
    resolve_codex_bin_candidate(OsString::from("codex"))
}
fn resolve_codex_bin_candidate(candidate: OsString) -> OsString {
    let path = Path::new(&candidate);
    if path.is_absolute() || path.components().count() > 1 {
        return candidate;
    }
    resolve_codex_via_asdf()
        .or_else(|| resolve_executable_on_path(path))
        .map(|path| path.into_os_string())
        .unwrap_or(candidate)
}
fn resolve_codex_via_asdf() -> Option<PathBuf> {
    use std::process::{Command, Stdio};
    let asdf_bin = resolve_executable_on_path(Path::new("asdf"))?;
    let mut command = Command::new(asdf_bin);
    command.args(["which", "codex"]);
    command.stdin(Stdio::null());
    command.stdout(Stdio::piped());
    command.stderr(Stdio::null());
    command.env_clear();
    for (key, value) in dry_fix_asdf_lookup_env() {
        command.env(key, value);
    }
    let output = command.output().ok()?;
    if !output.status.success() {
        return None;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let path = stdout.lines().next()?.trim();
    if path.is_empty() { None } else { Some(PathBuf::from(path)) }
}
fn dry_fix_asdf_lookup_env() -> Vec<(OsString, OsString)> {
    const SAFE_VARS: &[&str] =
        &["PATH", "ASDF_DATA_DIR", "ASDF_CONFIG_FILE", "ASDF_DIR", "TMPDIR", "TEMP", "TMP"];
    SAFE_VARS
        .iter()
        .filter_map(|var| {
            let value = std::env::var_os(var).filter(|value| !value.is_empty())?;
            Some((OsString::from(*var), value))
        })
        .collect()
}
fn resolve_executable_on_path(executable: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(executable);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
fn bin_parent_dir(bin: &OsString) -> Option<PathBuf> {
    let p = Path::new(bin);
    if p.is_absolute() { p.parent().map(PathBuf::from) } else { None }
}
fn dry_fix_smoke_test_forbidden_sandbox() -> Result<(), CompositionError> {
    let val = std::env::var("CODEX_SANDBOX").unwrap_or_default();
    if matches!(val.as_str(), "danger-full-access" | "dangerously-bypass-approvals-and-sandbox") {
        return Err(CompositionError::Infrastructure(format!(
            "[ERROR] smoke test failed: forbidden sandbox override detected in environment: \
         CODEX_SANDBOX={val} — danger-full-access and \
         dangerously-bypass-approvals-and-sandbox are prohibited"
        )));
    }
    Ok(())
}
pub(crate) fn dry_fix_smoke_test_codex_version(
    bin: &OsString,
    safe_env: &[(OsString, OsString)],
) -> Result<(), CompositionError> {
    use std::process::{Command, Stdio};
    let mut cmd = Command::new(bin);
    cmd.arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.env_clear();
    for (key, value) in safe_env {
        cmd.env(key, value);
    }
    let output = cmd.output().map_err(|e| {
        CompositionError::Infrastructure(format!(
            "[ERROR] smoke test failed: codex CLI not found or failed to execute: {e}"
        ))
    })?;
    let combined = {
        let mut s = String::from_utf8_lossy(&output.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&output.stderr));
        s
    };
    let version_str = parse_semver_from_output(&combined).ok_or_else(|| {
        CompositionError::Infrastructure(
            "[ERROR] smoke test failed: cannot determine codex version from `codex --version` output"
                .to_owned(),
        )
    })?;
    let (major, minor) = parse_major_minor_version(&version_str).ok_or_else(|| {
        CompositionError::Infrastructure(format!(
            "[ERROR] smoke test failed: cannot parse codex version components from '{version_str}'"
        ))
    })?;
    if major > 0 {
        return Err(CompositionError::Infrastructure(format!(
            "[ERROR] smoke test failed: codex version {version_str} is outside validated range \
         (>= 0.115.0, < 1.0.0): major version upgrade requires re-validation"
        )));
    }
    if minor < 115 {
        return Err(CompositionError::Infrastructure(format!(
            "[ERROR] smoke test failed: codex version {version_str} is below minimum validated \
         version 0.115.0"
        )));
    }
    Ok(())
}
fn parse_semver_from_output(text: &str) -> Option<String> {
    for token in text.split_whitespace() {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() >= 3 && parts.iter().all(|p| p.chars().all(|c| c.is_ascii_digit())) {
            return Some(token.to_owned());
        }
    }
    None
}
fn parse_major_minor_version(version: &str) -> Option<(u32, u32)> {
    let mut parts = version.splitn(3, '.');
    let major = parts.next()?.parse::<u32>().ok()?;
    let minor = parts.next()?.parse::<u32>().ok()?;
    Some((major, minor))
}
fn prepend_dir_to_path(dir: &Path) -> Result<OsString, CompositionError> {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        if !existing.is_empty() {
            paths.extend(std::env::split_paths(&existing));
        }
    }
    std::env::join_paths(paths).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "failed to prepend {} to PATH: {e}",
            dir.display()
        ))
    })
}
fn dry_fix_resolve_codex_home() -> Result<PathBuf, CompositionError> {
    if let Ok(explicit) = std::env::var("CODEX_HOME") {
        if !explicit.is_empty() {
            let p = if let Some(rest) = explicit.strip_prefix("~/") {
                let home = std::env::var("HOME").map_err(|e| {
                    CompositionError::Infrastructure(format!(
                        "CODEX_HOME starts with ~/ but HOME not set: {e}"
                    ))
                })?;
                PathBuf::from(home).join(rest)
            } else if explicit == "~" {
                let home = std::env::var("HOME").map_err(|e| {
                    CompositionError::Infrastructure(format!(
                        "CODEX_HOME is ~ but HOME not set: {e}"
                    ))
                })?;
                PathBuf::from(home).join(".codex")
            } else {
                PathBuf::from(&explicit)
            };
            return dry_fix_make_absolute(p);
        }
    }
    let home = std::env::var("HOME").map_err(|e| {
        CompositionError::Infrastructure(format!(
            "HOME env var is not set (cannot resolve default CODEX_HOME): {e}"
        ))
    })?;
    dry_fix_make_absolute(PathBuf::from(home).join(".codex"))
}
fn dry_fix_make_absolute(path: PathBuf) -> Result<PathBuf, CompositionError> {
    if path.is_absolute() {
        return Ok(path);
    }
    let cwd = std::env::current_dir().map_err(|e| {
        CompositionError::Infrastructure(format!("failed to resolve current directory: {e}"))
    })?;
    Ok(cwd.join(path))
}
fn dry_fix_create_safe_home() -> Result<PathBuf, CompositionError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    for _ in 0..16_u8 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| {
                CompositionError::Infrastructure(format!("failed to compute timestamp: {e}"))
            })?
            .as_nanos();
        let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = dir.join(format!("dry-fix-codex-home-{}-{ts}-{seq}", std::process::id()));
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
                return Err(CompositionError::Infrastructure(format!(
                    "failed to create safe HOME {}: {e}",
                    path.display()
                )));
            }
        }
    }
    Err(CompositionError::Infrastructure(
        "failed to create a unique safe HOME after repeated attempts".to_owned(),
    ))
}
pub(crate) fn dry_fix_build_safe_env(
    safe_home: &Path,
    codex_home: &Path,
    extra_path_prefix: Option<&Path>,
) -> Result<Vec<(OsString, OsString)>, CompositionError> {
    #[rustfmt::skip]
    const BLOCKED: &[&str] = &[
        "GITHUB_TOKEN", "SSH_AUTH_SOCK", "GIT_SSH", "GIT_SSH_COMMAND",
        "SSH_CONNECTION", "SSH_CLIENT", "HOME", "CODEX_HOME",
    ];
    #[rustfmt::skip]
    const SAFE_VARS: &[&str] = &[
        "PATH", "USER", "LOGNAME", "TERM", "LANG", "LC_ALL", "TMPDIR", "TEMP", "TMP",
        "CARGO_HOME", "RUSTUP_HOME", "RUSTUP_TOOLCHAIN", "CARGO_TARGET_DIR",
        "DOCKER_HOST", "COMPOSE_PROJECT_NAME", "CLAUDE_PROJECT_DIR",
        "CARGO_MAKE_CURRENT_TASK_NAME",
        "OPENAI_API_KEY", "OPENAI_ORG_ID", "OPENAI_BASE_URL", "CODEX_API_KEY",
    ];
    let mut env: Vec<(OsString, OsString)> = Vec::new();
    env.push((OsString::from("GIT_SSH_COMMAND"), OsString::from("/bin/false")));
    env.push((OsString::from("HOME"), safe_home.as_os_str().to_os_string()));
    env.push((OsString::from("CODEX_HOME"), codex_home.as_os_str().to_os_string()));
    for &var in SAFE_VARS {
        if BLOCKED.contains(&var) {
            continue;
        }
        if var == "PATH" {
            let path_val = if let Some(prefix) = extra_path_prefix {
                prepend_dir_to_path(prefix)?
            } else if let Some(path) = std::env::var_os("PATH") {
                path
            } else {
                continue;
            };
            env.push((OsString::from("PATH"), path_val));
            continue;
        }
        if let Some(val) = std::env::var_os(var) {
            env.push((OsString::from(var), val));
        }
    }
    Ok(env)
}
pub(crate) fn dry_fix_build_smoke_env(
    safe_env: &[(OsString, OsString)],
) -> Vec<(OsString, OsString)> {
    safe_env
        .iter()
        .filter(|(key, _)| {
            key.to_str().map(|key| !DRY_FIX_REDACTED_ENV_VARS.contains(&key)).unwrap_or(true)
        })
        .cloned()
        .collect()
}
pub(crate) fn build_dry_fix_invocation(
    model: &str,
    codex_home: &Path,
    safe_home: &Path,
    output_last_message: &Path,
) -> Vec<OsString> {
    let writable_roots_config = dry_fix_writable_roots_config(&[codex_home, safe_home]);
    let mut args = vec![OsString::from("exec"), OsString::from("--model"), OsString::from(model)];
    args.extend([OsString::from("--sandbox"), OsString::from("workspace-write")]);
    args.extend([OsString::from("-c"), writable_roots_config]);
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
fn dry_fix_writable_roots_config(roots: &[&Path]) -> OsString {
    let roots = roots
        .iter()
        .map(|root| {
            let escaped = dry_fix_escape_config_string(&root.to_string_lossy());
            format!("\"{escaped}\"")
        })
        .collect::<Vec<_>>()
        .join(",");
    OsString::from(format!("sandbox_workspace_write.writable_roots=[{roots}]"))
}
fn dry_fix_escape_config_string(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}
fn build_dry_fix_prompt(track_id: &str, briefing_file: &Path) -> Result<String, CompositionError> {
    let briefing_path = briefing_file.to_str().ok_or_else(|| {
        CompositionError::Infrastructure(format!(
            "briefing_file path is not valid UTF-8: {}",
            briefing_file.display()
        ))
    })?;
    if briefing_path.is_empty()
        || briefing_path
            .chars()
            .any(|c| c == '`' || c.is_control() || matches!(c, '\u{2028}' | '\u{2029}'))
    {
        return Err(CompositionError::Infrastructure(format!(
            "briefing_file path contains characters that are unsafe in the fixer prompt: \
         {}",
            briefing_file.display()
        )));
    }
    let briefing_content = std::fs::read_to_string(briefing_file).map_err(|e| {
        CompositionError::Infrastructure(format!(
            "failed to read briefing file {briefing_path}: {e}"
        ))
    })?;
    let prompt = format!(
        "$dry-fix-lead\n\n\
     {briefing_content}\n\n\
     ---\n\n\
     ## Orchestrator Assignment\n\n\
     - Track ID: {track_id}\n\n\
     When you finish (DRY gate Approved, loop exhausted with violations remaining, \
     or tooling error), print EXACTLY one of these status lines as your final output \
     line, with no trailing text:\n\n\
     \x20\x20DRY_FIX_STATUS: completed\n\
     \x20\x20DRY_FIX_STATUS: blocked\n\
     \x20\x20DRY_FIX_STATUS: failed",
    );
    Ok(prompt)
}
fn dry_fix_runtime_path(prefix: &str, ext: &str) -> Result<PathBuf, CompositionError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CompositionError::Infrastructure(format!("failed to compute timestamp: {e}")))?
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = PathBuf::from("tmp/reviewer-runtime")
        .join(format!("{prefix}-{}-{timestamp}-{seq}.{ext}", std::process::id()));
    let parent = path.parent().ok_or_else(|| {
        CompositionError::Infrastructure(format!(
            "runtime path must have a parent: {}",
            path.display()
        ))
    })?;
    std::fs::create_dir_all(parent).map_err(|e| {
        CompositionError::Infrastructure(format!("failed to create {}: {e}", parent.display()))
    })?;
    Ok(path)
}
pub(crate) fn dry_fix_spawn_and_collect(
    bin: &OsString,
    args: &[OsString],
    safe_env: &[(OsString, OsString)],
    prompt: &str,
) -> Result<(String, PathBuf), CompositionError> {
    use std::io::Write as _;
    use std::process::{Command, Stdio};
    use std::thread;
    let log_path = dry_fix_runtime_path("dry-fix-codex-session", "log")?;
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
        CompositionError::Infrastructure(format!("failed to spawn Codex fixer: {e}"))
    })?;
    let redactions = dry_fix_redaction_values(safe_env);
    let stdout_pipe = child.stdout.take();
    let stdout_redactions = redactions.clone();
    let stdout_handle = thread::spawn(move || collect_pipe(stdout_pipe, false, &stdout_redactions));
    let stderr_pipe = child.stderr.take();
    let stderr_handle = thread::spawn(move || collect_pipe(stderr_pipe, true, &redactions));
    let prompt_result = match child.stdin.take() {
        Some(mut stdin) => stdin
            .write_all(prompt.as_bytes())
            .map_err(|e| format!("failed to write prompt to Codex fixer stdin: {e}")),
        None => Err("failed to open Codex fixer stdin pipe".to_owned()),
    };
    if let Err(msg) = prompt_result {
        let _ = child.kill();
        let _ = child.wait();
        let stdout = stdout_handle.join().ok().and_then(|r| r.ok()).unwrap_or_default();
        let stderr = stderr_handle.join().ok().and_then(|r| r.ok()).unwrap_or_default();
        write_dry_fix_log(&log_path, bin, "killed", &stdout, &stderr);
        return Err(CompositionError::Infrastructure(format!(
            "{msg}; log: {}",
            log_path.display()
        )));
    }
    let exit_status = child.wait().map_err(|e| {
        CompositionError::Infrastructure(format!("failed to wait for Codex fixer: {e}"))
    })?;
    let status_str = exit_status.to_string();
    let (stdout, stdout_error) =
        dry_fix_collector_result_for_log(join_dry_fix_collector(stdout_handle, "stdout"), "stdout");
    let (stderr, stderr_error) =
        dry_fix_collector_result_for_log(join_dry_fix_collector(stderr_handle, "stderr"), "stderr");
    write_dry_fix_log(&log_path, bin, &status_str, &stdout, &stderr);
    if let Some(error) = stdout_error.or(stderr_error) {
        return Err(CompositionError::Infrastructure(format!(
            "{}; log: {}",
            error,
            log_path.display()
        )));
    }
    Ok((stdout, log_path))
}
fn dry_fix_collector_result_for_log(
    result: Result<String, CompositionError>,
    stream_name: &str,
) -> (String, Option<CompositionError>) {
    match result {
        Ok(output) => (output, None),
        Err(error) => (format!("[failed to collect {stream_name}: {error}]\n"), Some(error)),
    }
}
fn join_dry_fix_collector(
    handle: std::thread::JoinHandle<Result<String, CompositionError>>,
    stream_name: &str,
) -> Result<String, CompositionError> {
    handle
        .join()
        .map_err(|_| {
            CompositionError::Infrastructure(format!("{stream_name} collector thread panicked"))
        })?
        .map_err(|e| {
            CompositionError::Infrastructure(format!("{stream_name} collection error: {e}"))
        })
}
fn collect_pipe<R: std::io::Read>(
    pipe: Option<R>,
    echo_to_stderr: bool,
    redactions: &[(String, String)],
) -> Result<String, CompositionError> {
    use std::io::{BufRead, BufReader};
    let mut collected = String::new();
    if let Some(pipe) = pipe {
        let reader = BufReader::new(pipe);
        for line in reader.lines() {
            let line = line.map_err(|e| {
                CompositionError::Infrastructure(format!("failed to read Codex fixer output: {e}"))
            })?;
            let line = redact_dry_fix_sensitive_text(&line, redactions);
            if echo_to_stderr {
                eprintln!("{line}");
            }
            collected.push_str(&line);
            collected.push('\n');
        }
    }
    Ok(collected)
}
fn dry_fix_redaction_values(safe_env: &[(OsString, OsString)]) -> Vec<(String, String)> {
    let mut values = safe_env
        .iter()
        .filter_map(|(key, value)| {
            let key = key.to_str()?.to_owned();
            if !DRY_FIX_REDACTED_ENV_VARS.contains(&key.as_str()) {
                return None;
            }
            let value = value.to_string_lossy();
            if value.is_empty() { None } else { Some((key, value.into_owned())) }
        })
        .collect::<Vec<_>>();
    values.sort_by(|(var_a, val_a), (var_b, val_b)| {
        val_b.len().cmp(&val_a.len()).then_with(|| var_a.cmp(var_b))
    });
    values
}
fn redact_dry_fix_sensitive_text(text: &str, redactions: &[(String, String)]) -> String {
    let mut redacted = text.to_owned();
    for (var, secret) in redactions {
        let placeholder = format!("[REDACTED:{var}]");
        redacted = redacted.replace(secret, &placeholder);
    }
    redacted
}
fn write_dry_fix_log(log_path: &Path, bin: &OsString, status: &str, stdout: &str, stderr: &str) {
    let content = format!(
        "bin: {}\nstatus: {}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        bin.to_string_lossy(),
        status,
        stdout,
        stderr
    );
    let _ = std::fs::write(log_path, content);
}
fn parse_dry_fix_sentinel(output: &str) -> Option<&'static str> {
    let last_line = output.lines().rev().find(|line| !line.trim().is_empty())?;
    if let Some(status) = last_line.strip_prefix(DRY_FIX_SENTINEL_PREFIX) {
        match status {
            "completed" => Some("completed"),
            "blocked" => Some("blocked"),
            "failed" => Some("failed"),
            _ => None,
        }
    } else {
        None
    }
}
fn dry_fix_sentinel_to_exit_code(status: &str) -> i32 {
    match status {
        "completed" => 0,
        "blocked" => 2,
        _ => 1,
    }
}
struct DryFixSafeHomeCleanup(PathBuf);
#[rustfmt::skip]
impl Drop for DryFixSafeHomeCleanup {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}
struct DryFixLastMessageCleanup(PathBuf);
#[rustfmt::skip]
impl Drop for DryFixLastMessageCleanup {
    fn drop(&mut self) { let _ = std::fs::remove_file(&self.0); }
}
pub(crate) struct DryFixSessionLogCleanup {
    path: PathBuf,
    remove_on_drop: bool,
}
impl DryFixSessionLogCleanup {
    pub(crate) fn new(path: PathBuf) -> Self {
        Self { path, remove_on_drop: true }
    }
    pub(crate) fn keep_for_diagnosis(mut self) {
        self.remove_on_drop = false;
    }
}
impl Drop for DryFixSessionLogCleanup {
    fn drop(&mut self) {
        if self.remove_on_drop {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}
