//! [`usecase::dry_driver::DryDriverPort`] adapter for `dry fix-local` (OS-08).
//!
//! `dry write` / `dry results` / `dry check-approved` are backed by their own
//! IN-14 driver services wired directly in
//! `cli_composition::dry::DryCompositionRoot::dry_driver()`. `dry fix-local`
//! remains backed by [`CodexDryFixLocalRunner`], which spawns the Codex fixer
//! subprocess end-to-end (agent-profile resolution, safe-env construction,
//! invocation, sentinel parsing). [`DryDriverAdapter`] implements the
//! [`DryDriverPort`] secondary port by delegating to that runner (ADR
//! 2026-06-21-1328 D7: port implementations live in `libs/infrastructure`).
//!
//! Wired via [`usecase::dry_driver::DryDriverInteractor::new`] which drives the
//! service trait, keeping the wire pattern symmetric with the other three
//! driver services (interactor + injected port).

mod env;
mod prompt;
mod sentinel;
mod session_log;
mod smoke_test;
mod spawn;

use std::io::{Read as _, Seek as _};
use std::path::{Path, PathBuf};

use domain::TrackId;
use usecase::dry_driver::{DryDriverOutcome, DryDriverPort, DryFixLocalDriverInput};

use crate::agent_profiles::{AGENT_PROFILES_PATH, AgentProfiles, RoundType};
use crate::git_cli::SystemGitRepo;
use crate::track::symlink_guard::reject_symlinks_below;

use env::{
    bin_parent_dir, build_dry_fix_invocation, dry_fix_build_safe_env, dry_fix_build_smoke_env,
    dry_fix_create_safe_home, dry_fix_resolve_codex_home, resolve_codex_bin,
};
use prompt::build_dry_fix_prompt;
use sentinel::{dry_fix_sentinel_to_exit_code, parse_dry_fix_sentinel};
use session_log::DryFixSessionLogCleanup;
use smoke_test::{dry_fix_smoke_test_codex_version, dry_fix_smoke_test_forbidden_sandbox};
use spawn::{dry_fix_runtime_path, dry_fix_spawn_and_collect};

/// Names of environment variables carrying authentication credentials that
/// [`env::dry_fix_build_safe_env`] forwards to the sandboxed Codex subprocess.
/// [`env::dry_fix_build_smoke_env`] strips them from the version-smoke-test
/// environment, and [`session_log::dry_fix_redaction_values`] uses this same
/// list to redact captured subprocess output before it is echoed or written
/// to a persistent session log (`knowledge/conventions/security.md`).
const DRY_FIX_REDACTED_ENV_VARS: &[&str] =
    &["OPENAI_API_KEY", "OPENAI_ORG_ID", "OPENAI_BASE_URL", "CODEX_API_KEY"];
const DRY_FIX_LAST_MESSAGE_READ_LIMIT_BYTES: u64 = 64 * 1024;

/// Runs the Codex-backed `dry fix-local` fixer subprocess end-to-end (OS-08).
///
/// Unit struct: no adapter dependencies are injected at construction time.
pub struct CodexDryFixLocalRunner;

impl CodexDryFixLocalRunner {
    /// Create a new `CodexDryFixLocalRunner`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CodexDryFixLocalRunner {
    fn default() -> Self {
        Self::new()
    }
}

impl CodexDryFixLocalRunner {
    /// Run `sotp dry fix-local`: resolve the git root, load agent profiles,
    /// resolve the `dry-fix-lead` capability, and dispatch to the configured
    /// provider (currently only `"codex"` is supported).
    ///
    /// Never returns an `Err`: failures at any stage are folded into a
    /// [`DryDriverOutcome::failure`] so this method satisfies
    /// [`DryDriverPort::dry_fix_local`]'s infallible signature.
    pub fn dry_run_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome {
        match self.dry_run_fix_local_inner(input) {
            Ok(outcome) => outcome,
            Err(message) => DryDriverOutcome::failure(Some(message)),
        }
    }

