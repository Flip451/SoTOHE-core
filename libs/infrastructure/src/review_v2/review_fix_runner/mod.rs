mod env;
mod launch_context;
mod prompt;
mod sentinel;
mod session_log;
mod smoke_test;
mod spawn;

pub(crate) use launch_context::TrustedLaunchContext;

use crate::codex_common::resolve_codex_runtime;
use env::{build_codex_fixer_invocation, build_safe_env, create_safe_home, resolve_codex_home};
use prompt::build_prompt_with_context;
use sentinel::{parse_sentinel, sentinel_to_exit_code};
use session_log::{CREDENTIAL_VARS, SessionLogCleanup};
use smoke_test::{is_forbidden_sandbox_value, parse_major_minor, parse_semver_from_text};
use spawn::spawn_and_collect_codex;
#[cfg(test)]
use std::ffi::OsString;
use std::path::PathBuf;
#[cfg(test)]
use std::process::Command;
use usecase::capability_exec::{ModelName, ReasoningEffort};
use usecase::review_v2::run_review_fix::{
    ReviewFixRunner, ReviewFixRunnerError, RunReviewFixCommand, RunReviewFixOutput,
};

pub struct CodexReviewFixRunner {
    model: ModelName,
    effort: ReasoningEffort,
    #[cfg(test)]
    bin_override: Option<OsString>,
}

impl CodexReviewFixRunner {
    #[must_use]
    pub fn new(model: ModelName, effort: ReasoningEffort) -> CodexReviewFixRunner {
        Self {
            model,
            effort,
            #[cfg(test)]
            bin_override: None,
        }
    }

    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn with_bin(mut self, bin: impl Into<OsString>) -> Self {
        self.bin_override = Some(bin.into());
        self
    }

    fn smoke_test_forbidden_sandbox(&self) -> Result<(), ReviewFixRunnerError> {
        let val = std::env::var("CODEX_SANDBOX").unwrap_or_default();
        if is_forbidden_sandbox_value(&val) {
            return Err(ReviewFixRunnerError::SmokeTestFailed(
                usecase::git_workflow::DiagnosticText::new(format!(
                    "forbidden sandbox override detected in environment: \
                 CODEX_SANDBOX={val} — danger-full-access and \
                 dangerously-bypass-approvals-and-sandbox are prohibited \
                 (ADR D3/CN-03)"
                )),
            ));
        }
        Ok(())
    }

    fn smoke_test_codex_version(
        &self,
        bin: &std::ffi::OsStr,
        safe_env: &[(std::ffi::OsString, std::ffi::OsString)],
        launch_context: &TrustedLaunchContext,
    ) -> Result<(), ReviewFixRunnerError> {
        let probe_env = safe_env
            .iter()
            .filter(|(key, _)| !CREDENTIAL_VARS.iter().any(|credential| key == credential))
            .cloned()
            .collect::<Vec<_>>();
        let (status, combined) =
            launch_context.run_version_probe(bin, &probe_env).map_err(|error| {
                ReviewFixRunnerError::SmokeTestFailed(usecase::git_workflow::DiagnosticText::new(
                    format!("codex CLI not found in PATH or failed to execute: {error}"),
                ))
            })?;
        if !status.success() {
            return Err(ReviewFixRunnerError::SmokeTestFailed(
                usecase::git_workflow::DiagnosticText::new(format!(
                    "codex --version failed with {status}: {combined}"
                )),
            ));
        }
        let version_str = parse_semver_from_text(&combined).ok_or_else(|| {
            ReviewFixRunnerError::SmokeTestFailed(usecase::git_workflow::DiagnosticText::new(
                "cannot determine codex version from `codex --version` output",
            ))
        })?;
        let (major, minor) = parse_major_minor(&version_str).ok_or_else(|| {
            ReviewFixRunnerError::SmokeTestFailed(usecase::git_workflow::DiagnosticText::new(
                format!("cannot parse codex version components from '{version_str}'"),
            ))
        })?;
        if major > 0 {
            return Err(ReviewFixRunnerError::SmokeTestFailed(
                usecase::git_workflow::DiagnosticText::new(format!(
                    "codex version {version_str} is outside validated range (>= 0.115.0, < 1.0.0): \
                 major version upgrade requires re-validation"
                )),
            ));
        }
        if minor < 115 {
            return Err(ReviewFixRunnerError::SmokeTestFailed(
                usecase::git_workflow::DiagnosticText::new(format!(
                    "codex version {version_str} is below minimum validated version 0.115.0"
                )),
            ));
        }
        Ok(())
    }
}

