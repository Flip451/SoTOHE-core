//! Validated values and error vocabulary for generic capability dispatch.
//!
//! This module deliberately contains no I/O, profile resolution, or provider
//! dispatch. Those concerns are introduced through ports and the interactor in
//! the follow-up task.

use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, LazyLock};

use crate::dry_write_driver::CapabilityName;

/// Validation error for a capability-dispatch input value.
#[derive(Debug, thiserror::Error)]
pub enum CapabilityInputValidationError {
    /// The supplied provider name was empty.
    #[error("provider name must not be empty")]
    EmptyProviderName,
    /// The supplied model name was empty.
    #[error("model name must not be empty")]
    EmptyModelName,
    /// The supplied file path was empty.
    #[error("file path must not be empty")]
    EmptyFilePath,
    /// The supplied file path was not a repository-relative, traversal-free path.
    #[error("file path must be repository-relative and traversal-free")]
    InvalidFilePath,
    /// The supplied technical text was empty or whitespace-only.
    #[error("content must not be empty")]
    EmptyContent,
    /// The supplied timeout was zero seconds.
    #[error("timeout seconds must be greater than zero")]
    ZeroTimeoutSeconds,
}

/// Validated technical provider identifier for capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderName(String);

impl ProviderName {
    /// Validates and wraps a provider identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyProviderName`] when
    /// `value` is empty or whitespace-only.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CapabilityInputValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyProviderName);
        }
        Ok(Self(value))
    }

    /// Returns the validated provider identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ProviderName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validated technical model identifier for capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelName(String);

impl ModelName {
    /// Validates and wraps a model identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyModelName`] when `value`
    /// is empty or whitespace-only.
    pub fn try_new(value: impl Into<String>) -> Result<Self, CapabilityInputValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyModelName);
        }
        Ok(Self(value))
    }

    /// Returns the validated model identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModelName {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Validated technical file path for a capability briefing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFilePath(PathBuf);

impl CapabilityFilePath {
    /// Validates and wraps a briefing-file path.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyFilePath`] when `path`
    /// has no path components, or [`CapabilityInputValidationError::InvalidFilePath`]
    /// when `path` is absolute or contains a parent-directory component.
    pub fn try_new(path: PathBuf) -> Result<Self, CapabilityInputValidationError> {
        if path.as_os_str().is_empty() {
            return Err(CapabilityInputValidationError::EmptyFilePath);
        }
        if path.is_absolute()
            || path
                .components()
                .any(|component| matches!(component, Component::ParentDir | Component::Prefix(_)))
        {
            return Err(CapabilityInputValidationError::InvalidFilePath);
        }
        Ok(Self(path))
    }

    /// Returns the validated briefing-file path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.0
    }
}

impl std::fmt::Display for CapabilityFilePath {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{}", self.as_path().display())
    }
}

/// Fixed provider identifier used by the Codex adapter.
pub static CODEX_PROVIDER_NAME: LazyLock<ProviderName> =
    LazyLock::new(|| ProviderName("codex".to_owned()));

/// Fixed provider identifier used by the Claude adapter.
pub static CLAUDE_PROVIDER_NAME: LazyLock<ProviderName> =
    LazyLock::new(|| ProviderName("claude".to_owned()));

/// Fixed path used by the repository-owned capability discipline source.
pub static CAPABILITY_EXEC_DISCIPLINE_PATH: LazyLock<CapabilityFilePath> = LazyLock::new(|| {
    CapabilityFilePath(PathBuf::from(".harness/prompts/capability-exec-discipline.md"))
});

/// Validated technical briefing text loaded by a source adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BriefingText(String);

impl BriefingText {
    /// Validates and wraps briefing content.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyContent`] when `value`
    /// is empty or whitespace-only.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyContent);
        }
        Ok(Self(value))
    }

    /// Returns the validated briefing text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated technical discipline text loaded by a source adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DisciplineText(String);

impl DisciplineText {
    /// Validates and wraps discipline content.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyContent`] when `value`
    /// is empty or whitespace-only.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyContent);
        }
        Ok(Self(value))
    }

    /// Returns the validated discipline text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Opaque diagnostic detail carried by capability-dispatch errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityFailureDetail(String);