    fn dry_run_fix_local_inner(
        &self,
        input: DryFixLocalDriverInput,
    ) -> Result<DryDriverOutcome, String> {
        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("[ERROR] failed to discover git repository root: {e}"))?;
        let profiles = load_dry_fix_agent_profiles(repo.root())?;
        let track_id = TrackId::try_new(input.track_id.trim())
            .map_err(|e| format!("invalid --track-id: {e}"))?;
        let resolved =
            profiles.resolve_execution("dry-fix-lead", RoundType::Final).ok_or_else(|| {
                "[ERROR] dry-fix-lead capability not defined in agent-profiles.json".to_owned()
            })?;
        let model = input.model.clone().or_else(|| resolved.model.clone()).ok_or_else(|| {
            "[ERROR] no model specified: pass --model or set model in agent-profiles.json \
             dry-fix-lead capability"
                .to_owned()
        })?;
        eprintln!("[sotp dry fix-local] provider={} model={}", resolved.provider, &model);
        match resolved.provider.as_str() {
            "codex" => {
                run_dry_fix_codex(&model, track_id.as_ref(), &input.briefing_file, repo.root())
            }
            other => Err(format!(
                "[ERROR] unsupported dry-fix-lead provider '{other}' (supported: 'codex')"
            )),
        }
    }
}

fn load_dry_fix_agent_profiles(repo_root: &Path) -> Result<AgentProfiles, String> {
    let canonical_root = repo_root.canonicalize().map_err(|e| {
        format!("[ERROR] failed to canonicalize repo root '{}': {e}", repo_root.display())
    })?;
    let profiles_path = canonical_root.join(AGENT_PROFILES_PATH);
    reject_symlinks_below(&profiles_path, &canonical_root).map_err(|e| {
        format!("symlink guard agent-profiles.json '{}': {e}", profiles_path.display())
    })?;
    AgentProfiles::load(&profiles_path)
        .map_err(|e| format!("[ERROR] failed to load agent-profiles.json: {e}"))
}

fn run_dry_fix_codex(
    model: &str,
    track_id: &str,
    briefing_file: &Path,
    trusted_root: &Path,
) -> Result<DryDriverOutcome, String> {
    let codex_bin = resolve_codex_bin();
    let extra_path = bin_parent_dir(&codex_bin);
    dry_fix_smoke_test_forbidden_sandbox()?;
    let codex_home = dry_fix_resolve_codex_home()?;
    let safe_home = dry_fix_create_safe_home()?;
    let _home_cleanup = DryFixSafeHomeCleanup(safe_home.clone());
    let safe_env = dry_fix_build_safe_env(&safe_home, &codex_home, extra_path.as_deref())?;
    let smoke_env = dry_fix_build_smoke_env(&safe_env);
    dry_fix_smoke_test_codex_version(&codex_bin, &smoke_env)?;
    let prompt = build_dry_fix_prompt(track_id, briefing_file, trusted_root)?;
    let output_last_message = dry_fix_runtime_path("dry-fix-codex-last-message", "txt")?;
    std::fs::write(&output_last_message, "")
        .map_err(|e| format!("failed to initialize last-message file: {e}"))?;
    let _last_message_cleanup = DryFixLastMessageCleanup(output_last_message.clone());
    let args = build_dry_fix_invocation(model, &codex_home, &safe_home, &output_last_message);
    let (stdout, log_path) = dry_fix_spawn_and_collect(&codex_bin, &args, &safe_env, &prompt)?;
    let log_cleanup = DryFixSessionLogCleanup::new(log_path.clone());
    let last_message_content = match read_dry_fix_last_message_tail(&output_last_message) {
        Ok(content) => content,
        Err(e) => {
            log_cleanup.keep_for_diagnosis();
            return Err(format!(
                "failed to read last-message file: {e}; log: {}",
                log_path.display()
            ));
        }
    };
    let status =
        parse_dry_fix_sentinel(&last_message_content).or_else(|| parse_dry_fix_sentinel(&stdout));
    let status = match status {
        Some(s) => s,
        None => {
            log_cleanup.keep_for_diagnosis();
            return Err(format!(
                "no DRY_FIX_STATUS sentinel found in fixer output; log: {}",
                log_path.display()
            ));
        }
    };
    if status != "completed" {
        log_cleanup.keep_for_diagnosis();
    }
    let exit_code = dry_fix_sentinel_to_exit_code(status);
    Ok(DryDriverOutcome {
        stdout: Some(format!("DRY_FIX_STATUS: {status}")),
        stderr: None,
        exit_code: u8::try_from(exit_code).unwrap_or(1),
    })
}

