//! CLI composition root for the `sotp` binary.
//!
//! Provides `CommandOutcome` (the unified return type) and per-context composition roots.
//! All public method arguments and return types use only `String`, `&str`,
//! `PathBuf`, primitives, `CommandOutcome`, or DTOs defined in this crate —
//! no `usecase` / `domain` / `infrastructure` types appear on the public face (CN-02).

// ---------------------------------------------------------------------------
// Submodule declarations (grouped by command family)
// ---------------------------------------------------------------------------

mod arch;
mod cmd_outcome;
mod conventions;
mod demo;
mod domain;
pub mod dry;
pub mod error;
mod file;
mod git;
mod guard;
mod hook;
mod pr;
mod ref_verify;
pub mod review_v2;
mod semantic_dup;
pub mod signal;
mod signal_layer_chain;
mod telemetry;
pub mod track;
pub mod verify;

pub mod dry_driver_adapter;
mod dry_fix_runner;
pub mod semantic_dup_driver_adapter;

/// Telemetry wiring for the composition root.
///
/// Provides subscriber initialisation, branch-bound `TelemetryWriter`
/// construction, and fire-and-forget event emit helpers.  This module is the
/// only place in the codebase where `tracing_subscriber` is initialised
/// (IN-01 / CN-04 / AC-01).
pub mod telemetry_wiring;

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
pub(crate) mod test_support {
    use std::path::{Path, PathBuf};
    use std::process::Command;
    use std::sync::{Mutex, OnceLock};

    pub(crate) fn process_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    pub(crate) fn repo_root_for_tests() -> PathBuf {
        let mut root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        root.pop();
        root.pop();
        root
    }

    #[cfg(unix)]
    pub(crate) fn make_executable(script: &std::path::Path) {
        use std::os::unix::fs::PermissionsExt;
        let result = std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755));
        assert!(result.is_ok(), "failed to make {} executable: {result:?}", script.display());
    }

    pub(crate) fn run_in_dir<T>(path: &Path, run: impl FnOnce() -> T) -> T {
        let original = std::env::current_dir().unwrap();
        std::env::set_current_dir(path).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(run));
        std::env::set_current_dir(original).unwrap();
        match result {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn run_git(path: &Path, args: &[&str]) {
        let status = Command::new("git")
            .args(args)
            .current_dir(path)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@test.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@test.com")
            .status()
            .unwrap();
        assert!(status.success(), "git {args:?} failed with {status}");
    }

    pub(crate) fn seed_repo(path: &Path, branch: &str) {
        run_git(path, &["init", "-q"]);
        run_git(path, &["checkout", "-B", branch]);
        run_git(path, &["commit", "--allow-empty", "-m", "init", "--no-gpg-sign"]);
    }
}

// ---------------------------------------------------------------------------
// Public re-exports for all DTOs (callers use `cli_composition::ReviewRunCodexInput` etc.)
// ---------------------------------------------------------------------------

pub use domain::ExportSchemaInput;
pub use dry::{DryCheckApprovedInput, DryResultsInput, DryWriteInput, RunDryFixLocalInput};
pub use error::CompositionError;
pub use ref_verify::{RefVerifyCheckApprovedInput, RefVerifyRunInput};
pub use review_v2::{
    ReviewResultsInput, ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput,
    RunReviewFixLocalInput,
};
pub use semantic_dup::{
    DupCheckInput, DupIndexBuildInput, DupIndexMeasureQualityInput, FindSimilarInput,
};
pub use signal::SignalGateName;
pub use telemetry::TelemetryReportInput;
pub use track::fixpoint_resolve::FixpointResolveInput;

// ---------------------------------------------------------------------------
// Per-context composition root re-exports (AC-04 / D2)
// ---------------------------------------------------------------------------

pub use arch::ArchCompositionRoot;
pub use conventions::ConventionsCompositionRoot;
pub use demo::DemoCompositionRoot;
pub use domain::DomainCompositionRoot;
pub use dry::DryCompositionRoot;
pub use dry_fix_runner::DryFixRunnerCompositionRoot;
pub use file::FileCompositionRoot;
pub use git::GitCompositionRoot;
pub use guard::GuardCompositionRoot;
pub use hook::HookCompositionRoot;
pub use pr::PrCompositionRoot;
pub use ref_verify::RefVerifyCompositionRoot;
pub use review_v2::ReviewCompositionRoot;
pub use semantic_dup::SemanticDupCompositionRoot;
pub use signal::SignalCompositionRoot;
pub use telemetry::TelemetryCompositionRoot;
pub use track::composition_root::TrackCompositionRoot;
pub use verify::VerifyCompositionRoot;

/// Tee the child process's stderr to a log file while also forwarding each
/// line to the current process's stderr.
///
/// Implemented natively with stdlib types so that `apps/cli` can call this
/// helper without importing `infrastructure` directly.  The signature uses
/// only `std` types; no infrastructure types cross the boundary.
pub fn tee_stderr_to_file(pipe: std::process::ChildStderr, mut log_file: std::fs::File) {
    use std::io::{BufRead as _, BufReader, Write as _};

    let reader = BufReader::new(pipe);
    for line in reader.lines() {
        match line {
            Ok(line) => {
                let _ = writeln!(log_file, "{line}");
                eprintln!("{line}");
            }
            Err(_) => break,
        }
    }
    let _ = log_file.flush();
}

/// Build the argument vector for a `codex exec --sandbox read-only` invocation.
///
/// Re-exports [`infrastructure::codex_common::build_codex_read_only_invocation`]
/// so that `apps/cli` test helpers can reuse it without importing
/// `infrastructure` directly (which the architecture disallows for `cli`).
///
/// # Arguments
/// - `model`: Codex model name.
/// - `reasoning_effort`: `model_reasoning_effort` value (e.g. `"high"`).
/// - `prompt`: Full prompt string.
/// - `output_last_message`: Path where Codex writes the last message JSON.
/// - `output_schema`: Path to the JSON schema file for structured output.
pub fn build_codex_read_only_invocation(
    model: &str,
    reasoning_effort: &str,
    prompt: &str,
    output_last_message: &std::path::Path,
    output_schema: &std::path::Path,
) -> Vec<std::ffi::OsString> {
    infrastructure::codex_common::build_codex_read_only_invocation(
        model,
        reasoning_effort,
        prompt,
        output_last_message,
        output_schema,
    )
}

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Unified return type for all command methods.
///
/// `bin` reads `stdout` / `stderr` and emits them, then exits with `exit_code`.
/// All fields are primitives so `bin` never needs to import domain types (CN-02).
#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: u8,
}

impl CommandOutcome {
    /// Convenience constructor: success with optional stdout text.
    pub fn success(stdout: Option<String>) -> Self {
        Self { stdout, stderr: None, exit_code: 0 }
    }

    /// Convenience constructor: failure with optional stderr text.
    pub fn failure(stderr: Option<String>) -> Self {
        Self { stdout: None, stderr, exit_code: 1 }
    }
}