impl CapabilityFailureDetail {
    /// Wraps presentation-only diagnostic text.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CapabilityFailureDetail {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// Execution category declared by a capability profile.
#[derive(Debug, PartialEq, Eq)]
pub enum ExecutionMode {
    /// The capability returns free-form output for an orchestrator to consume.
    OrchestratorOutput,
    /// The capability has a fixed, machine-consumed output contract.
    TypedPipeline,
}

impl Copy for ExecutionMode {}

impl Clone for ExecutionMode {
    fn clone(&self) -> Self {
        *self
    }
}

/// Validated positive provider-process timeout in seconds.
#[derive(Debug, PartialEq, Eq)]
pub struct TimeoutSeconds(u64);

impl Copy for TimeoutSeconds {}

impl Clone for TimeoutSeconds {
    fn clone(&self) -> Self {
        *self
    }
}

impl TimeoutSeconds {
    /// Validates and wraps a timeout value.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::ZeroTimeoutSeconds`] when
    /// `value` is zero.
    pub fn try_new(value: u64) -> Result<Self, CapabilityInputValidationError> {
        if value == 0 {
            return Err(CapabilityInputValidationError::ZeroTimeoutSeconds);
        }
        Ok(Self(value))
    }

    /// Returns the validated timeout in seconds.
    #[must_use]
    pub fn as_secs(&self) -> u64 {
        self.0
    }
}

/// Request values supplied to generic capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityExecRequest {
    /// Capability selected from the runtime profile.
    pub capability: CapabilityName,
    /// Provider of the host that invoked this command.
    pub host: ProviderName,
    /// Validated path to the capability briefing.
    pub briefing_file: CapabilityFilePath,
    /// Provider-process timeout; `None` waits without a time limit.
    pub timeout: Option<TimeoutSeconds>,
}

/// Typed routing data resolved from the capability profile.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityProfile {
    /// Provider selected by the profile.
    pub provider: ProviderName,
    /// Single model selected by the profile.
    pub model: ModelName,
    /// Execution category selected by the profile.
    pub execution_mode: ExecutionMode,
}

/// Fully resolved data supplied to a provider-native dispatch adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityDispatchRequest {
    /// The original validated command request.
    pub request: CapabilityExecRequest,
    /// The routing profile resolved for the requested capability.
    pub profile: CapabilityProfile,
    /// The validated contents of the requested briefing file.
    pub briefing: BriefingText,
    /// The validated common execution discipline.
    pub discipline: DisciplineText,
}

/// Observable result of a generic capability dispatch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityDispatchOutcome {
    /// A provider subprocess completed with its opaque process exit status.
    Executed {
        /// Provider that executed the subprocess.
        provider: ProviderName,
        /// Opaque process status returned by the provider executable.
        exit_code: u8,
    },
    /// The current host must invoke the validated native Claude adapter itself.
    DelegateInHost {
        /// Capability to invoke through the host-native mechanism.
        capability: CapabilityName,
        /// Validated briefing path to pass to the host-native mechanism.
        briefing_file: CapabilityFilePath,
        /// Validated discipline text that must accompany the briefing.
        discipline: DisciplineText,
    },
}

/// Application service for generic capability dispatch.
pub trait CapabilityExecService: Send + Sync {
    /// Dispatches a capability after validating all profile and adapter inputs.
    ///
    /// # Errors
    ///
    /// Returns a fail-closed [`CapabilityExecError`] before an in-host
    /// delegation or provider subprocess can occur when any input is invalid.
    fn execute(
        &self,
        request: CapabilityExecRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError>;
}

/// Resolves a capability routing profile from the runtime configuration.
pub trait CapabilityProfilePort: Send + Sync {
    /// Resolves the profile for `capability`.
    ///
    /// # Errors
    ///
    /// Implementations may return only [`CapabilityExecError::ProfileResolution`]
    /// or [`CapabilityExecError::ModelMissing`].
    fn resolve(
        &self,
        capability: &CapabilityName,
    ) -> Result<CapabilityProfile, CapabilityExecError>;
}

/// Loads and validates the common text inputs required by every dispatch branch.
pub trait CapabilitySourcePort: Send + Sync {
    /// Loads and validates the requested briefing content.
    ///
    /// # Errors
    ///
    /// Implementations return [`CapabilityExecError::SourceValidation`] for an
    /// unreadable, non-UTF-8, non-file, or empty source.
    fn load_briefing(&self, path: &CapabilityFilePath)
    -> Result<BriefingText, CapabilityExecError>;