impl ReviewFixRunner for CodexReviewFixRunner {
    fn run_fix(
        &self,
        command: RunReviewFixCommand,
    ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
        let briefing_content =
            crate::review_v2::review_fix_briefing::read_trusted_briefing(&command)?;
        self.run_fix_with_briefing(command, briefing_content)
    }
}

impl CodexReviewFixRunner {
    pub(super) fn run_fix_with_briefing(
        &self,
        command: RunReviewFixCommand,
        briefing_content: String,
    ) -> Result<RunReviewFixOutput, ReviewFixRunnerError> {
        let codex_home = resolve_codex_home()?;
        #[cfg(test)]
        let runtime = if self.bin_override.is_none() {
            Some(resolve_codex_runtime(command.repository_root()).map_err(|error| {
                ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(
                    error.to_string(),
                ))
            })?)
        } else {
            None
        };
        #[cfg(test)]
        let (bin, path_prefix, runtime_for_log) = match (&self.bin_override, runtime.as_ref()) {
            (Some(bin), _) => (bin.clone(), None, None),
            (None, Some(runtime)) => {
                (runtime.executable().to_os_string(), runtime.path_prefix(), Some(runtime))
            }
            (None, None) => {
                return Err(ReviewFixRunnerError::Unexpected(
                    usecase::git_workflow::DiagnosticText::new("test Codex runtime missing"),
                ));
            }
        };
        #[cfg(not(test))]
        let runtime = resolve_codex_runtime(command.repository_root()).map_err(|error| {
            ReviewFixRunnerError::Unexpected(usecase::git_workflow::DiagnosticText::new(
                error.to_string(),
            ))
        })?;
        #[cfg(not(test))]
        let (bin, path_prefix, runtime_for_log) =
            (runtime.executable().to_os_string(), runtime.path_prefix(), Some(&runtime));
        let launch_context = TrustedLaunchContext::for_repository(command.repository_root())?;
        self.smoke_test_forbidden_sandbox()?;
        let safe_home = create_safe_home()?;
        let _home_cleanup = SafeHomeCleanup(safe_home.clone());
        let safe_env = build_safe_env(&safe_home, &codex_home, path_prefix)?;
        self.smoke_test_codex_version(&bin, &safe_env, &launch_context)?;
        let prompt = build_prompt_with_context(command.scope(), &command, &briefing_content)?;
        let output_last_message =
            launch_context.create_runtime_file("review-fix-codex-last-message", "txt")?;
        let output_last_message_path = output_last_message.path().to_path_buf();
        let _last_message_cleanup = OutputLastMessageCleanup(output_last_message);
        let args = build_codex_fixer_invocation(
            self.model.as_str(),
            self.effort,
            &codex_home,
            &output_last_message_path,
        );
        let (stdout, child_status, log_file) = spawn_and_collect_codex(
            &bin,
            &args,
            &safe_env,
            &prompt,
            &launch_context,
            runtime_for_log,
        )?;
        // By default the guard removes the log on drop. Disarm it on failure
        // paths so the log is retained for diagnosis.
        let log_path = log_file.path().to_path_buf();
        let log_cleanup = SessionLogCleanup::new(log_file);
        let last_message_content = match launch_context
            .read_runtime_file_bounded(&_last_message_cleanup.0, MAX_OUTPUT_LAST_MESSAGE_BYTES)
        {
            Ok(content) => content,
            Err(e) => {
                log_cleanup.keep_for_diagnosis();
                return Err(ReviewFixRunnerError::Unexpected(
                    usecase::git_workflow::DiagnosticText::new(format!(
                        "failed to read output-last-message {}: {e}; session log: {}",
                        output_last_message_path.display(),
                        log_path.display()
                    )),
                ));
            }
        };
        let status = parse_sentinel(&last_message_content).or_else(|| parse_sentinel(&stdout));
        let status = match status {
            Some(s) => s,
            None => {
                // Disarm the cleanup guard: log must persist so the caller can diagnose.
                log_cleanup.keep_for_diagnosis();
                let child_exit = child_status.code().map_or_else(
                    || format!("exit status {child_status}"),
                    |code| format!("exit code {code}"),
                );
                return Err(ReviewFixRunnerError::SentinelNotFound(
                    usecase::git_workflow::DiagnosticText::new(format!(
                        "no REVIEW_FIX_STATUS sentinel found; codex fixer {child_exit}; session log: {}",
                        log_path.display()
                    )),
                ));
            }
        };
        if status != "completed" {
            log_cleanup.keep_for_diagnosis();
        }
        let exit_code = sentinel_to_exit_code(status);
        Ok(RunReviewFixOutput { status: status.to_owned(), exit_code, stderr: None })
    }
}

