mod env;
mod prompt;
mod sentinel;
mod session_log;
mod smoke_test;
mod spawn;

use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use usecase::review_v2::run_review_fix::{
    ReviewFixRunner, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixOutput,
};

use env::{build_codex_fixer_invocation, build_safe_env, create_safe_home, resolve_codex_home};
use prompt::build_prompt;
use sentinel::{parse_sentinel, sentinel_to_exit_code};
use session_log::SessionLogCleanup;
use smoke_test::{is_forbidden_sandbox_value, parse_major_minor, parse_semver_from_text};
use spawn::{fixer_runtime_path, spawn_and_collect_codex};

#[cfg(test)]
pub(crate) const CODEX_BIN_ENV: &str = "SOTP_CODEX_BIN";

pub struct CodexReviewFixRunner {
    model: String,
    #[cfg(test)]
    bin_override: Option<OsString>,
}

impl CodexReviewFixRunner {
    #[must_use]
    pub fn new(model: String, _scope: String, _briefing_file: PathBuf) -> Self {
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

    fn smoke_test_codex_version(
        &self,
        bin: &std::ffi::OsStr,
        extra_path_prefix: Option<&Path>,
    ) -> Result<(), ReviewFixRunnerError> {
        let mut cmd = Command::new(bin);
        cmd.arg("--version").stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        if let Some(prefix) = extra_path_prefix {
            let path = env::prepend_dir_to_path(prefix)?;
            cmd.env("PATH", path);
        }
        let output = cmd.output().map_err(|e| {
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

/// Resolve the codex binary from an optional env-var value.
///
/// Resolution order:
///   (test-only) `SOTP_CODEX_BIN` env var  →  `codex_bin_var` (`CODEX_BIN`)  →  `"codex"`.
///
/// `CODEX_BIN` lets the environment inject the real codex binary path (e.g.
/// when `codex` on PATH is a toolchain-manager shim that breaks under the
/// sanitized env). Non-absolute values are resolved before credential
/// isolation in `run_fix`.
fn resolve_codex_bin_from(codex_bin_var: Option<OsString>) -> OsString {
    if let Some(value) = codex_bin_var.filter(|v| !v.is_empty()) {
        return value;
    }
    OsString::from("codex")
}

fn codex_bin() -> OsString {
    #[cfg(test)]
    if let Some(value) = std::env::var_os(CODEX_BIN_ENV).filter(|v| !v.is_empty()) {
        return value;
    }
    resolve_codex_bin_from(std::env::var_os("CODEX_BIN"))
}

/// Return the parent directory of `bin` if `bin` is an absolute path, or
/// `None` when `bin` is a bare name (will be resolved via PATH as-is).
fn bin_parent_dir(bin: &OsString) -> Option<PathBuf> {
    let p = Path::new(bin);
    if p.is_absolute() { p.parent().map(PathBuf::from) } else { None }
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
        let extra_path = bin_parent_dir(&bin);
        self.smoke_test_forbidden_sandbox()?;
        self.smoke_test_codex_version(&bin, extra_path.as_deref())?;
        let prompt = build_prompt(&command.scope, &command.briefing_file, &command)?;
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
        let safe_env = build_safe_env(&safe_home, &codex_home, extra_path.as_deref())?;
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

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use spawn::REVIEW_RUNTIME_DIR;
    use std::path::Path;

    fn make_command() -> RunReviewFixCommand {
        RunReviewFixCommand {
            scope: "infrastructure".to_owned(),
            briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            track_id: "review-fix-codex-rustify-2026-05-31".to_owned(),
            round_type: "fast".to_owned(),
            model: "gpt-5.5".to_owned(),
        }
    }

    fn make_runner() -> CodexReviewFixRunner {
        CodexReviewFixRunner::new(
            "gpt-5.5".to_owned(),
            "infrastructure".to_owned(),
            PathBuf::from("tmp/reviewer-runtime/briefing.md"),
        )
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
        let result = runner.smoke_test_codex_version(fake.as_os_str(), None);
        assert!(result.is_ok(), "expected Ok for valid version 0.125.0, got: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_smoke_test_codex_version_too_old_returns_smoke_test_failed() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex(dir.path(), "codex 0.114.9");
        let runner = make_runner().with_bin(&fake);
        let result = runner.smoke_test_codex_version(fake.as_os_str(), None);
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
        let result = runner.smoke_test_codex_version(fake.as_os_str(), None);
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
        command.briefing_file = briefing;
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
        command.briefing_file = briefing;
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
        command.briefing_file = briefing;
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
        command.briefing_file = briefing;
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

    // ── resolve_codex_bin_from ────────────────────────────────────────────────

    #[test]
    fn test_resolve_codex_bin_from_none_returns_codex() {
        let result = resolve_codex_bin_from(None);
        assert_eq!(result, OsString::from("codex"));
    }

    #[test]
    fn test_resolve_codex_bin_from_empty_returns_codex() {
        let result = resolve_codex_bin_from(Some(OsString::from("")));
        assert_eq!(result, OsString::from("codex"));
    }

    #[test]
    fn test_resolve_codex_bin_from_absolute_path_returns_that_path() {
        let abs = OsString::from("/usr/local/bin/codex");
        let result = resolve_codex_bin_from(Some(abs.clone()));
        assert_eq!(result, abs);
    }

    // ── bin_parent_dir ────────────────────────────────────────────────────────

    #[test]
    fn test_bin_parent_dir_absolute_returns_parent() {
        let bin = OsString::from("/usr/local/bin/codex");
        let parent = bin_parent_dir(&bin);
        assert_eq!(parent.as_deref(), Some(Path::new("/usr/local/bin")));
    }

    #[test]
    fn test_bin_parent_dir_bare_name_returns_none() {
        let bin = OsString::from("codex");
        let parent = bin_parent_dir(&bin);
        assert!(parent.is_none(), "bare name must return None");
    }

    // ── make_command and make_runner are needed for unused-variable lint ──────

    #[test]
    fn test_make_command_and_runner_compile() {
        let _cmd = make_command();
        let _runner = make_runner();
    }
}