    /// Loads and validates the fixed shared execution discipline.
    ///
    /// # Errors
    ///
    /// Implementations return [`CapabilityExecError::SourceValidation`] when
    /// the fixed template cannot be used.
    fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError>;
}

/// Provider-native dispatch adapter selected through the runtime profile.
pub trait CapabilityProviderPort: Send + Sync {
    /// Returns the provider identifier implemented by this adapter.
    fn provider(&self) -> &ProviderName;

    /// Performs provider-native preflight and dispatch.
    ///
    /// # Errors
    ///
    /// Implementations may return only [`CapabilityExecError::AdapterPreflight`]
    /// or [`CapabilityExecError::DispatchFailed`].
    fn dispatch(
        &self,
        request: &CapabilityDispatchRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError>;
}

/// Pure application-layer coordinator for generic capability dispatch.
pub struct CapabilityExecInteractor {
    profile: Arc<dyn CapabilityProfilePort>,
    source: Arc<dyn CapabilitySourcePort>,
    providers: Vec<Arc<dyn CapabilityProviderPort>>,
}

impl CapabilityExecInteractor {
    /// Creates an interactor from its profile, source, and provider ports.
    #[must_use]
    pub fn new(
        profile: Arc<dyn CapabilityProfilePort>,
        source: Arc<dyn CapabilitySourcePort>,
        providers: Vec<Arc<dyn CapabilityProviderPort>>,
    ) -> Self {
        Self { profile, source, providers }
    }
}

impl CapabilityExecService for CapabilityExecInteractor {
    fn execute(
        &self,
        request: CapabilityExecRequest,
    ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
        // Both shared sources are deliberately loaded before routing decisions.
        // This keeps every exit path fail-closed under the same discipline.
        let briefing = self.source.load_briefing(&request.briefing_file)?;
        let discipline = self.source.load_discipline()?;
        let profile = self.profile.resolve(&request.capability)?;

        if profile.execution_mode != ExecutionMode::OrchestratorOutput {
            return Err(CapabilityExecError::ExecutionModeRejected {
                capability: request.capability,
                mode: profile.execution_mode,
            });
        }

        let provider = self
            .providers
            .iter()
            .find(|adapter| adapter.provider() == &profile.provider)
            .ok_or_else(|| CapabilityExecError::UnsupportedProvider {
                provider: profile.provider.clone(),
            })?;

        provider.dispatch(&CapabilityDispatchRequest { request, profile, briefing, discipline })
    }
}

/// Errors that stop capability dispatch before a successful outcome.
#[derive(Debug, thiserror::Error)]
pub enum CapabilityExecError {
    /// Profile lookup or decoding failed for a capability.
    #[error("capability profile resolution failed for '{capability}': {detail}")]
    ProfileResolution {
        /// Capability whose profile could not be resolved.
        capability: CapabilityName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A profile's execution category is not eligible for generic dispatch.
    #[error("execution mode rejected for capability '{capability}': {mode:?}")]
    ExecutionModeRejected {
        /// Capability whose execution category was rejected.
        capability: CapabilityName,
        /// Rejected execution category.
        mode: ExecutionMode,
    },
    /// A profile omitted the single model required for dispatch.
    #[error("capability profile has no model for '{capability}'")]
    ModelMissing {
        /// Capability whose profile omitted its model.
        capability: CapabilityName,
    },
    /// A provider has no supported generic-dispatch adapter.
    #[error("unsupported capability provider '{provider}'")]
    UnsupportedProvider {
        /// Unsupported provider identifier.
        provider: ProviderName,
    },
    /// Briefing or discipline source validation failed.
    #[error("capability source validation failed for '{path}': {detail}")]
    SourceValidation {
        /// Source path associated with the failed validation.
        path: CapabilityFilePath,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A provider-native adapter did not meet its preflight requirements.
    #[error(
        "capability adapter preflight failed for '{capability}' on provider '{provider}': {detail}"
    )]
    AdapterPreflight {
        /// Capability whose adapter could not pass preflight.
        capability: CapabilityName,
        /// Provider whose adapter failed preflight.
        provider: ProviderName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
    /// A provider-native dispatch attempt failed.
    #[error("capability dispatch failed for provider '{provider}': {detail}")]
    DispatchFailed {
        /// Provider whose dispatch attempt failed.
        provider: ProviderName,
        /// Opaque adapter diagnostic.
        detail: CapabilityFailureDetail,
    },
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::indexing_slicing)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{
        BriefingText, CapabilityDispatchOutcome, CapabilityDispatchRequest, CapabilityExecError,
        CapabilityExecInteractor, CapabilityExecRequest, CapabilityExecService,
        CapabilityFailureDetail, CapabilityFilePath, CapabilityInputValidationError,
        CapabilityProfile, CapabilityProfilePort, CapabilityProviderPort, CapabilitySourcePort,
        DisciplineText, ExecutionMode, ModelName, ProviderName, TimeoutSeconds,
    };
    use crate::dry_write_driver::CapabilityName;