struct SafeHomeCleanup(PathBuf);
#[rustfmt::skip]
impl Drop for SafeHomeCleanup {
    fn drop(&mut self) { let _ = std::fs::remove_dir_all(&self.0); }
}

const MAX_OUTPUT_LAST_MESSAGE_BYTES: u64 = 64 * 1024;

struct OutputLastMessageCleanup(spawn::RuntimeFile);
#[rustfmt::skip]
impl Drop for OutputLastMessageCleanup {
    fn drop(&mut self) { self.0.remove(); }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::codex_common::{REVIEW_RUNTIME_DIR, resolve_codex_runtime_for_repository_start};

    fn make_command() -> RunReviewFixCommand {
        make_command_with_briefing_file(PathBuf::from("tmp/reviewer-runtime/briefing.md"))
    }

    fn make_command_with_briefing_file(briefing_file: PathBuf) -> RunReviewFixCommand {
        RunReviewFixCommand::new_resolved(
            usecase::review_v2::ReviewScopeName::try_new("infrastructure".to_owned())
                .expect("valid scope"),
            briefing_file,
            usecase::review_v2::run_review_fix::ReviewFixResolution::new(
                usecase::review_v2::run_review_fix::ReviewTrackId::try_new(
                    "review-fix-codex-rustify-2026-05-31".to_owned(),
                )
                .expect("valid track ID"),
                std::env::current_dir().expect("repository root"),
            ),
            usecase::review_v2::ReviewRoundType::Fast,
            Some(ModelName::try_new("gpt-5.5").expect("valid model")),
        )
    }

    fn trusted_briefing_fixture() -> (tempfile::TempDir, PathBuf) {
        let repository_root = std::env::current_dir().expect("repository root");
        let fixture_root = repository_root.join("tmp");
        std::fs::create_dir_all(&fixture_root).expect("create trusted fixture root");
        let directory = tempfile::Builder::new()
            .prefix("review-fix-runner-")
            .tempdir_in(&fixture_root)
            .expect("trusted briefing fixture directory");
        let briefing_file = directory
            .path()
            .strip_prefix(&repository_root)
            .expect("fixture must be beneath repository root")
            .join("briefing.md");
        std::fs::write(repository_root.join(&briefing_file), "# Briefing\n")
            .expect("write trusted briefing fixture");
        (directory, briefing_file)
    }

    fn prepared_launch_context() -> TrustedLaunchContext {
        let (_directory, _) = trusted_briefing_fixture();
        TrustedLaunchContext::for_repository(&std::env::current_dir().expect("repository root"))
            .expect("repository root must prepare a launch context")
    }

