//! Validated technical values every capability dispatch is assembled from.
//!
//! Extracted from `capability_exec` to keep the parent module under the
//! 700-line production-code limit; the parent re-exports every item here, so
//! the module is an internal split rather than a new public path. The parent
//! keeps the dispatch contract itself — the request, the ports, the
//! interactor, and the error vocabulary — and this module keeps the values
//! those contracts are stated in.

use std::path::{Component, Path, PathBuf};
use std::sync::LazyLock;

use crate::conventions_resolve::ConventionResolution;

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
    /// The supplied target-artifact collection was empty.
    #[error("target artifact set must not be empty")]
    EmptyTargetArtifactSet,
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

/// Validated normalized repository-relative artifact path used in capability
/// session identity.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TargetArtifactPath {
    path: PathBuf,
}

impl TargetArtifactPath {
    /// Validates and normalizes a repository-relative artifact path.
    ///
    /// # Errors
    ///
    /// Returns [`CapabilityInputValidationError::EmptyFilePath`] when `path`
    /// has no path components, or [`CapabilityInputValidationError::InvalidFilePath`]
    /// when it cannot identify a repository-relative artifact.
    pub fn try_new(path: PathBuf) -> Result<Self, CapabilityInputValidationError> {
        if path.as_os_str().is_empty() {
            return Err(CapabilityInputValidationError::EmptyFilePath);
        }
        if path.is_absolute() {
            return Err(CapabilityInputValidationError::InvalidFilePath);
        }

        let mut normalized = PathBuf::new();
        for component in path.components() {
            match component {
                Component::Normal(value) => normalized.push(value),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(CapabilityInputValidationError::InvalidFilePath);
                }
            }
        }
        if normalized.as_os_str().is_empty() {
            return Err(CapabilityInputValidationError::InvalidFilePath);
        }
        Ok(Self { path: normalized })
    }

    /// Returns the normalized repository-relative path.
    #[must_use]
    pub fn as_path(&self) -> &Path {
        &self.path
    }
}

/// Non-empty sorted deduplicated target-artifact identity for capability resume.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TargetArtifactSet {
    paths: Vec<TargetArtifactPath>,
}

impl TargetArtifactSet {
    /// Builds a canonical artifact identity from one or more validated paths.
    ///
    /// # Errors
    ///
    /// Returns only [`CapabilityInputValidationError::EmptyTargetArtifactSet`]
    /// when `paths` is empty.
    pub fn try_new(
        mut paths: Vec<TargetArtifactPath>,
    ) -> Result<Self, CapabilityInputValidationError> {
        if paths.is_empty() {
            return Err(CapabilityInputValidationError::EmptyTargetArtifactSet);
        }
        paths.sort();
        paths.dedup();
        Ok(Self { paths })
    }

    /// Returns the canonical sorted artifact paths.
    #[must_use]
    pub fn as_slice(&self) -> &[TargetArtifactPath] {
        &self.paths
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

    /// Returns this discipline text with the capability's resolved conventions
    /// appended as a reading obligation (`AC-10`, `CN-05`).
    ///
    /// This is the one site that renders a resolution into dispatch text.
    /// Because both dispatch outcomes carry the value produced here, the
    /// provider-executed route and the in-host delegation route are incapable
    /// of stating different obligations rather than kept equal by review. Taking
    /// the resolution value instead of a pre-rendered string is what keeps that
    /// true: a caller has nothing to pass but the resolver's own result, so a
    /// second rendering cannot appear in a route branch.
    ///
    /// The obligation names the document paths the resolution holds and nothing
    /// else — no fixed convention filename and no section heading (`CN-06`) —
    /// and it summarizes no document body (`CN-05`); the reader is told to read
    /// each listed document in full. The rendered text introduces itself as
    /// running prose rather than under a heading, so the appended obligation
    /// contributes no heading of its own to the dispatched document. A
    /// resolution that matched nothing states that it resolved to zero
    /// documents, so the routes agree on the empty case as well and it is never
    /// mistaken for conventions having gone unresolved.
    ///
    /// Infallible: `self` is non-empty by construction and appending to it
    /// cannot make it empty.
    #[must_use]
    pub fn with_conventions(&self, conventions: &ConventionResolution) -> Self {
        let mut rendered = self.0.clone();
        if conventions.is_empty() {
            rendered.push_str(
                "\n\nConvention resolution ran for this capability and returned zero project \
                 convention documents, so this dispatch carries no convention reading \
                 obligation.\n",
            );
            return Self(rendered);
        }
        rendered.push_str(
            "\n\nConvention resolution ran for this capability and returned the project \
             convention documents listed below. Read each of them in full before acting; this \
             list is the whole of what applies.\n",
        );
        for document in conventions.documents() {
            rendered.push_str("\n- ");
            rendered.push_str(&document.to_string());
        }
        rendered.push('\n');
        Self(rendered)
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

/// Provider-independent reasoning effort selected by a capability profile.
#[derive(Debug, Copy, PartialEq, Eq)]
pub enum ReasoningEffort {
    /// Lowest supported reasoning effort.
    Low,
    /// Medium reasoning effort.
    Medium,
    /// High reasoning effort.
    High,
    /// Extra-high reasoning effort, above `High`.
    XHigh,
    /// Highest supported reasoning effort.
    Max,
}

impl Clone for ReasoningEffort {
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