fn dry_fix_path_trusted_root(path: &Path) -> &Path {
    if path.is_absolute() {
        path.ancestors().last().unwrap_or_else(|| Path::new("/"))
    } else {
        Path::new("")
    }
}

fn read_dry_fix_last_message_tail(path: &Path) -> Result<String, String> {
    let exists = reject_symlinks_below(path, dry_fix_path_trusted_root(path))
        .map_err(|e| format!("symlink guard last-message file {}: {e}", path.display()))?;
    if !exists {
        return Err(format!("last-message file {} not found", path.display()));
    }
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|e| format!("failed to inspect last-message file {}: {e}", path.display()))?;
    if !metadata.is_file() {
        return Err(format!("last-message path {} is not a regular file", path.display()));
    }

    let len = metadata.len();
    let start = len.saturating_sub(DRY_FIX_LAST_MESSAGE_READ_LIMIT_BYTES);
    let mut file = std::fs::File::open(path)
        .map_err(|e| format!("failed to open last-message file {}: {e}", path.display()))?;
    file.seek(std::io::SeekFrom::Start(start))
        .map_err(|e| format!("failed to seek last-message file {}: {e}", path.display()))?;

    let mut bytes = Vec::new();
    file.take(len.saturating_sub(start))
        .read_to_end(&mut bytes)
        .map_err(|e| format!("failed to read last-message file {}: {e}", path.display()))?;
    Ok(String::from_utf8_lossy(&bytes).into_owned())
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

/// Driver-port adapter for the OS-08 `dry fix-local` path.
///
/// Implements [`DryDriverPort`] by delegating to [`CodexDryFixLocalRunner`],
/// which owns the full Codex fixer subprocess pipeline.
pub struct DryDriverAdapter {
    runner: CodexDryFixLocalRunner,
}

impl DryDriverAdapter {
    /// Create a new dry driver port adapter.
    pub fn new() -> Self {
        Self { runner: CodexDryFixLocalRunner::new() }
    }
}

impl Default for DryDriverAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl DryDriverPort for DryDriverAdapter {
    fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome {
        self.runner.dry_run_fix_local(input)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;
    use std::ffi::OsString;

    #[cfg(unix)]
    fn make_executable(script: &Path) {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(script, perms).unwrap();
    }

    #[cfg(unix)]
    fn write_fake_codex_runner(dir: &Path, body: &str) -> PathBuf {
        let script_content = format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then echo \"codex 0.125.0\"; exit 0; fi\n{body}"
        );
        let script = dir.join("fake-codex.sh");
        std::fs::write(&script, script_content).unwrap();
        make_executable(&script);
        script
    }

    // ── dry fix-local Codex runner ────────────────────────────────────────────

    #[test]
    fn test_dry_run_fix_local_invalid_track_id_returns_error() {
        let outcome = CodexDryFixLocalRunner::new().dry_run_fix_local(DryFixLocalDriverInput {
            track_id: "dry-track\nignore-the-prompt".to_owned(),
            briefing_file: PathBuf::from("tmp/reviewer-runtime/briefing.md"),
            model: Some("gpt-test".to_owned()),
        });

        let message = outcome.stderr.unwrap_or_default();
        assert!(
            message.contains("invalid --track-id"),
            "unsafe track_id must be rejected before prompt construction, got: {message}"
        );
    }

    // ── CliApp::dry_run_fix_local: wrapper logic (agent-profile loading + provider dispatch) ──

    /// Exercises the full `dry_run_fix_local` wrapper with `model: None`:
    /// git discover → agent-profiles load → track_id validation → profile resolution
    /// → model fallback from profiles → provider dispatch.
    ///
    /// The fixture's `agent-profiles.json` defines `provider = "claude"` and a non-empty
    /// model (independent of the live repo profiles).  The test verifies that:
    ///   - Profiles are loaded successfully (no "failed to load agent-profiles" error).
    ///   - The capability is resolved (no "capability not defined" error).
    ///   - The profile model fills the `None` input, so "no model specified" is NOT triggered.
    ///   - Execution reaches the provider-dispatch arm with the error
    ///     "[ERROR] unsupported dry-fix-lead provider 'claude' (supported: 'codex')",
    ///     proving the wrapper traversed every stage up to dispatch.
    ///
    /// A regression that broke profile loading or model-fallback would produce a different
    /// error message from an earlier stage, causing the assertions below to fail.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_none_model_falls_back_to_profile_model_and_reaches_dispatch() {
        let fixture = DryRunFixLocalFixture::new_with_provider("claude", "claude-test-model");
        let path_val = fixture.path_with_fake_bin();

