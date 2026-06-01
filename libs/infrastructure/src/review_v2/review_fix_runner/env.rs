use std::ffi::OsString;
use std::path::{Path, PathBuf};
use usecase::review_v2::run_review_fix::ReviewFixRunnerError;

pub(super) fn create_safe_home() -> Result<PathBuf, ReviewFixRunnerError> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

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

pub(super) fn make_absolute(path: PathBuf) -> Result<PathBuf, ReviewFixRunnerError> {
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

pub(super) fn resolve_codex_home() -> Result<PathBuf, ReviewFixRunnerError> {
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

pub(super) fn build_codex_fixer_invocation(
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

pub(super) fn escape_config_string(raw: &str) -> String {
    raw.replace('\\', "\\\\").replace('"', "\\\"")
}

/// Prepend `dir` to the current `PATH` environment variable and return the
/// result. When `PATH` is unset, the returned value contains only `dir`.
///
/// This is a pure helper used so that a colocated runtime (e.g. `node` living
/// alongside the real codex binary) can be found under the sanitized env.
/// The helper knows nothing about which toolchain manager placed the binary
/// there — it simply prepends an arbitrary directory to PATH.
pub(super) fn prepend_dir_to_path(dir: &Path) -> Result<OsString, ReviewFixRunnerError> {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        if !existing.is_empty() {
            paths.extend(std::env::split_paths(&existing));
        }
    }
    std::env::join_paths(paths).map_err(|e| {
        ReviewFixRunnerError::Unexpected(format!(
            "failed to prepend {} to PATH for codex fixer: {e}",
            dir.display()
        ))
    })
}

pub(super) fn build_safe_env(
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
    for &var in SAFE_VARS {
        if BLOCKED.contains(&var) {
            continue;
        }
        if var == "PATH" {
            let path_val = if let Some(prefix) = extra_path_prefix {
                // Prepend the binary's parent dir so a colocated runtime is found first.
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

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
    fn test_build_safe_env_passes_through_path() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let env = build_safe_env(&safe_home, &codex_home, None).unwrap();
        let has_path = env.iter().any(|(k, _)| k == "PATH");
        // PATH passthrough is present when the environment has PATH set (which it always does in CI)
        if std::env::var_os("PATH").is_some() {
            assert!(has_path, "PATH must be passed through when set in the environment");
        }
    }

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

    // ── prepend_dir_to_path ───────────────────────────────────────────────────

    #[test]
    fn test_prepend_dir_to_path_produces_dir_colon_existing() {
        // Test the pure logic: given a known PATH value, verify the dir is prepended.
        // We exercise prepend_dir_to_path indirectly via build_safe_env which reads
        // the real PATH, but we also test the helper directly with a fabricated input
        // by calling it and checking the prefix is present.
        let dir = Path::new("/fake/prefix/bin");
        let result = prepend_dir_to_path(dir).unwrap();
        let mut paths = std::env::split_paths(&result);
        let first = paths.next();
        assert_eq!(first.as_deref(), Some(dir), "prepended dir must come first");
        if std::env::var_os("PATH").is_some_and(|path| !path.is_empty()) {
            assert!(paths.next().is_some(), "existing PATH entries must be preserved");
        }
    }

    #[test]
    fn test_build_safe_env_with_extra_path_prefix_prepends_dir() {
        let safe_home = PathBuf::from("/tmp/safe-home");
        let codex_home = PathBuf::from("/home/user/.codex");
        let prefix = Path::new("/real/node/bin");
        let env = build_safe_env(&safe_home, &codex_home, Some(prefix)).unwrap();
        let path_val = env
            .iter()
            .find(|(k, _)| k == "PATH")
            .map(|(_, v)| v.clone())
            .expect("PATH must be present when extra_path_prefix is given");
        let first = std::env::split_paths(&path_val).next();
        assert_eq!(
            first.as_deref(),
            Some(prefix),
            "extra_path_prefix must be at the front of PATH"
        );
    }
}
