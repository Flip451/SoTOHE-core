//! Parsed YAML front-matter of a single ADR file as a domain value object.

use thiserror::Error;

use super::entry::AdrDecisionEntry;

/// Validation errors for [`AdrFrontMatter`] construction.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrFrontMatterError {
    /// The `adr_id` field must not be empty.
    #[error("adr_id must not be empty")]
    EmptyAdrId,
}

/// Parsed representation of a single ADR file's YAML front-matter block.
///
/// Wraps the deserialized `adr_id` + `decisions[]` payload as a domain value
/// object. Construction from raw YAML text is delegated to the infrastructure
/// `parse_adr_frontmatter` free function (T003); the domain holds only the
/// validated form. No serde derives — deserialization lives in the
/// infrastructure adapter per the CN-05 hexagonal rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrFrontMatter {
    adr_id: String,
    decisions: Vec<AdrDecisionEntry>,
}

impl AdrFrontMatter {
    /// Construct a new [`AdrFrontMatter`] from the validated parts.
    ///
    /// # Errors
    ///
    /// Returns [`AdrFrontMatterError::EmptyAdrId`] when `adr_id` is empty.
    pub fn new(
        adr_id: impl Into<String>,
        decisions: Vec<AdrDecisionEntry>,
    ) -> Result<Self, AdrFrontMatterError> {
        let adr_id = adr_id.into();
        if adr_id.is_empty() {
            return Err(AdrFrontMatterError::EmptyAdrId);
        }
        Ok(Self { adr_id, decisions })
    }

    /// The `adr_id` field — the ADR's stable identifier (filename slug).
    #[must_use]
    pub fn adr_id(&self) -> &str {
        &self.adr_id
    }

    /// The decision entries in lifecycle order.
    #[must_use]
    pub fn decisions(&self) -> &[AdrDecisionEntry] {
        &self.decisions
    }

    /// Consume the front-matter and yield owned decisions for downstream
    /// pipelines that iterate by value.
    #[must_use]
    pub fn into_decisions(self) -> Vec<AdrDecisionEntry> {
        self.decisions
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::adr_decision::{AdrDecisionCommon, ProposedDecision};

    fn proposed(id: &str) -> AdrDecisionEntry {
        AdrDecisionEntry::ProposedDecision(ProposedDecision::new(
            AdrDecisionCommon::new(id, None, None, None, false).unwrap(),
        ))
    }

    #[test]
    fn test_adr_front_matter_new_records_id_and_decisions() {
        let fm = AdrFrontMatter::new("2026-04-27-1234-foo", vec![proposed("D1"), proposed("D2")])
            .unwrap();
        assert_eq!(fm.adr_id(), "2026-04-27-1234-foo");
        assert_eq!(fm.decisions().len(), 2);
    }

    #[test]
    fn test_adr_front_matter_into_decisions_yields_owned_vec() {
        let fm = AdrFrontMatter::new("foo", vec![proposed("D1")]).unwrap();
        let decisions = fm.into_decisions();
        assert_eq!(decisions.len(), 1);
    }

    #[test]
    fn test_adr_front_matter_with_empty_decisions_is_allowed() {
        let fm = AdrFrontMatter::new("empty", vec![]).unwrap();
        assert_eq!(fm.adr_id(), "empty");
        assert!(fm.decisions().is_empty());
    }

    #[test]
    fn test_adr_front_matter_with_empty_adr_id_returns_error() {
        let result = AdrFrontMatter::new("", vec![]);
        assert!(matches!(result, Err(AdrFrontMatterError::EmptyAdrId)));
    }
}
