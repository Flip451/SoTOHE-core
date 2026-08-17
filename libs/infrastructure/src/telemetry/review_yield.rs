//! Serde-boundary DTO for one structured local review round.

use std::str::FromStr;

use domain::review_v2::{RoundType, ScopeName};
use serde::de::Error as _;
use serde::ser::SerializeStruct as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use usecase::capability_exec::{ModelName, ProviderName, ReasoningEffort};
use usecase::review_v2::ResolvedReviewerAssignment;
use usecase::telemetry::review_yield::ReviewFindingCount;

use crate::agent_profiles::ReasoningEffortDto;

/// Private-field serde DTO for one completed structured local review round.
///
/// The persisted representation uses strings for the validated domain and
/// usecase values because those types intentionally do not depend on serde.
/// Deserialization validates each string before constructing this DTO, so the
/// rest of the infrastructure layer can consume typed values.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructuredReviewRoundDto {
    scope: ScopeName,
    round_type: RoundType,
    provider: ProviderName,
    model: ModelName,
    reasoning_effort: ReasoningEffort,
    findings_count: ReviewFindingCount,
}

impl StructuredReviewRoundDto {
    /// Creates a structured review-round DTO from validated layer values.
    #[must_use]
    pub fn new(
        assignment: &ResolvedReviewerAssignment,
        round_type: RoundType,
        findings_count: ReviewFindingCount,
    ) -> Self {
        Self {
            scope: assignment.scope().clone(),
            round_type,
            provider: assignment.provider().clone(),
            model: assignment.model().clone(),
            reasoning_effort: assignment.reasoning_effort(),
            findings_count,
        }
    }

    fn from_wire_parts(
        scope: ScopeName,
        round_type: RoundType,
        provider: ProviderName,
        model: ModelName,
        reasoning_effort: ReasoningEffort,
        findings_count: ReviewFindingCount,
    ) -> Self {
        Self { scope, round_type, provider, model, reasoning_effort, findings_count }
    }

    /// Returns the validated review scope.
    #[must_use]
    pub fn scope(&self) -> &ScopeName {
        &self.scope
    }

    /// Returns the review round type.
    #[must_use]
    pub fn round_type(&self) -> RoundType {
        self.round_type
    }

    /// Returns the resolved reviewer provider.
    #[must_use]
    pub fn provider(&self) -> &ProviderName {
        &self.provider
    }

    /// Returns the resolved reviewer model.
    #[must_use]
    pub fn model(&self) -> &ModelName {
        &self.model
    }

    /// Returns the resolved reviewer reasoning effort.
    #[must_use]
    pub fn reasoning_effort(&self) -> ReasoningEffort {
        self.reasoning_effort
    }

    /// Returns the finding count produced by the round.
    #[must_use]
    pub fn findings_count(&self) -> ReviewFindingCount {
        self.findings_count
    }
}

impl Serialize for StructuredReviewRoundDto {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("StructuredReviewRoundDto", 6)?;
        state.serialize_field("scope", &self.scope.to_string())?;
        state.serialize_field("round_type", &self.round_type.to_string())?;
        state.serialize_field("provider", &self.provider.to_string())?;
        state.serialize_field("model", &self.model.to_string())?;
        state.serialize_field("reasoning_effort", reasoning_effort_label(self.reasoning_effort))?;
        state.serialize_field("findings_count", &self.findings_count.value())?;
        state.end()
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct StructuredReviewRoundWire {
    scope: String,
    round_type: String,
    provider: String,
    model: String,
    reasoning_effort: ReasoningEffortDto,
    findings_count: u32,
}

impl<'de> Deserialize<'de> for StructuredReviewRoundDto {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = StructuredReviewRoundWire::deserialize(deserializer)?;
        let scope = ScopeName::parse(&wire.scope).map_err(D::Error::custom)?;
        let round_type = RoundType::from_str(&wire.round_type).map_err(D::Error::custom)?;
        let provider = ProviderName::try_new(wire.provider).map_err(D::Error::custom)?;
        let model = ModelName::try_new(wire.model).map_err(D::Error::custom)?;

        Ok(Self::from_wire_parts(
            scope,
            round_type,
            provider,
            model,
            wire.reasoning_effort.into_domain(),
            ReviewFindingCount::new(wire.findings_count),
        ))
    }
}

