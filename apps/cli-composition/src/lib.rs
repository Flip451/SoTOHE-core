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
mod conventions;
mod demo;
mod domain;
pub mod dry;
mod file;
mod git;
mod guard;
mod hook;
mod pr;
pub mod review_v2;
mod semantic_dup;
mod track;
mod verify;

mod dry_fix_runner;

#[cfg(test)]
pub(crate) mod test_support {
    use std::sync::{Mutex, OnceLock};

    pub(crate) fn process_env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }
}

// ---------------------------------------------------------------------------
// Public re-exports for all DTOs (callers use `cli_composition::ReviewRunCodexInput` etc.)
// ---------------------------------------------------------------------------

pub use domain::ExportSchemaInput;
pub use dry::{DryCheckApprovedInput, DryResultsInput, DryWriteInput, RunDryFixLocalInput};
pub use review_v2::{
    ReviewResultsInput, ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput,
    RunReviewFixLocalInput,
};
pub use semantic_dup::{
    DupCheckInput, DupIndexBuildInput, DupIndexMeasureQualityInput, FindSimilarInput,
};

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
