//! Primary adapter driver for generic capability dispatch.

use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use usecase::capability_exec::{
    CapabilityDispatchOutcome, CapabilityExecRequest, CapabilityExecService, CapabilityFilePath,
    ProviderName, TimeoutSeconds,
};
use usecase::dry_write_driver::CapabilityName;

use crate::render::CommandOutcome;

/// CLI-boundary mirror of a validated capability name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityNameArg(CapabilityName);

impl core::str::FromStr for CapabilityNameArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        CapabilityName::try_new(value.to_owned()).map(Self).map_err(|error| error.to_string())
    }
}

/// CLI-boundary mirror of a validated host provider name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderNameArg(ProviderName);

impl core::str::FromStr for ProviderNameArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        ProviderName::try_new(value.to_owned()).map(Self).map_err(|error| error.to_string())
    }
}

/// CLI-boundary mirror of a validated capability briefing path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFilePathArg(CapabilityFilePath);

impl core::str::FromStr for CapabilityFilePathArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        CapabilityFilePath::try_new(PathBuf::from(value))
            .map(Self)
            .map_err(|error| error.to_string())
    }
}

/// CLI-boundary mirror of a validated provider-process timeout.
#[derive(Debug, PartialEq, Eq)]
pub struct TimeoutSecondsArg(TimeoutSeconds);

impl Copy for TimeoutSecondsArg {}

impl Clone for TimeoutSecondsArg {
    fn clone(&self) -> Self {
        *self
    }
}

impl FromStr for TimeoutSecondsArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let seconds: u64 =
            value.parse().map_err(|error| format!("invalid timeout seconds: {error}"))?;
        TimeoutSeconds::try_new(seconds).map(Self).map_err(|error| error.to_string())
    }
}

/// Parsed input for `sotp capability exec`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExecDriverInput {
    /// Name of the capability to resolve from the runtime profile.
    pub capability: CapabilityNameArg,
    /// Actual provider hosting the orchestrator that invoked this command.
    pub host: ProviderNameArg,
    /// Validated path to the capability briefing file.
    pub briefing_file: CapabilityFilePathArg,
    /// Provider-process timeout; `None` waits without a time limit.
    pub timeout_seconds: Option<TimeoutSecondsArg>,
}

/// Primary adapter driver for generic capability dispatch.
pub struct CapabilityDriver {
    service: Arc<dyn CapabilityExecService>,
}

impl CapabilityDriver {
    /// Creates a driver from an injected usecase service.
    #[must_use]
    pub fn new(service: Arc<dyn CapabilityExecService>) -> Self {
        Self { service }
    }

    /// Builds a prevalidated request, runs dispatch, and renders a discriminated outcome.
    #[must_use]
    pub fn handle(&self, input: CapabilityExecDriverInput) -> CommandOutcome {
        let request = into_request(input);
        match self.service.execute(request) {
            Ok(outcome) => render_outcome(outcome),
            Err(error) => CommandOutcome::failure(Some(error.to_string())),
        }
    }
}

fn into_request(input: CapabilityExecDriverInput) -> CapabilityExecRequest {
    CapabilityExecRequest {
        capability: input.capability.0,
        host: input.host.0,
        briefing_file: input.briefing_file.0,
        timeout: input.timeout_seconds.map(|timeout| timeout.0),
    }
}