fn reasoning_effort_label(effort: ReasoningEffort) -> &'static str {
    match effort {
        ReasoningEffort::Low => "low",
        ReasoningEffort::Medium => "medium",
        ReasoningEffort::High => "high",
        ReasoningEffort::XHigh => "xhigh",
        ReasoningEffort::Max => "max",
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;

    fn sample_assignment() -> ResolvedReviewerAssignment {
        ResolvedReviewerAssignment::new(
            domain::TrackId::try_new("review-yield-test").expect("valid track id"),
            ScopeName::parse("application").expect("valid scope"),
            ProviderName::try_new("codex").expect("valid provider"),
            ModelName::try_new("gpt-5.4-mini").expect("valid model"),
            ReasoningEffort::High,
        )
    }

    fn sample_round() -> StructuredReviewRoundDto {
        StructuredReviewRoundDto::new(
            &sample_assignment(),
            RoundType::Fast,
            ReviewFindingCount::new(3),
        )
    }

    #[test]
    fn test_structured_review_round_new_preserves_typed_values() {
        let round = sample_round();

        assert_eq!(round.scope().to_string(), "application");
        assert_eq!(round.round_type(), RoundType::Fast);
        assert_eq!(round.provider().as_str(), "codex");
        assert_eq!(round.model().as_str(), "gpt-5.4-mini");
        assert_eq!(round.reasoning_effort(), ReasoningEffort::High);
        assert_eq!(round.findings_count().value(), 3);
    }

    #[test]
    fn test_structured_review_round_serialization_writes_wire_fields() {
        let json = serde_json::to_value(sample_round()).expect("DTO should serialize");

        assert_eq!(json.get("scope").unwrap(), "application");
        assert_eq!(json.get("round_type").unwrap(), "fast");
        assert_eq!(json.get("provider").unwrap(), "codex");
        assert_eq!(json.get("model").unwrap(), "gpt-5.4-mini");
        assert_eq!(json.get("reasoning_effort").unwrap(), "high");
        assert_eq!(json.get("findings_count").unwrap(), 3);
    }

    #[test]
    fn test_structured_review_round_deserialization_restores_typed_values() {
        let json = r#"{
            "scope":"application",
            "round_type":"final",
            "provider":"claude",
            "model":"claude-sonnet",
            "reasoning_effort":"max",
            "findings_count":0
        }"#;

        let round: StructuredReviewRoundDto =
            serde_json::from_str(json).expect("valid DTO JSON should deserialize");

        assert_eq!(round.scope().to_string(), "application");
        assert_eq!(round.round_type(), RoundType::Final);
        assert_eq!(round.provider().as_str(), "claude");
        assert_eq!(round.model().as_str(), "claude-sonnet");
        assert_eq!(round.reasoning_effort(), ReasoningEffort::Max);
        assert_eq!(round.findings_count().value(), 0);
    }

    #[test]
    fn test_structured_review_round_deserialization_rejects_invalid_scope() {
        let json = r#"{
            "scope":"",
            "round_type":"fast",
            "provider":"codex",
            "model":"gpt-5.4-mini",
            "reasoning_effort":"high",
            "findings_count":1
        }"#;

        let result = serde_json::from_str::<StructuredReviewRoundDto>(json);

        assert!(result.is_err(), "an invalid scope must be rejected");
    }

    #[test]
    fn test_structured_review_round_deserialization_rejects_excluded_dry_round_type() {
        let json = r#"{
            "scope":"application",
            "round_type":"dry",
            "provider":"codex",
            "model":"gpt-5.4-mini",
            "reasoning_effort":"high",
            "findings_count":1
        }"#;

        let result = serde_json::from_str::<StructuredReviewRoundDto>(json);

        assert!(result.is_err(), "dry-check rounds must not become structured review DTOs");
    }

    #[test]
    fn test_structured_review_round_deserialization_rejects_unknown_wire_field() {
        let json = r#"{
            "scope":"application",
            "round_type":"fast",
            "provider":"codex",
            "model":"gpt-5.4-mini",
            "reasoning_effort":"high",
            "findings_count":1,
            "unexpected":true
        }"#;

        let result = serde_json::from_str::<StructuredReviewRoundDto>(json);

        assert!(result.is_err(), "unknown DTO fields must be rejected");
    }
}
