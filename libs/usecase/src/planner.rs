//! Secondary port for the planner capability.
//!
//! `PlannerPort` is the boundary between `cli_driver` (primary adapter) and
//! `infrastructure` (planner adapter). The trait carries only application-level
//! data: model name, prompt string, and timeout. All infrastructure concerns
//! (process management, session log paths) are contained within the adapter
//! implementation (hexagonal-architecture.md / CN-02).
//!
//! No filesystem types or infrastructure details cross this boundary.

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
/// Implementations live in `infrastructure`. `cli_driver` holds an injected
/// `Arc<dyn PlannerPort>` and invokes `run(...)` without importing any
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