    fn make_runner() -> CodexReviewFixRunner {
        CodexReviewFixRunner::new(
            ModelName::try_new("gpt-5.5").expect("valid test model"),
            ReasoningEffort::Low,
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
        write_fake_codex_with_exit_status(dir, version_output, 0)
    }

    #[cfg(unix)]
    fn write_fake_codex_with_exit_status(
        dir: &std::path::Path,
        version_output: &str,
        exit_status: i32,
    ) -> PathBuf {
        let script = dir.join("fake-codex.sh");
        let script_content = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"{version_output}\"; exit {exit_status}; fi\nexit 0\n"
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
    fn write_fake_codex_runner_capturing_working_directory(dir: &std::path::Path) -> PathBuf {
        let script = dir.join("fake-codex-captures-working-directory.sh");
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
printf '%s\n' "$PWD" > "$(dirname "$0")/captured-working-directory.txt"
cat >/dev/null
printf 'REVIEW_FIX_STATUS: completed\n' > "$out"
exit 0
"#;
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    #[cfg(unix)]
    fn write_fake_codex_runner_without_sentinel(dir: &std::path::Path, exit_code: i32) -> PathBuf {
        let script = dir.join("fake-codex-no-sentinel.sh");
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
printf 'not a sentinel\n' > "$out"
printf 'fake stdout without sentinel\n'
exit {exit_code}
"#
        );
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
    fn write_fake_codex_runner_with_oversize_last_message(dir: &std::path::Path) -> PathBuf {
        let script = dir.join("fake-codex-oversize-last-message.sh");
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
head -c 65537 /dev/zero | tr '\000' x > "$out"
exit 0
"#;
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
        let context = prepared_launch_context();
        let result = runner.smoke_test_codex_version(fake.as_os_str(), &[], &context);
        assert!(result.is_ok(), "expected Ok for valid version 0.125.0, got: {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_smoke_test_codex_version_too_old_returns_smoke_test_failed() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex(dir.path(), "codex 0.114.9");
        let runner = make_runner().with_bin(&fake);
        let context = prepared_launch_context();
        let result = runner.smoke_test_codex_version(fake.as_os_str(), &[], &context);
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
        let context = prepared_launch_context();
        let result = runner.smoke_test_codex_version(fake.as_os_str(), &[], &context);
        assert!(
            matches!(result, Err(ReviewFixRunnerError::SmokeTestFailed(_))),
            "expected SmokeTestFailed for major version 1.0.0, got: {result:?}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_smoke_test_codex_version_rejects_nonzero_status_with_valid_semver() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_with_exit_status(dir.path(), "codex 0.125.0", 17);
        let runner = make_runner().with_bin(&fake);
        let context = prepared_launch_context();

        let result = runner.smoke_test_codex_version(fake.as_os_str(), &[], &context);

        assert!(matches!(result, Err(ReviewFixRunnerError::SmokeTestFailed(_))));
    }

    #[cfg(unix)]
    #[test]
    fn test_runtime_resolution_from_subdirectory_uses_git_root() {
        use std::os::unix::fs::symlink;

        let directory = tempfile::tempdir().unwrap();
        let repository_root = directory.path().join("repository");
        let git_init =
            Command::new("git").args(["init", "--quiet"]).arg(&repository_root).output().unwrap();
        assert!(git_init.status.success(), "git init must create the fixture repository");

        let subdirectory = repository_root.join("nested/command");
        std::fs::create_dir_all(&subdirectory).unwrap();
        let runtime_binary = repository_root.join("fixture-codex.sh");
        std::fs::write(&runtime_binary, "#!/bin/sh\necho 'codex 0.125.0'\n").unwrap();
        make_executable(&runtime_binary);
        let runtime_link = repository_root.join(".harness/tools/bin/codex");
        std::fs::create_dir_all(runtime_link.parent().unwrap()).unwrap();
        symlink(&runtime_binary, &runtime_link).unwrap();

        let runtime = resolve_codex_runtime_for_repository_start(&subdirectory)
            .expect("subdirectory invocation must resolve the repository-local runtime");

        assert_eq!(runtime.real_path(), runtime_binary.canonicalize().unwrap());
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_fake_codex_completed_returns_completed() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_runner(dir.path());
        let (_briefing_directory, briefing) = trusted_briefing_fixture();
        let command = make_command_with_briefing_file(briefing);
        let runner = make_runner().with_bin(&fake);

        let output = runner.run_fix(command).unwrap();

        assert_eq!(output.status, "completed");
        assert_eq!(output.exit_code, 0);
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_spawns_fake_codex_from_resolver_proven_repository_root() {
        let process_cwd = std::env::current_dir().expect("process current directory");
        let fixture_parent = process_cwd.join("tmp");
        std::fs::create_dir_all(&fixture_parent).expect("fixture parent directory");
        let fixture = tempfile::Builder::new()
            .prefix("review-fix-trusted-root-")
            .tempdir_in(&fixture_parent)
            .expect("trusted root fixture directory");
        let repository_root = fixture.path().join("repository");
        std::fs::create_dir_all(&repository_root).expect("repository root directory");
        std::fs::write(repository_root.join("briefing.md"), "# Briefing\n")
            .expect("trusted briefing file");
        let fake = write_fake_codex_runner_capturing_working_directory(fixture.path());
        let command = RunReviewFixCommand::new_resolved(
            usecase::review_v2::ReviewScopeName::try_new("infrastructure".to_owned())
                .expect("valid scope"),
            PathBuf::from("briefing.md"),
            usecase::review_v2::run_review_fix::ReviewFixResolution::new(
                usecase::review_v2::run_review_fix::ReviewTrackId::try_new(
                    "review-fix-trusted-root-2026".to_owned(),
                )
                .expect("valid track ID"),
                repository_root.clone(),
            ),
            usecase::review_v2::ReviewRoundType::Fast,
            Some(ModelName::try_new("gpt-5.5").expect("valid model")),
        );

        assert_ne!(process_cwd, repository_root, "fixture must differ from process CWD");
        let output = make_runner().with_bin(&fake).run_fix(command).expect("completed run");

        assert_eq!(output.status, "completed");
        let captured =
            std::fs::read_to_string(fixture.path().join("captured-working-directory.txt"))
                .expect("fake runner working-directory capture");
        assert_eq!(
            PathBuf::from(captured.trim()).canonicalize().expect("captured directory"),
            repository_root.canonicalize().expect("repository root")
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_fake_codex_without_sentinel_returns_sentinel_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_runner_without_sentinel(dir.path(), 0);
        let (_briefing_directory, briefing) = trusted_briefing_fixture();
        let command = make_command_with_briefing_file(briefing);
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
    fn test_run_fix_without_sentinel_reports_child_exit_code_and_session_log() {
        let dir = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_runner_without_sentinel(dir.path(), 126);
        let (_briefing_directory, briefing) = trusted_briefing_fixture();
        let command = make_command_with_briefing_file(briefing);
        let runner = make_runner().with_bin(&fake);

        let result = runner.run_fix(command);

        match result {
            Err(ReviewFixRunnerError::SentinelNotFound(message)) => {
                assert!(message.as_str().contains("exit code 126"));
                assert!(message.as_str().contains("session log:"));
            }
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
        let (_briefing_directory, briefing) = trusted_briefing_fixture();
        let command = make_command_with_briefing_file(briefing);
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
        let (_briefing_directory, briefing) = trusted_briefing_fixture();
        let command = make_command_with_briefing_file(briefing);
        let runner = make_runner().with_bin(&fake);

        let result = runner.run_fix(command);

        match result {
            Err(ReviewFixRunnerError::Unexpected(message)) => {
                assert!(message.as_str().contains("failed to read output-last-message"));
            }
            Err(other) => panic!("expected Unexpected read error, got error: {other:?}"),
            Ok(output) => panic!("expected read error, got status: {}", output.status),
        }
        let log_path = retained_session_log_containing(&marker)
            .expect("read error must retain the session log for diagnosis");
        std::fs::remove_file(log_path).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_run_fix_rejects_oversize_output_last_message() {
        let directory = tempfile::tempdir().unwrap();
        let fake = write_fake_codex_runner_with_oversize_last_message(directory.path());
        let (_briefing_directory, briefing) = trusted_briefing_fixture();
        let command = make_command_with_briefing_file(briefing);

        let result = make_runner().with_bin(&fake).run_fix(command);

        assert!(matches!(
            result,
            Err(ReviewFixRunnerError::Unexpected(message))
                if message.as_str().contains("exceeds")
        ));
    }

    // ── make_command and make_runner are needed for unused-variable lint ──────

    #[test]
    fn test_make_command_and_runner_compile() {
        let _cmd = make_command();
        let _runner = make_runner();
    }
}
