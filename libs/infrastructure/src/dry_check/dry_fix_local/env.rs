use std::ffi::OsString;
use std::path::{Path, PathBuf};

// ── Safe HOME / CODEX_HOME resolution ──────────────────────────────────────

fn prepend_dir_to_path(dir: &Path) -> Result<OsString, String> {
    let mut paths = vec![dir.to_path_buf()];
    if let Some(existing) = std::env::var_os("PATH") {
        if !existing.is_empty() {
            paths.extend(std::env::split_paths(&existing));
        }
    }
    std::env::join_paths(paths)
        .map_err(|e| format!("failed to prepend {} to PATH: {e}", dir.display()))
}

pub(super) fn dry_fix_resolve_codex_home() -> Result<PathBuf, String> {
    if let Ok(explicit) = std::env::var("CODEX_HOME") {
        if !explicit.is_empty() {
            let p = if let Some(rest) = explicit.strip_prefix("~/") {
                let home = std::env::var("HOME")
                    .map_err(|e| format!("CODEX_HOME starts with ~/ but HOME not set: {e}"))?;
                PathBuf::from(home).join(rest)
            } else if explicit == "~" {
                let home = std::env::var("HOME")
                    .map_err(|e| format!("CODEX_HOME is ~ but HOME not set: {e}"))?;
                PathBuf::from(home).join(".codex")
            } else {
                PathBuf::from(&explicit)
            };
            return dry_fix_make_absolute(p);
        }
    }
    let home = std::env::var("HOME")
        .map_err(|e| format!("HOME env var is not set (cannot resolve default CODEX_HOME): {e}"))?;
    dry_fix_make_absolute(PathBuf::from(home).join(".codex"))
}

fn dry_fix_make_absolute(path: PathBuf) -> Result<PathBuf, String> {
    if path.is_absolute() {
        return Ok(path);
    }
    let cwd =
        std::env::current_dir().map_err(|e| format!("failed to resolve current directory: {e}"))?;
    Ok(cwd.join(path))
}

pub(super) fn dry_fix_create_safe_home() -> Result<PathBuf, String> {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir();
    for _ in 0..16_u8 {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| format!("failed to compute timestamp: {e}"))?
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
            Err(e) => return Err(format!("failed to create safe HOME {}: {e}", path.display())),
        }
    }
    Err("failed to create a unique safe HOME after repeated attempts".to_owned())
}

// ── Safe environment construction ──────────────────────────────────────────

pub(super) fn dry_fix_build_safe_env(
    safe_home: &Path,
    codex_home: &Path,
    extra_path_prefix: Option<&Path>,
) -> Result<Vec<(OsString, OsString)>, String> {
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

pub(super) fn dry_fix_build_smoke_env(
    safe_env: &[(OsString, OsString)],
) -> Vec<(OsString, OsString)> {
    safe_env
        .iter()
        .filter(|(key, _)| {
            key.to_str().map(|key| !super::DRY_FIX_REDACTED_ENV_VARS.contains(&key)).unwrap_or(true)
        })
        .cloned()
        .collect()
}

// ── Codex invocation arguments ─────────────────────────────────────────────

pub(super) fn build_dry_fix_invocation(
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    #[test]
    fn test_dry_fix_build_safe_env_strips_repository_credentials() {
        temp_env::with_vars(
            [
                ("GITHUB_TOKEN", Some("ghp-secret")),
                ("SSH_AUTH_SOCK", Some("/tmp/ssh-agent.sock")),
                ("HOME", Some("/real-home")),
                ("CODEX_HOME", Some("/real-codex-home")),
                ("CODEX_API_KEY", Some("codex-secret")),
            ],
            || {
                let safe_home = PathBuf::from("/tmp/safe-home");
                let codex_home = PathBuf::from("/tmp/codex-home");
                let env = dry_fix_build_safe_env(&safe_home, &codex_home, None).unwrap();
                let keys: Vec<String> =
                    env.iter().map(|(key, _)| key.to_string_lossy().into_owned()).collect();

                assert!(!keys.iter().any(|key| key == "GITHUB_TOKEN"));
                assert!(!keys.iter().any(|key| key == "SSH_AUTH_SOCK"));
                assert_eq!(
                    env.iter()
                        .find(|(key, _)| key.to_string_lossy() == "HOME")
                        .map(|(_, value)| value.to_string_lossy().into_owned())
                        .as_deref(),
                    Some("/tmp/safe-home")
                );
                assert_eq!(
                    env.iter()
                        .find(|(key, _)| key.to_string_lossy() == "CODEX_HOME")
                        .map(|(_, value)| value.to_string_lossy().into_owned())
                        .as_deref(),
                    Some("/tmp/codex-home")
                );
                assert_eq!(
                    env.iter()
                        .find(|(key, _)| key.to_string_lossy() == "GIT_SSH_COMMAND")
                        .map(|(_, value)| value.to_string_lossy().into_owned())
                        .as_deref(),
                    Some("/bin/false")
                );
                assert!(keys.iter().any(|key| key == "CODEX_API_KEY"));
            },
        );
    }

    #[test]
    fn test_build_dry_fix_invocation_includes_safe_home_writable_root() {
        let codex_home = PathBuf::from("/tmp/codex-home");
        let safe_home = PathBuf::from("/tmp/safe-home");
        let output_last_message = PathBuf::from("/tmp/dry-fix-last-message.txt");

        let args =
            build_dry_fix_invocation("gpt-test", &codex_home, &safe_home, &output_last_message);
        let args_str: Vec<String> =
            args.iter().map(|arg| arg.to_string_lossy().into_owned()).collect();
        let writable_roots = args_str
            .iter()
            .find(|arg| arg.contains("sandbox_workspace_write.writable_roots"))
            .expect("writable_roots config must be present");

        assert!(writable_roots.contains("/tmp/codex-home"));
        assert!(writable_roots.contains("/tmp/safe-home"));
    }
}
