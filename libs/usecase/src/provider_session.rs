//! Typed provider-session cache contracts shared by reviewer and capability dispatch.

use std::hash::{Hash, Hasher};

use domain::review_v2::{RoundType, ScopeName};
use domain::{CommitHash, TrackId};

use crate::capability_exec::{
    CapabilityInputValidationError, ModelName, ProviderName, ReasoningEffort, TargetArtifactSet,
};
use crate::dry_write_driver::CapabilityName;
use crate::git_workflow::DiagnosticText;

/// Validated non-empty provider-native session identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSessionId {
    value: String,
}

impl ProviderSessionId {
    /// Validates a provider-native session identifier.
    ///
    /// # Errors
    ///
    /// Returns `CapabilityInputValidationError::EmptyContent` for empty input.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyContent);
        }
        Ok(Self { value })
    }

    /// Returns the validated provider-native session identifier.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Validated non-empty reviewer prompt used to start or resume a session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerPrompt {
    value: String,
}

impl ReviewerPrompt {
    /// Validates a reviewer prompt fragment.
    ///
    /// # Errors
    ///
    /// Returns `CapabilityInputValidationError::EmptyContent` for empty input.
    pub fn try_new(value: String) -> Result<Self, CapabilityInputValidationError> {
        if value.trim().is_empty() {
            return Err(CapabilityInputValidationError::EmptyContent);
        }
        Ok(Self { value })
    }

    /// Returns the validated prompt fragment.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

/// Provider-session cache payload bound only to its resolved provider and model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSessionCacheEntry {
    session_id: ProviderSessionId,
    provider: ProviderName,
    model: ModelName,
    effort: ReasoningEffort,
}

impl ProviderSessionCacheEntry {
    /// Creates an entry for a successfully established provider session.
    #[must_use]
    pub fn new(
        session_id: ProviderSessionId,
        provider: ProviderName,
        model: ModelName,
        effort: ReasoningEffort,
    ) -> Self {
        Self { session_id, provider, model, effort }
    }

    /// Returns the provider-native session identifier.
    #[must_use]
    pub fn session_id(&self) -> &ProviderSessionId {
        &self.session_id
    }

    /// Returns the provider that created the session.
    #[must_use]
    pub fn provider(&self) -> &ProviderName {
        &self.provider
    }

    /// Returns the model used to create the session.
    #[must_use]
    pub fn model(&self) -> &ModelName {
        &self.model
    }

    /// Returns the explicitly selected reasoning effort.
    #[must_use]
    pub fn effort(&self) -> ReasoningEffort {
        self.effort
    }
}

/// Separates reviewer, track capability, and workspace capability sessions by identity.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProviderSessionCacheKey {
    /// A reviewer session for one track, scope, round type, and review diff base.
    Review { track_id: TrackId, scope: ScopeName, round_type: RoundType, diff_base: CommitHash },
    /// A track-bound capability session for one capability.
    TrackCapability { track_id: TrackId, capability: CapabilityName },
    /// A workspace-bound capability session for one capability and artifact identity.
    WorkspaceCapability { capability: CapabilityName, target_artifacts: TargetArtifactSet },
}

impl Hash for ProviderSessionCacheKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        match self {
            Self::Review { track_id, scope, round_type, diff_base } => {
                0_u8.hash(state);
                track_id.hash(state);
                scope.hash(state);
                match round_type {
                    RoundType::Fast => 0_u8.hash(state),
                    RoundType::Final => 1_u8.hash(state),
                }
                diff_base.hash(state);
            }
            Self::TrackCapability { track_id, capability } => {
                1_u8.hash(state);
                track_id.hash(state);
                capability.as_str().hash(state);
            }
            Self::WorkspaceCapability { capability, target_artifacts } => {
                2_u8.hash(state);
                capability.as_str().hash(state);
                target_artifacts.hash(state);
            }
        }
    }
}

/// Classifies machine-local session cache failures without leaking local paths.
#[derive(Debug, thiserror::Error)]
pub enum ProviderSessionCacheError {
    /// Cache storage could not be read, written, or synchronized.
    #[error("provider session cache storage unavailable: {0}")]
    StorageUnavailable(DiagnosticText),
    /// A persisted cache entry could not be decoded, encoded, or validated.
    #[error("provider session cache entry invalid: {0}")]
    EntryInvalid(DiagnosticText),
    /// The cache location crossed a trusted identity boundary.
    #[error("provider session cache identity boundary violation: {0}")]
    IdentityBoundaryViolation(DiagnosticText),
}

/// Synchronous machine-local provider-session cache port.
pub trait ProviderSessionCachePort: Send + Sync {
    /// Loads the session entry for a validated key, if one exists.
    fn load(
        &self,
        key: &ProviderSessionCacheKey,
    ) -> Result<Option<ProviderSessionCacheEntry>, ProviderSessionCacheError>;

    /// Saves a session entry under a validated key.
    fn save(
        &self,
        key: &ProviderSessionCacheKey,
        entry: &ProviderSessionCacheEntry,
    ) -> Result<(), ProviderSessionCacheError>;

    /// Removes the entry selected by a validated key.
    fn remove(&self, key: &ProviderSessionCacheKey) -> Result<(), ProviderSessionCacheError>;
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::{
        ProviderSessionCacheEntry, ProviderSessionCacheError, ProviderSessionId, ReviewerPrompt,
    };
    use crate::capability_exec::{
        CapabilityInputValidationError, ModelName, ProviderName, ReasoningEffort,
    };
    use crate::git_workflow::DiagnosticText;

    #[test]
    fn test_provider_session_identifiers_reject_empty_content() {
        assert!(matches!(
            ProviderSessionId::try_new(" ".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
        assert!(matches!(
            ReviewerPrompt::try_new("\t".to_owned()),
            Err(CapabilityInputValidationError::EmptyContent)
        ));
    }

    #[test]
    fn test_provider_session_entry_retains_resolved_profile()
    -> Result<(), Box<dyn std::error::Error>> {
        let entry = ProviderSessionCacheEntry::new(
            ProviderSessionId::try_new("session-1".to_owned())?,
            ProviderName::try_new("codex")?,
            ModelName::try_new("gpt-5")?,
            ReasoningEffort::High,
        );

        assert_eq!(entry.session_id().as_str(), "session-1");
        assert_eq!(entry.provider().as_str(), "codex");
        assert_eq!(entry.model().as_str(), "gpt-5");
        assert_eq!(entry.effort(), ReasoningEffort::High);
        Ok(())
    }

    #[test]
    fn test_provider_session_cache_error_display_returns_semantic_diagnostics() {
        let storage =
            ProviderSessionCacheError::StorageUnavailable(DiagnosticText::new("permission denied"));
        let entry = ProviderSessionCacheError::EntryInvalid(DiagnosticText::new("invalid JSON"));
        let boundary = ProviderSessionCacheError::IdentityBoundaryViolation(DiagnosticText::new(
            "symlink encountered",
        ));

        assert_eq!(
            storage.to_string(),
            "provider session cache storage unavailable: permission denied"
        );
        assert_eq!(entry.to_string(), "provider session cache entry invalid: invalid JSON");
        assert_eq!(
            boundary.to_string(),
            "provider session cache identity boundary violation: symlink encountered"
        );
    }
}
