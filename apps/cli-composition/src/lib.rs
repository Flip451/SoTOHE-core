//! CLI composition root for the `sotp` binary.
//!
//! Provides `CliApp` (the facade) and `CommandOutcome` (the unified return type).
//! All public method arguments and return types use only `String`, `&str`,
//! `PathBuf`, primitives, `CommandOutcome`, or DTOs defined in this crate —
//! no `usecase` / `domain` / `infrastructure` types appear on the public face (CN-02).

// ---------------------------------------------------------------------------
// Submodule declarations (impl blocks for CliApp, grouped by command family)
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

mod dry_fix_runner;

/// Telemetry wiring for the composition root.
///
/// Provides subscriber initialisation, branch-bound `TelemetryWriter`
/// construction, and fire-and-forget event emit helpers.  This module is the
/// only place in the codebase where `tracing_subscriber` is initialised
/// (IN-01 / CN-04 / AC-01).
pub mod telemetry_wiring;

#[cfg(test)]
pub(crate) mod test_support {
    use std::path::PathBuf;
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
pub use file::FileCompositionRoot;
pub use track::composition_root::TrackCompositionRoot;

/// Re-exports [`infrastructure::codex_common::tee_stderr_to_file`] so that
/// `apps/cli` can use it without importing `infrastructure` directly (which the
/// architecture disallows for normal `cli` dependencies).
///
/// Only `std` types appear in the signature, so no infrastructure types leak
/// across the boundary.
pub use infrastructure::codex_common::tee_stderr_to_file;

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

/// Unified return type for all `CliApp` command methods.
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

// ---------------------------------------------------------------------------
// Facade
// ---------------------------------------------------------------------------

/// Facade for all CLI command families.
///
/// Each method corresponds to one CLI subcommand.  T002 provides stub
/// implementations (`Err("not implemented")`); T003-T005 fill in the real
/// composition logic.
///
/// Public method signatures are fixed in T002 and must not change in T003-T005:
/// only the bodies are replaced.
pub struct CliApp;

impl CliApp {
    /// Create a new `CliApp` instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for CliApp {
    fn default() -> Self {
        Self::new()
    }
}
