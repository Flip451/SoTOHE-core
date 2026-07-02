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
mod plan;
mod pr;
mod ref_verify;
pub mod review_v2;
mod semantic_dup;
pub mod signal;
mod signal_layer_chain;
pub mod task_contract;
mod telemetry;
pub mod track;
pub mod verify;

pub(crate) mod dry_driver_adapter;
mod dry_fix_runner;
pub(crate) mod semantic_dup_driver_adapter;

/// Telemetry wiring for the composition root.
///
/// Provides subscriber initialisation, branch-bound `TelemetryWriter`
/// construction, and fire-and-forget event emit helpers.  This module is the
/// only place in the codebase where `tracing_subscriber` is initialised
/// (IN-01 / CN-04 / AC-01).
pub mod telemetry_wiring;

// ---------------------------------------------------------------------------
// Public re-exports for all DTOs (callers use `cli_composition::ReviewRunCodexInput` etc.)
// ---------------------------------------------------------------------------

pub use domain::ExportSchemaInput;
pub use dry::{DryCheckApprovedInput, DryResultsInput, DryWriteInput, RunDryFixLocalInput};
pub use error::CompositionError;
pub use ref_verify::{
    RefVerifyChainFilter, RefVerifyCheckApprovedInput, RefVerifyResultsInput, RefVerifyRunInput,
    RefVerifyVerdictFilter,
};
pub use review_v2::{
    ReviewResultsInput, ReviewRunClaudeInput, ReviewRunCodexInput, ReviewRunLocalInput,
    RunReviewFixLocalInput,
};
pub use semantic_dup::{
    DupCheckInput, DupIndexBuildInput, DupIndexMeasureQualityInput, FindSimilarInput,
};
pub use signal::SignalGateName;
pub use telemetry::TelemetryReportInput;

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
pub use plan::PlanCompositionRoot;
pub use pr::PrCompositionRoot;
pub use ref_verify::RefVerifyCompositionRoot;
pub use review_v2::ReviewCompositionRoot;
pub use semantic_dup::SemanticDupCompositionRoot;
pub use signal::SignalCompositionRoot;
pub use task_contract::TaskContractCompositionRoot;
pub use telemetry::TelemetryCompositionRoot;
pub use track::composition_root::TrackCompositionRoot;
pub use verify::VerifyCompositionRoot;

// ---------------------------------------------------------------------------
// Public API types
// ---------------------------------------------------------------------------

/// Unified return type for all command methods.
///
/// Re-exported from `cli_driver` as the canonical single definition.
/// `bin` reads `stdout` / `stderr` and emits them, then exits with `exit_code`.
/// All fields are primitives so `bin` never needs to import domain types (CN-02).
pub use cli_driver::CommandOutcome;

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
