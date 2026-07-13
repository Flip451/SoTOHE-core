//! I/O for `.harness/config/agent-profiles.json` — capability-centric agent routing.
//!
//! Reads the v2 schema (`schema_version: 1`) where each capability directly
//! specifies its provider and model.

mod profiles;
mod types;

pub use profiles::AgentProfiles;
pub use types::{
    AgentProfilesError, CapabilityConfigDto, ExecutionModeDto, ModelNameDto, ProviderNameDto,
    ReasoningEffortDto, ResolvedExecution, RoundType,
};

/// Default path for the agent profiles configuration file.
pub const AGENT_PROFILES_PATH: &str = ".harness/config/agent-profiles.json";

#[cfg(any())]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod legacy_tests;

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests;