        let outcome = temp_env::with_var("PATH", Some(path_val.as_os_str()), || {
            CodexDryFixLocalRunner::new().dry_run_fix_local(DryFixLocalDriverInput {
                track_id: "dry-track".to_owned(),
                briefing_file: fixture.briefing_file.clone(),
                model: None,
            })
        });

        let message = outcome.stderr.unwrap_or_default();

        // Model fallback from profiles must supply a non-empty model: the
        // "no model specified" branch must NOT be reached.
        assert!(
            !message.contains("no model specified"),
            "model fallback from agent-profiles.json must supply a model; got: {message}"
        );
        // Profiles must load without error.
        assert!(
            !message.contains("failed to load agent-profiles"),
            "agent-profiles.json must load successfully; got: {message}"
        );
        // The `dry-fix-lead` capability must be found in profiles.
        assert!(
            !message.contains("capability not defined"),
            "dry-fix-lead capability must be defined in agent-profiles.json; got: {message}"
        );
        // Provider dispatch must be reached with the sentinel error for 'claude'.
        assert!(
            message.contains("unsupported dry-fix-lead provider 'claude'"),
            "None model must reach provider dispatch with 'claude' provider error; got: {message}"
        );
    }

    /// Exercises `dry_run_fix_local` with an explicit `model: Some(x)`.
    ///
    /// Uses a fixture `agent-profiles.json` (same 'claude' provider as the `model: None` test)
    /// so the test is isolated from the live configuration.  The explicit model
    /// short-circuits `or_else(|| resolved.model.clone())` so the profile model is NOT
    /// consulted for model selection.  Profiles are still loaded to resolve the provider.
    ///
    /// Difference from the `model: None` variant: this path never reads `resolved.model`;
    /// a bug that ignored `input.model` and fell through to `resolved.model` would not be
    /// caught here, but a regression that broke the OR-logic so that explicit model
    /// triggered "no model specified" WOULD be caught.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_explicit_model_bypasses_profile_model_and_reaches_dispatch() {
        let fixture = DryRunFixLocalFixture::new_with_provider("claude", "claude-test-model");
        let path_val = fixture.path_with_fake_bin();

        let outcome = temp_env::with_var("PATH", Some(path_val.as_os_str()), || {
            CodexDryFixLocalRunner::new().dry_run_fix_local(DryFixLocalDriverInput {
                track_id: "dry-track".to_owned(),
                briefing_file: fixture.briefing_file.clone(),
                model: Some("gpt-explicit-override".to_owned()),
            })
        });

        let message = outcome.stderr.unwrap_or_default();

        // Explicit model must NOT trigger the "no model specified" fallback error.
        assert!(
            !message.contains("no model specified"),
            "explicit model must bypass the profile-model fallback; got: {message}"
        );
        assert!(
            !message.contains("invalid --track-id"),
            "valid track_id must not be rejected; got: {message}"
        );
        // Provider dispatch must be reached with the sentinel 'claude' error.
        assert!(
            message.contains("unsupported dry-fix-lead provider 'claude'"),
            "explicit model must reach provider dispatch with 'claude' provider error; got: {message}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_symlinked_agent_profiles_returns_error() {
        let fixture = DryRunFixLocalFixture::new_with_provider("claude", "claude-test-model");
        let config_dir = fixture._project_dir.path().join(".harness/config");
        let profiles_path = config_dir.join("agent-profiles.json");
        let real_profiles_path = config_dir.join("real-agent-profiles.json");
        std::fs::rename(&profiles_path, &real_profiles_path).unwrap();
        std::os::unix::fs::symlink(&real_profiles_path, &profiles_path).unwrap();
        let path_val = fixture.path_with_fake_bin();

        let outcome = temp_env::with_var("PATH", Some(path_val.as_os_str()), || {
            CodexDryFixLocalRunner::new().dry_run_fix_local(DryFixLocalDriverInput {
                track_id: "dry-track".to_owned(),
                briefing_file: fixture.briefing_file.clone(),
                model: None,
            })
        });

        let message = outcome.stderr.unwrap_or_default();
        assert!(message.contains("symlink guard agent-profiles.json"), "got: {message}");
        assert!(message.contains("symlink"), "got: {message}");
        assert!(
            !message.contains("unsupported dry-fix-lead provider"),
            "symlinked profiles must be rejected before provider dispatch, got: {message}"
        );
    }

    /// Shared fixture for `dry_run_fix_local` codex-provider end-to-end tests.
    ///
    /// Creates a temp project dir, writes `agent-profiles.json` with `provider = "codex"`
    /// and the given `profile_model`, installs a fake `git` that returns the project dir
    /// as the repository root, and pre-creates a `CODEX_HOME` directory with a
    /// `captured-model.txt` capture path exposed for tests that need it.
    #[cfg(unix)]
    struct DryRunFixLocalFixture {
        /// Temp dir that acts as the project root — must be kept alive for the duration of the test.
        _project_dir: tempfile::TempDir,
        fake_bin_dir: PathBuf,
        codex_home: PathBuf,
        /// Path to the `captured-model.txt` file written by a capture-script fake codex.
        capture_file: PathBuf,
        briefing_file: PathBuf,
    }

    #[cfg(unix)]
    impl DryRunFixLocalFixture {
        /// Build the fixture with `profile_model` written into `agent-profiles.json`
        /// (provider fixed to `codex`).
        fn new(profile_model: &str) -> Self {
            Self::new_with_provider("codex", profile_model)
        }

        /// Build the fixture with an explicit `dry-fix-lead` provider, so wrapper tests
        /// stay deterministic regardless of the live repo's `agent-profiles.json`.
        fn new_with_provider(provider: &str, profile_model: &str) -> Self {
            let project_dir = tempfile::tempdir().unwrap();
            let config_dir = project_dir.path().join(".harness").join("config");
            std::fs::create_dir_all(&config_dir).unwrap();
            std::fs::write(
                config_dir.join("agent-profiles.json"),
                format!(
                    r#"{{
  "schema_version": 1,
  "providers": {{ "{provider}": {{ "label": "Test Provider" }} }},
  "capabilities": {{
    "dry-fix-lead": {{
      "provider": "{provider}",
      "model": "{profile_model}"
    }}
  }}
}}"#
                ),
            )
            .unwrap();

            // Fake git: returns the project dir as the repository root.
            let fake_bin_dir = project_dir.path().join("fake-bin");
            std::fs::create_dir_all(&fake_bin_dir).unwrap();
            let fake_git = fake_bin_dir.join("git");
            let project_root_str = project_dir.path().to_string_lossy();
            std::fs::write(
                &fake_git,
                format!(
                    "#!/bin/sh\n\
                     if [ \"$1\" = \"rev-parse\" ] && [ \"$2\" = \"--show-toplevel\" ]; then\n\
                     \x20\x20printf '%s\\n' '{project_root_str}'\n\
                     \x20\x20exit 0\n\
                     fi\n\
                     exit 1\n"
                ),
            )
            .unwrap();
            make_executable(&fake_git);

            let codex_home = project_dir.path().join(".codex");
            std::fs::create_dir_all(&codex_home).unwrap();
            let capture_file = codex_home.join("captured-model.txt");

            let briefing_file = PathBuf::from("briefing.md");
            std::fs::write(project_dir.path().join(&briefing_file), "# dry briefing\n").unwrap();

            Self {
                _project_dir: project_dir,
                fake_bin_dir,
                codex_home,
                capture_file,
                briefing_file,
            }
        }

        /// Computes the `PATH` value with the fixture's fake-bin dir prepended, so
        /// `SystemGitRepo::discover()` resolves the temp project root via the fake git.
        fn path_with_fake_bin(&self) -> OsString {
            let mut path_entries = vec![self.fake_bin_dir.clone()];
            if let Some(existing) = std::env::var_os("PATH") {
                path_entries.extend(std::env::split_paths(&existing));
            }
            std::env::join_paths(path_entries).unwrap()
        }

        /// Installs the fake codex script and calls `dry_run_fix_local` with the given
        /// input `model`, scoping `PATH`/`CODEX_BIN`/`CODEX_HOME` to the fixture for the
        /// duration of the call.
        fn run(&self, codex_script: &str, model: Option<&str>) -> DryDriverOutcome {
            let fake_codex = write_fake_codex_runner(&self.fake_bin_dir, codex_script);
            let path_val = self.path_with_fake_bin();

            temp_env::with_vars(
                [
                    ("PATH", Some(path_val.as_os_str())),
                    ("CODEX_BIN", Some(fake_codex.as_os_str())),
                    ("CODEX_HOME", Some(self.codex_home.as_os_str())),
                ],
                || {
                    CodexDryFixLocalRunner::new().dry_run_fix_local(DryFixLocalDriverInput {
                        track_id: "dry-track".to_owned(),
                        briefing_file: self.briefing_file.clone(),
                        model: model.map(|s| s.to_owned()),
                    })
                },
            )
        }
    }

    /// End-to-end success test for `CliApp::dry_run_fix_local`:
    /// verifies that the wrapper resolves the git root, loads a project-local
    /// `agent-profiles.json`, picks the provider and model, and returns a
    /// successful outcome when the configured provider is "codex" and the
    /// fake codex binary outputs the completed sentinel.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_with_codex_provider_returns_completed() {
        let fixture = DryRunFixLocalFixture::new("gpt-test");

        let outcome = fixture.run(
            r#"out=""
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
while IFS= read -r _line; do :; done
printf 'DRY_FIX_STATUS: completed\n' > "$out"
printf 'codex done\n'
exit 0
"#,
            None,
        );

        assert_eq!(outcome.exit_code, 0, "dry_run_fix_local must succeed: {outcome:?}");
        assert_eq!(
            outcome.stdout.as_deref(),
            Some("DRY_FIX_STATUS: completed"),
            "outcome must report completed sentinel: {outcome:?}"
        );
    }

    /// Shared helper: creates a `DryRunFixLocalFixture` with `profile_model`, installs a
    /// fake codex that captures the `--model` argument to `${CODEX_HOME}/captured-model.txt`,
    /// runs `dry_run_fix_local` with `input_model`, and asserts the captured model equals
    /// `expected_model`.
    #[cfg(unix)]
    fn assert_dry_run_fix_local_forwards_model(
        profile_model: &str,
        input_model: Option<&str>,
        expected_model: &str,
    ) {
        let fixture = DryRunFixLocalFixture::new(profile_model);
        let outcome = fixture.run(
            r#"model=""
out=""
while [ "$#" -gt 0 ]; do
  case "$1" in
    --model) model="$2"; shift 2 ;;
    --output-last-message) out="$2"; shift 2 ;;
    *) shift ;;
  esac
