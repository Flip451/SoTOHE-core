//! Planner use case port, application service, and interactor.
//!
//! `PlannerPort` is the secondary port boundary between the usecase layer and
//! `infrastructure` (planner adapter). The trait carries only application-level
//! data: model name, prompt string, and timeout. All infrastructure concerns
//! (process management, session log paths) are contained within the adapter
//! implementation (hexagonal-architecture.md / CN-02).
//!
//! `PlannerService` is the application-level contract that `PlanDriver`
//! (primary adapter in `cli_driver`) depends on via DIP. `PlannerInteractor`
//! implements `PlannerService`, absorbing prompt construction before delegating
//! to `PlannerPort`.
//!
//! No filesystem types or infrastructure details cross the `PlannerPort` boundary.

/// Output from a single planner execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanRunOutput {
    /// Exit code returned by the planner (0 = success).
    pub exit_code: u8,
}

/// Error type for planner port failures.
///
/// Variants describe planner-level failure concepts only. Infrastructure
/// details (process IDs, file paths, OS errors) must not appear here.
#[derive(Debug, thiserror::Error)]
pub enum PlannerPortError {
    /// No prompt source was provided.
    #[error("either --briefing-file or --prompt is required")]
    MissingPromptSource,

    /// The planner could not be started or contacted.
    #[error("planner unavailable: {reason}")]
    PlannerUnavailable { reason: String },

    /// The planner request exceeded the allowed time.
    #[error("planner timed out after {elapsed_seconds}s")]
    PlannerTimeout { elapsed_seconds: u64 },

    /// The planner ran but reported failure.
    #[error("planner failed with exit code {exit_code}")]
    PlannerFailed { exit_code: u8 },
}

/// Secondary port — executes a planner request and returns the exit code.
///
/// Implementations live in `infrastructure`. The usecase interactor holds an
/// injected `Arc<dyn PlannerPort>` and invokes `run(...)` without importing any
/// infrastructure or domain types (hexagonal-architecture.md / CN-02).
pub trait PlannerPort: Send + Sync {
    /// Execute a planner request and return the result.
    ///
    /// # Arguments
    /// - `model`: Planner model name.
    /// - `prompt`: The full prompt string.
    /// - `timeout_seconds`: Planner execution timeout in seconds.
    ///
    /// # Errors
    /// Returns [`PlannerPortError`] if the planner is unavailable, times out,
    /// or exits with a failure code.
    fn run(
        &self,
        model: &str,
        prompt: &str,
        timeout_seconds: u64,
    ) -> Result<PlanRunOutput, PlannerPortError>;
}

// ---------------------------------------------------------------------------
// Application service
// ---------------------------------------------------------------------------

/// Application-level contract for the planner capability.
///
/// `PrimaryAdapter` (`PlanDriver`) depends on this interface rather than directly on
/// `PlannerPort` (DIP). `PlannerInteractor` implements this service, absorbing prompt
/// construction from `plan_input_from_args` before delegating to the injected `PlannerPort`.
pub trait PlannerService: Send + Sync {
    /// Resolve the prompt from `briefing_file` or inline `prompt`, then execute the planner.
    ///
    /// Returns [`PlannerPortError::MissingPromptSource`] if neither is provided.
    ///
    /// # Arguments
    /// - `model`: Planner model name.
    /// - `briefing_file`: Optional path to a briefing file.
    /// - `prompt`: Optional inline prompt string.
    /// - `timeout_seconds`: Planner execution timeout in seconds.
    ///
    /// # Errors
    /// Returns [`PlannerPortError`] if:
    /// - neither `briefing_file` nor `prompt` is provided ([`PlannerPortError::MissingPromptSource`])
    /// - the underlying port fails ([`PlannerPortError::PlannerUnavailable`],
    ///   [`PlannerPortError::PlannerTimeout`], [`PlannerPortError::PlannerFailed`])
    fn run_codex_local(
        &self,
        model: String,
        briefing_file: Option<std::path::PathBuf>,
        prompt: Option<String>,
        timeout_seconds: u64,
    ) -> Result<PlanRunOutput, PlannerPortError>;
}

// ---------------------------------------------------------------------------
// Interactor
// ---------------------------------------------------------------------------

/// Interactor that implements `PlannerService`.
///
/// Resolves the prompt from a briefing file path or an inline string, then delegates
/// execution to the injected `PlannerPort`. Absorbs the `plan_input_from_args`
/// application logic for prompt construction.
pub struct PlannerInteractor {
    port: std::sync::Arc<dyn PlannerPort>,
}

impl PlannerInteractor {
    /// Create a new `PlannerInteractor` wrapping the given `PlannerPort`.
    #[must_use]
    pub fn new(port: std::sync::Arc<dyn PlannerPort>) -> Self {
        Self { port }
    }
}

impl PlannerService for PlannerInteractor {
    fn run_codex_local(
        &self,
        model: String,
        briefing_file: Option<std::path::PathBuf>,
        prompt: Option<String>,
        timeout_seconds: u64,
    ) -> Result<PlanRunOutput, PlannerPortError> {
        let resolved_prompt = if let Some(path) = briefing_file {
            if !path.is_file() {
                return Err(PlannerPortError::MissingPromptSource);
            }
            format!("Read {} and perform the task described there.", path.display())
        } else if let Some(inline) = prompt {
            inline
        } else {
            return Err(PlannerPortError::MissingPromptSource);
        };

        self.port.run(&model, &resolved_prompt, timeout_seconds)
    }
}