    struct StaticProfilePort {
        profile: CapabilityProfile,
        calls: Arc<AtomicUsize>,
    }

    impl CapabilityProfilePort for StaticProfilePort {
        fn resolve(
            &self,
            _capability: &CapabilityName,
        ) -> Result<CapabilityProfile, CapabilityExecError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.profile.clone())
        }
    }

    struct StaticSourcePort {
        briefing: Result<BriefingText, CapabilityFailureDetail>,
        discipline: Result<DisciplineText, CapabilityFailureDetail>,
        briefing_path: CapabilityFilePath,
        calls: Arc<AtomicUsize>,
    }

    impl CapabilitySourcePort for StaticSourcePort {
        fn load_briefing(
            &self,
            path: &CapabilityFilePath,
        ) -> Result<BriefingText, CapabilityExecError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.briefing.clone().map_err(|detail| CapabilityExecError::SourceValidation {
                path: path.clone(),
                detail,
            })
        }

        fn load_discipline(&self) -> Result<DisciplineText, CapabilityExecError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            self.discipline.clone().map_err(|detail| CapabilityExecError::SourceValidation {
                path: self.briefing_path.clone(),
                detail,
            })
        }
    }

    struct RecordingProviderPort {
        provider: ProviderName,
        outcome: CapabilityDispatchOutcome,
        dispatches: Arc<Mutex<Vec<CapabilityDispatchRequest>>>,
    }

    impl CapabilityProviderPort for RecordingProviderPort {
        fn provider(&self) -> &ProviderName {
            &self.provider
        }

        fn dispatch(
            &self,
            request: &CapabilityDispatchRequest,
        ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
            self.dispatches.lock().expect("test dispatch recorder lock").push(request.clone());
            Ok(self.outcome.clone())
        }
    }

    struct PreflightFailingProviderPort {
        provider: ProviderName,
        dispatches: Arc<AtomicUsize>,
    }

    impl CapabilityProviderPort for PreflightFailingProviderPort {
        fn provider(&self) -> &ProviderName {
            &self.provider
        }

        fn dispatch(
            &self,
            request: &CapabilityDispatchRequest,
        ) -> Result<CapabilityDispatchOutcome, CapabilityExecError> {
            self.dispatches.fetch_add(1, Ordering::SeqCst);
            Err(CapabilityExecError::AdapterPreflight {
                capability: request.request.capability.clone(),
                provider: self.provider.clone(),
                detail: CapabilityFailureDetail::new(
                    "adapter definition model does not match profile",
                ),
            })
        }
    }

    fn request() -> Result<CapabilityExecRequest, Box<dyn std::error::Error>> {
        Ok(CapabilityExecRequest {
            capability: CapabilityName::try_new("implementer")?,
            host: ProviderName::try_new("codex")?,
            briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
            timeout: None,
        })
    }

    fn profile(mode: ExecutionMode) -> Result<CapabilityProfile, CapabilityInputValidationError> {
        Ok(CapabilityProfile {
            provider: ProviderName::try_new("codex")?,
            model: ModelName::try_new("gpt-5")?,
            execution_mode: mode,
        })
    }

    #[test]
    fn test_provider_name_whitespace_rejected() {
        assert!(matches!(
            ProviderName::try_new(" \t\n "),
            Err(CapabilityInputValidationError::EmptyProviderName)
        ));
    }

    #[test]
    fn test_provider_name_valid_value_preserved() {
        assert!(matches!(
            ProviderName::try_new("codex"),
            Ok(provider) if provider.as_str() == "codex" && provider.to_string() == "codex"
        ));
    }

    #[test]
    fn test_model_name_whitespace_rejected() {
        assert!(matches!(
            ModelName::try_new(" "),
            Err(CapabilityInputValidationError::EmptyModelName)
        ));
    }

    #[test]
    fn test_model_name_valid_value_preserved() {
        assert!(matches!(
            ModelName::try_new("gpt-5"),
            Ok(model) if model.as_str() == "gpt-5" && model.to_string() == "gpt-5"
        ));
    }

    #[test]
    fn test_capability_file_path_empty_path_rejected() {
        assert!(matches!(
            CapabilityFilePath::try_new(PathBuf::new()),
            Err(CapabilityInputValidationError::EmptyFilePath)
        ));
    }

    #[test]
    fn test_capability_file_path_valid_path_preserved() {
        assert!(matches!(
            CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md")),
            Ok(path) if path.as_path() == "tmp/briefing.md"
                && path.to_string() == "tmp/briefing.md"
        ));
    }

    #[test]
    fn test_capability_file_path_absolute_and_traversal_paths_rejected() {
        for path in [PathBuf::from("/outside/briefing.md"), PathBuf::from("tmp/../briefing.md")] {
            assert!(matches!(
                CapabilityFilePath::try_new(path),
                Err(CapabilityInputValidationError::InvalidFilePath)
            ));
        }
    }

    #[test]
    fn test_briefing_text_whitespace_rejected() {
        assert!(matches!(
            BriefingText::try_new("\n  \t".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_briefing_text_valid_content_preserved() {
        assert!(matches!(
            BriefingText::try_new("Implement T001.".to_owned()),
            Ok(briefing) if briefing.as_str() == "Implement T001."
        ));
    }

    #[test]
    fn test_discipline_text_whitespace_rejected() {
        assert!(matches!(
            DisciplineText::try_new("\n  \t".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_discipline_text_valid_content_preserved() {
        assert!(matches!(
            DisciplineText::try_new("Do not stage changes.".to_owned()),
            Ok(discipline) if discipline.as_str() == "Do not stage changes."
        ));
    }

    #[test]
    fn test_capability_profile_valid_values_retained() -> Result<(), Box<dyn std::error::Error>> {
        let profile = CapabilityProfile {
            provider: ProviderName::try_new("codex")?,
            model: ModelName::try_new("gpt-5")?,
            execution_mode: ExecutionMode::OrchestratorOutput,
        };

        assert_eq!(profile.provider.as_str(), "codex");
        assert_eq!(profile.model.as_str(), "gpt-5");
        assert_eq!(profile.execution_mode, ExecutionMode::OrchestratorOutput);
        Ok(())
    }

    #[test]
    fn test_capability_exec_request_shared_capability_name_retained()
    -> Result<(), Box<dyn std::error::Error>> {
        let request = CapabilityExecRequest {
            capability: CapabilityName::try_new("implementer")?,
            host: ProviderName::try_new("codex")?,
            briefing_file: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
            timeout: Some(TimeoutSeconds::try_new(1800)?),
        };

        assert_eq!(request.capability.as_str(), "implementer");
        assert_eq!(request.host.as_str(), "codex");
        assert_eq!(request.briefing_file.as_path(), PathBuf::from("tmp/briefing.md"));
        assert_eq!(request.timeout.map(|timeout| timeout.as_secs()), Some(1800));
        Ok(())
    }

    #[test]
    fn test_timeout_seconds_zero_rejected() {
        assert!(matches!(
            TimeoutSeconds::try_new(0),
            Err(CapabilityInputValidationError::ZeroTimeoutSeconds)
        ));
    }

    #[test]
    fn test_timeout_seconds_positive_value_preserved() {
        assert!(matches!(
            TimeoutSeconds::try_new(600),
            Ok(timeout) if timeout.as_secs() == 600
        ));
    }

    #[test]
    fn test_execution_mode_distinct_variants_retained() {
        assert_ne!(ExecutionMode::OrchestratorOutput, ExecutionMode::TypedPipeline);
    }

    #[test]
    fn test_capability_failure_detail_input_preserved() {
        let detail = CapabilityFailureDetail::new("definition is missing");

        assert_eq!(detail.as_str(), "definition is missing");
        assert_eq!(detail.to_string(), "definition is missing");
    }

    #[test]
    fn test_capability_exec_error_all_variants_render_diagnostics()
    -> Result<(), Box<dyn std::error::Error>> {
        let detail = CapabilityFailureDetail::new("definition is missing");
        let variants = [
            CapabilityExecError::ProfileResolution {
                capability: CapabilityName::try_new("implementer")?,
                detail: detail.clone(),
            },
            CapabilityExecError::ExecutionModeRejected {
                capability: CapabilityName::try_new("implementer")?,
                mode: ExecutionMode::TypedPipeline,
            },
            CapabilityExecError::ModelMissing {
                capability: CapabilityName::try_new("implementer")?,
            },
            CapabilityExecError::UnsupportedProvider { provider: ProviderName::try_new("codex")? },
            CapabilityExecError::SourceValidation {
                path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                detail: detail.clone(),
            },
            CapabilityExecError::AdapterPreflight {
                capability: CapabilityName::try_new("implementer")?,
                provider: ProviderName::try_new("codex")?,
                detail: detail.clone(),
            },
            CapabilityExecError::DispatchFailed {
                provider: ProviderName::try_new("codex")?,
                detail,
            },
        ];

        for error in variants {
            assert!(!error.to_string().is_empty());
        }
        Ok(())
    }

    #[test]
    fn test_capability_exec_valid_orchestrator_output_dispatches_validated_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let source_calls = Arc::new(AtomicUsize::new(0));
        let profile_calls = Arc::new(AtomicUsize::new(0));
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let provider = ProviderName::try_new("codex")?;
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: profile_calls.clone(),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: source_calls.clone(),
            }),
            vec![Arc::new(RecordingProviderPort {
                provider: provider.clone(),
                outcome: CapabilityDispatchOutcome::Executed { provider, exit_code: 0 },
                dispatches: dispatches.clone(),
            })],
        );

        let outcome = interactor.execute(request()?)?;

        assert!(matches!(outcome, CapabilityDispatchOutcome::Executed { exit_code: 0, .. }));
        assert_eq!(source_calls.load(Ordering::SeqCst), 2);
        assert_eq!(profile_calls.load(Ordering::SeqCst), 1);
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].request.capability.as_str(), "implementer");
        assert_eq!(recorded[0].request.host.as_str(), "codex");
        assert_eq!(recorded[0].profile.provider.as_str(), "codex");
        assert_eq!(recorded[0].profile.model.as_str(), "gpt-5");
        assert_eq!(recorded[0].briefing.as_str(), "perform the task");
        assert_eq!(recorded[0].discipline.as_str(), "no direct git writes");
        Ok(())
    }

    #[test]
    fn test_capability_exec_claude_provider_dispatches_validated_request()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let provider = ProviderName::try_new("claude")?;
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: CapabilityProfile {
                    provider: provider.clone(),
                    model: ModelName::try_new("claude-opus")?,
                    execution_mode: ExecutionMode::OrchestratorOutput,
                },
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            vec![Arc::new(RecordingProviderPort {
                provider: provider.clone(),
                outcome: CapabilityDispatchOutcome::Executed { provider, exit_code: 0 },
                dispatches: dispatches.clone(),
            })],
        );

        let outcome = interactor.execute(request()?)?;

        assert!(matches!(
            outcome,
            CapabilityDispatchOutcome::Executed { ref provider, exit_code: 0 }
                if provider.as_str() == "claude"
        ));
        let recorded = dispatches.lock().expect("test dispatch recorder lock");
        assert_eq!(recorded.len(), 1);
        assert_eq!(recorded[0].profile.provider.as_str(), "claude");
        assert_eq!(recorded[0].profile.model.as_str(), "claude-opus");
        Ok(())
    }

    #[test]
    fn test_capability_exec_source_failure_prevents_profile_and_provider_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let source_calls = Arc::new(AtomicUsize::new(0));
        let profile_calls = Arc::new(AtomicUsize::new(0));
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: profile_calls.clone(),
            }),
            Arc::new(StaticSourcePort {
                briefing: Err(CapabilityFailureDetail::new("not readable")),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: source_calls.clone(),
            }),
            vec![Arc::new(RecordingProviderPort {
                provider: ProviderName::try_new("codex")?,
                outcome: CapabilityDispatchOutcome::Executed {
                    provider: ProviderName::try_new("codex")?,
                    exit_code: 0,
                },
                dispatches: dispatches.clone(),
            })],
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::SourceValidation { .. })
        ));
        assert_eq!(source_calls.load(Ordering::SeqCst), 1);
        assert_eq!(profile_calls.load(Ordering::SeqCst), 0);
        assert!(dispatches.lock().expect("test dispatch recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_capability_exec_typed_pipeline_rejected_before_provider_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::TypedPipeline)?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            vec![Arc::new(RecordingProviderPort {
                provider: ProviderName::try_new("codex")?,
                outcome: CapabilityDispatchOutcome::Executed {
                    provider: ProviderName::try_new("codex")?,
                    exit_code: 0,
                },
                dispatches: dispatches.clone(),
            })],
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::ExecutionModeRejected { .. })
        ));
        assert!(dispatches.lock().expect("test dispatch recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_capability_exec_unsupported_provider_rejected_before_provider_dispatch()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(Mutex::new(Vec::new()));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: CapabilityProfile {
                    provider: ProviderName::try_new("unsupported-provider")?,
                    model: ModelName::try_new("model-x")?,
                    execution_mode: ExecutionMode::OrchestratorOutput,
                },
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            vec![Arc::new(RecordingProviderPort {
                provider: ProviderName::try_new("codex")?,
                outcome: CapabilityDispatchOutcome::Executed {
                    provider: ProviderName::try_new("codex")?,
                    exit_code: 0,
                },
                dispatches: dispatches.clone(),
            })],
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::UnsupportedProvider { .. })
        ));
        assert!(dispatches.lock().expect("test dispatch recorder lock").is_empty());
        Ok(())
    }

    #[test]
    fn test_capability_exec_adapter_preflight_failure_returns_without_successful_outcome()
    -> Result<(), Box<dyn std::error::Error>> {
        let dispatches = Arc::new(AtomicUsize::new(0));
        let interactor = CapabilityExecInteractor::new(
            Arc::new(StaticProfilePort {
                profile: profile(ExecutionMode::OrchestratorOutput)?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            Arc::new(StaticSourcePort {
                briefing: Ok(BriefingText::try_new("perform the task".to_owned())?),
                discipline: Ok(DisciplineText::try_new("no direct git writes".to_owned())?),
                briefing_path: CapabilityFilePath::try_new(PathBuf::from("tmp/briefing.md"))?,
                calls: Arc::new(AtomicUsize::new(0)),
            }),
            vec![Arc::new(PreflightFailingProviderPort {
                provider: ProviderName::try_new("codex")?,
                dispatches: dispatches.clone(),
            })],
        );

        assert!(matches!(
            interactor.execute(request()?),
            Err(CapabilityExecError::AdapterPreflight { .. })
        ));
        assert_eq!(dispatches.load(Ordering::SeqCst), 1);
        Ok(())
    }
}