done
printf '%s' "$model" > "${CODEX_HOME}/captured-model.txt"
while IFS= read -r _line; do :; done
printf 'DRY_FIX_STATUS: completed\n' > "$out"
printf 'codex done\n'
exit 0
"#,
            input_model,
        );
        assert_eq!(outcome.exit_code, 0, "dry_run_fix_local must succeed: {outcome:?}");
        let captured_model = std::fs::read_to_string(&fixture.capture_file)
            .expect("capture file must be written by fake codex");
        assert_eq!(
            captured_model, expected_model,
            "model must be forwarded verbatim to codex --model arg; got: {captured_model:?}"
        );
    }

    /// Verifies that `dry_run_fix_local` forwards the **profile model** to the
    /// codex invocation when `model: None` is given.
    ///
    /// The fake codex captures the `--model` value and writes it to
    /// `${CODEX_HOME}/captured-model.txt`, proving the profile-model fallback
    /// reaches the codex binary — not just the wrapper dispatch.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_none_model_forwards_profile_model_to_codex() {
        assert_dry_run_fix_local_forwards_model("gpt-profile-model", None, "gpt-profile-model");
    }

    /// Verifies that `dry_run_fix_local` forwards an **explicit override model**
    /// to the codex invocation — bypassing the profile model entirely.
    ///
    /// Uses the same `${CODEX_HOME}/captured-model.txt` capture mechanism.  The
    /// profile defines `gpt-profile-model`; the explicit input is
    /// `gpt-explicit-override`.  After the run the test asserts the codex received
    /// `gpt-explicit-override`, not `gpt-profile-model`.
    #[cfg(unix)]
    #[test]
    fn test_dry_run_fix_local_explicit_model_forwards_override_model_to_codex() {
        assert_dry_run_fix_local_forwards_model(
            "gpt-profile-model",
            Some("gpt-explicit-override"),
            "gpt-explicit-override",
        );
    }

    /// Shared helper: write a fake codex runner with `script` body, scope `CODEX_BIN` and
    /// `CODEX_HOME` to it, write a briefing file, and call `run_dry_fix_codex`. Returns the
    /// result so each test can assert its own success or error path.
    #[cfg(unix)]
    fn run_dry_fix_with_fake_codex(script: &str) -> Result<DryDriverOutcome, String> {
        let dir = tempfile::tempdir().unwrap();
        let fake_codex = write_fake_codex_runner(dir.path(), script);
        let codex_home = dir.path().join(".codex");
        let briefing_file = PathBuf::from("briefing.md");
        std::fs::write(dir.path().join(&briefing_file), "# dry briefing\n").unwrap();

        temp_env::with_vars(
            [
                ("CODEX_BIN", Some(fake_codex.as_os_str())),
                ("CODEX_HOME", Some(codex_home.as_os_str())),
            ],
            || run_dry_fix_codex("gpt-test", "dry-track", &briefing_file, dir.path()),
        )
    }

    /// Calls `run_dry_fix_codex` directly: the higher-level `dry_run_fix_local`
    /// end-to-end success is covered by
    /// `test_dry_run_fix_local_with_codex_provider_returns_completed`.
    #[cfg(unix)]
    #[test]
    fn test_run_dry_fix_codex_completed_sentinel_returns_exit_zero() {
        let outcome = run_dry_fix_with_fake_codex(
            r#"out=""
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
while IFS= read -r _line; do :; done
printf 'DRY_FIX_STATUS: completed\n' > "$out"
printf 'nested dry fixer stdout\n'
exit 0
"#,
        )
        .unwrap();

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout.as_deref(), Some("DRY_FIX_STATUS: completed"));
        assert_eq!(outcome.stderr, None);
    }

    #[cfg(unix)]
    #[test]
    fn test_run_dry_fix_codex_missing_sentinel_returns_error() {
        let result = run_dry_fix_with_fake_codex(
            r#"out=""
while [ "$#" -gt 0 ]; do
  if [ "$1" = "--output-last-message" ]; then
    out="$2"
    shift 2
  else
    shift
  fi
done
while IFS= read -r _line; do :; done
printf 'not a sentinel\n' > "$out"
printf 'stdout without sentinel\n'
exit 0
"#,
        );

        let message = result.unwrap_err();
        assert!(
            message.contains("no DRY_FIX_STATUS sentinel"),
            "missing sentinel must return a diagnostic error, got: {message}"
        );
    }

    #[test]
    fn test_read_dry_fix_last_message_tail_bounds_large_file_and_keeps_sentinel() {
        let dir = tempfile::tempdir().unwrap();
        let last_message = dir.path().join("last-message.txt");
        let mut content = vec![b'x'; DRY_FIX_LAST_MESSAGE_READ_LIMIT_BYTES as usize + 4096];
        content.extend_from_slice(b"\nDRY_FIX_STATUS: completed\n");
        std::fs::write(&last_message, content).unwrap();

        let read = read_dry_fix_last_message_tail(&last_message).unwrap();

        assert!(read.len() <= DRY_FIX_LAST_MESSAGE_READ_LIMIT_BYTES as usize);
        assert_eq!(parse_dry_fix_sentinel(&read), Some("completed"));
    }

    #[cfg(unix)]
    #[test]
    fn test_read_dry_fix_last_message_tail_symlink_fails_closed() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_file = outside.path().join("last-message.txt");
        std::fs::write(&outside_file, "DRY_FIX_STATUS: completed\n").unwrap();
        let link = dir.path().join("last-message.txt");
        std::os::unix::fs::symlink(&outside_file, &link).unwrap();

        let err = read_dry_fix_last_message_tail(&link).unwrap_err();

        assert!(err.contains("symlink"), "got: {err}");
    }
}