fn render_outcome(outcome: CapabilityDispatchOutcome) -> CommandOutcome {
    match outcome {
        CapabilityDispatchOutcome::Executed { provider, exit_code } => CommandOutcome {
            stdout: Some(format!(
                "CAPABILITY_EXEC_OUTCOME: executed\nprovider: {provider}\nexit_code: {exit_code}"
            )),
            stderr: None,
            exit_code,
        },
        CapabilityDispatchOutcome::DelegateInHost { capability, briefing_file, discipline } => {
            CommandOutcome::success(Some(format!(
                "CAPABILITY_EXEC_OUTCOME: delegate-in-host\ncapability: {capability}\nbriefing_file: {briefing_file}\ndiscipline:\n{}",
                discipline.as_str()
            )))
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::path::PathBuf;
    use std::str::FromStr;
    use std::sync::{Arc, Mutex};

    use super::{
        CapabilityDriver, CapabilityExecDriverInput, CapabilityFilePathArg, CapabilityNameArg,
        ProviderNameArg, TimeoutSecondsArg,
    };
    use usecase::capability_exec::{
        CapabilityDispatchOutcome, CapabilityExecError, CapabilityExecRequest,
        CapabilityExecService, DisciplineText, ProviderName,
    };
    use usecase::dry_write_driver::CapabilityName;

    struct StaticService {
        outcome: CapabilityDispatchOutcome,
    }

    impl CapabilityExecService for StaticService {
        fn execute(
            &self,
            _request: CapabilityExecRequest,
        ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
            Ok(self.outcome.clone())
        }
    }

    struct RecordingService {
        outcome: CapabilityDispatchOutcome,
        requests: Arc<Mutex<Vec<CapabilityExecRequest>>>,
    }

    impl CapabilityExecService for RecordingService {
        fn execute(
            &self,
            request: CapabilityExecRequest,
        ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
            self.requests.lock().expect("test request recorder lock").push(request);
            Ok(self.outcome.clone())
        }
    }

    fn input() -> CapabilityExecDriverInput {
        CapabilityExecDriverInput {
            capability: CapabilityNameArg::from_str("implementer").expect("valid test capability"),
            host: ProviderNameArg::from_str("claude").expect("valid test provider"),
            briefing_file: CapabilityFilePathArg::from_str("tmp/briefing.md")
                .expect("valid test briefing path"),
            timeout_seconds: None,
        }
    }

    #[test]
    fn test_timeout_seconds_arg_rejects_zero_and_non_numeric_values() {
        assert!(TimeoutSecondsArg::from_str("0").is_err());
        assert!(TimeoutSecondsArg::from_str("abc").is_err());
        assert!(TimeoutSecondsArg::from_str("1800").is_ok());
    }

    #[test]
    fn test_capability_driver_forwards_timeout_to_request() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let driver = CapabilityDriver::new(Arc::new(RecordingService {
            outcome: CapabilityDispatchOutcome::Executed {
                provider: ProviderName::try_new("codex").expect("valid test provider"),
                exit_code: 0,
            },
            requests: requests.clone(),
        }));
        let mut input = input();
        input.timeout_seconds =
            Some(TimeoutSecondsArg::from_str("1800").expect("valid test timeout"));

        let _ = driver.handle(input);

        let recorded = requests.lock().expect("test request recorder lock");
        let request = recorded.first().expect("one request is recorded");
        assert_eq!(request.timeout.map(|timeout| timeout.as_secs()), Some(1800));
    }

    #[test]
    fn test_capability_driver_executed_outcome_is_machine_discriminated() {
        let driver = CapabilityDriver::new(Arc::new(StaticService {
            outcome: CapabilityDispatchOutcome::Executed {
                provider: ProviderName::try_new("codex").expect("valid test provider"),
                exit_code: 0,
            },
        }));

        let outcome = driver.handle(input());

        assert_eq!(outcome.exit_code, 0);
        assert!(
            outcome
                .stdout
                .as_deref()
                .is_some_and(|output| output.starts_with("CAPABILITY_EXEC_OUTCOME: executed"))
        );
    }

    #[test]
    fn test_capability_driver_in_host_outcome_includes_required_payload() {
        let driver = CapabilityDriver::new(Arc::new(StaticService {
            outcome: CapabilityDispatchOutcome::DelegateInHost {
                capability: CapabilityName::try_new("implementer").expect("valid test capability"),
                briefing_file: usecase::capability_exec::CapabilityFilePath::try_new(
                    PathBuf::from("tmp/briefing.md"),
                )
                .expect("valid test briefing"),
                discipline: DisciplineText::try_new("no direct git writes".to_owned())
                    .expect("valid test discipline"),
            },
        }));

        let outcome = driver.handle(input());
        let output = outcome.stdout.expect("in-host output is present");

        assert_eq!(outcome.exit_code, 0);
        assert!(output.contains("CAPABILITY_EXEC_OUTCOME: delegate-in-host"));
        assert!(output.contains("capability: implementer"));
        assert!(output.contains("briefing_file: tmp/briefing.md"));
        assert!(output.contains("no direct git writes"));
    }

    #[test]
    fn test_capability_driver_forwards_shared_exec_inputs_to_generic_service() {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let driver = CapabilityDriver::new(Arc::new(RecordingService {
            outcome: CapabilityDispatchOutcome::Executed {
                provider: ProviderName::try_new("codex").expect("valid test provider"),
                exit_code: 0,
            },
            requests: requests.clone(),
        }));

        let outcome = driver.handle(input());

        assert_eq!(outcome.exit_code, 0);
        let recorded = requests.lock().expect("test request recorder lock");
        assert_eq!(recorded.len(), 1);
        let request = recorded.first().expect("one request is recorded");
        assert_eq!(request.capability.as_str(), "implementer");
        assert_eq!(request.host.as_str(), "claude");
        assert_eq!(request.briefing_file.as_path(), PathBuf::from("tmp/briefing.md"));
    }

    #[test]
    fn test_capability_name_arg_blank_value_rejected() {
        assert!(CapabilityNameArg::from_str(" ").is_err());
    }

    #[test]
    fn test_capability_file_path_arg_parent_directory_rejected() {
        assert!(CapabilityFilePathArg::from_str("../briefing.md").is_err());
    }
}
