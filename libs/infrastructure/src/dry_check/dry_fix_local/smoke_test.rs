use std::ffi::OsString;
use std::process::{Command, Stdio};

pub(super) fn dry_fix_smoke_test_forbidden_sandbox() -> Result<(), String> {
    let val = std::env::var("CODEX_SANDBOX").unwrap_or_default();
    if matches!(val.as_str(), "danger-full-access" | "dangerously-bypass-approvals-and-sandbox") {
        return Err(format!(
            "[ERROR] smoke test failed: forbidden sandbox override detected in environment: \
             CODEX_SANDBOX={val} — danger-full-access and \
             dangerously-bypass-approvals-and-sandbox are prohibited"
        ));
    }
    Ok(())
}

pub(super) fn dry_fix_smoke_test_codex_version(
    bin: &OsString,
    safe_env: &[(OsString, OsString)],
) -> Result<(), String> {
    let mut cmd = Command::new(bin);
    cmd.arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
    cmd.env_clear();
    for (key, value) in safe_env {
        cmd.env(key, value);
    }
    let output = cmd.output().map_err(|e| {
        format!("[ERROR] smoke test failed: codex CLI not found or failed to execute: {e}")
    })?;
    let combined = {
        let mut s = String::from_utf8_lossy(&output.stdout).into_owned();
        s.push_str(&String::from_utf8_lossy(&output.stderr));
        s
    };
    let version_str = parse_semver_from_output(&combined).ok_or_else(|| {
        "[ERROR] smoke test failed: cannot determine codex version from `codex --version` output"
            .to_owned()
    })?;
    let (major, minor) = parse_major_minor_version(&version_str).ok_or_else(|| {
        format!(
            "[ERROR] smoke test failed: cannot parse codex version components from '{version_str}'"
        )
    })?;
    if major > 0 {
        return Err(format!(
            "[ERROR] smoke test failed: codex version {version_str} is outside validated range \
             (>= 0.115.0, < 1.0.0): major version upgrade requires re-validation"
        ));
    }
    if minor < 115 {
        return Err(format!(
            "[ERROR] smoke test failed: codex version {version_str} is below minimum validated \
             version 0.115.0"
        ));
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

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::path::Path;

    use super::super::env::dry_fix_build_smoke_env;

    #[cfg(unix)]
    fn make_executable(script: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(script, perms).unwrap();
    }

    #[cfg(unix)]
    fn write_executable_script(dir: &Path, name: &str, script_content: &str) -> std::path::PathBuf {
        let script = dir.join(name);
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    #[test]
    fn test_dry_fix_smoke_test_codex_version_uses_scrubbed_env() {
        temp_env::with_vars(
            [("GITHUB_TOKEN", Some("ghp-secret")), ("CODEX_API_KEY", Some("codex-secret"))],
            || {
                let dir = tempfile::tempdir().unwrap();
                let fake_codex = write_executable_script(
                    dir.path(),
                    "fake-codex-version.sh",
                    r#"#!/bin/sh
if [ "$1" = "--version" ]; then
  if [ -n "$GITHUB_TOKEN" ] || [ -n "$CODEX_API_KEY" ]; then
    echo "secret-bearing env reached version check" >&2
    exit 9
  fi
  if [ "$HOME" != "/tmp/safe-home" ]; then
    echo "safe HOME not applied" >&2
    exit 8
  fi
  echo "codex 0.125.0"
  exit 0
fi
exit 0
"#,
                );
                let safe_env = vec![
                    (OsString::from("HOME"), OsString::from("/tmp/safe-home")),
                    (OsString::from("CODEX_API_KEY"), OsString::from("codex-secret")),
                ];
                let smoke_env = dry_fix_build_smoke_env(&safe_env);

                dry_fix_smoke_test_codex_version(
                    &fake_codex.as_os_str().to_os_string(),
                    &smoke_env,
                )
                .unwrap();
            },
        );
    }
}
