//! Opaque diagnostic-text ValueObjects for `dry_write_driver` error variants.
//!
//! Extracted from `dry_write_driver` to keep the parent module under the
//! 700-line production-code limit (T038 added these seven types plus the
//! redesigned error enums that reference them).

// ── DiffHunkListingFailureDetail ──────────────────────────────────────────────

/// Opaque diagnostic-text carrier for a failed
/// `GitDryCheckDiffGetter::list_changed_hunks` call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunkListingFailureDetail(String);

impl DiffHunkListingFailureDetail {
    /// Wrap the rendered `Display` text of a failed diff-hunk listing call.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for DiffHunkListingFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── FragmentPipelineFailureDetail ─────────────────────────────────────────────

/// Opaque diagnostic-text carrier shared by `DryCorpusFragmentsError::FragmentExtractionFailed`
/// (`extract_code_fragments` over `workspace_root`) and
/// `FragmentPathNormalizationFailed` (the subsequent `CodeFragment`
/// path-rebuild step) — both are diagnostic text from the same
/// fragment-processing pipeline stage, distinguished by the enum variant
/// rather than by a second carrier type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FragmentPipelineFailureDetail(String);

impl FragmentPipelineFailureDetail {
    /// Wrap the rendered `Display` text of a failed fragment-pipeline step.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for FragmentPipelineFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── EmbeddingModelLoadFailureDetail ───────────────────────────────────────────

/// Opaque diagnostic-text carrier for a failed `FastEmbedAdapter::new()` call
/// (offline-cache preflight or `fastembed-rs` model init failure).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EmbeddingModelLoadFailureDetail(String);

impl EmbeddingModelLoadFailureDetail {
    /// Wrap the rendered `Display` text of a failed embedding-model load.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for EmbeddingModelLoadFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── SemanticIndexOpenFailureDetail ────────────────────────────────────────────

/// Opaque diagnostic-text carrier for a failed
/// `persistent_index::open_persistent_index_with_corpus` call (LanceDB
/// open/create failure, index cache marker read/write failure, or symlink
/// guard rejection of the index cache path).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticIndexOpenFailureDetail(String);

impl SemanticIndexOpenFailureDetail {
    /// Wrap the rendered `Display` text of a failed semantic-index open.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SemanticIndexOpenFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── AgentConfigResolutionFailureDetail ────────────────────────────────────────

/// Opaque diagnostic-text carrier for a failed
/// `checker_config::resolve_dry_checker_config` call (agent-profiles.json
/// symlink guard rejection, load/parse failure, no model configured for the
/// capability, or an invalid `reasoning_effort` value).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentConfigResolutionFailureDetail(String);

impl AgentConfigResolutionFailureDetail {
    /// Wrap the rendered `Display` text of a failed agent-config resolution.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentConfigResolutionFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

// ── CapabilityName ────────────────────────────────────────────────────────────

/// Validated non-empty capability lookup key used to query
/// `.harness/config/agent-profiles.json` (e.g. `"dry-checker"`), carried by
/// `DryCheckServiceFactoryError::AgentConfigResolutionFailed` so the failed
/// lookup names which capability was being resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityName(domain::NonEmptyString);

impl CapabilityName {
    /// Validate and wrap `value` as a [`CapabilityName`].
    ///
    /// # Errors
    ///
    /// Returns [`domain::ValidationError`] when `value` is empty or
    /// whitespace-only.
    pub fn try_new(value: impl Into<String>) -> Result<Self, domain::ValidationError> {
        Ok(Self(domain::NonEmptyString::try_new(value)?))
    }

    /// Borrow the validated capability name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl std::fmt::Display for CapabilityName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

// ── SerializationFailureDetail ────────────────────────────────────────────────

/// Opaque diagnostic-text carrier for a `serde_json::Error` produced when the
/// `dry-check-corpus-root.json` manifest DTO fails to serialize.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SerializationFailureDetail(String);

impl SerializationFailureDetail {
    /// Wrap the rendered `Display` text of a failed serialization.
    pub fn new(detail: impl Into<String>) -> Self {
        Self(detail.into())
    }

    /// Borrow the wrapped diagnostic text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SerializationFailureDetail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
