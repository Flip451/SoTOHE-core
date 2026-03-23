//! Domain types for structured spec documents (spec.json SSoT).
//!
//! `SpecDocument` is the aggregate root for a feature specification.
//! `spec.json` is the SSoT; `spec.md` is a read-only rendered view.

use crate::{ConfidenceSignal, SignalCounts, classify_source_tag};

// ---------------------------------------------------------------------------
// Value objects
// ---------------------------------------------------------------------------

/// A single requirement item with provenance sources.
///
/// Used in Scope (in/out), Constraints, and Acceptance Criteria.
///
/// # Errors
///
/// Returns `SpecValidationError::EmptyRequirementText` if text is empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRequirement {
    text: String,
    sources: Vec<String>,
}

impl SpecRequirement {
    /// Creates a new requirement.
    ///
    /// # Errors
    ///
    /// Returns error if `text` is empty or whitespace-only.
    pub fn new(text: impl Into<String>, sources: Vec<String>) -> Result<Self, SpecValidationError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(SpecValidationError::EmptyRequirementText);
        }
        Ok(Self { text, sources })
    }

    /// Returns the requirement text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the provenance sources.
    #[must_use]
    pub fn sources(&self) -> &[String] {
        &self.sources
    }

    /// Evaluates the confidence signal for this requirement.
    ///
    /// Multi-source policy: the signal is the highest confidence among all sources.
    /// Empty sources → Red (MissingSource).
    #[must_use]
    pub fn signal(&self) -> ConfidenceSignal {
        evaluate_requirement_signal(&self.sources)
    }
}

/// A domain state entry from the `## Domain States` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainStateEntry {
    name: String,
    description: String,
}

impl DomainStateEntry {
    /// Creates a new domain state entry.
    ///
    /// # Errors
    ///
    /// Returns error if `name` is empty or whitespace-only.
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
    ) -> Result<Self, SpecValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SpecValidationError::EmptyDomainStateName);
        }
        Ok(Self { name, description: description.into() })
    }

    /// Returns the state name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the state description.
    #[must_use]
    pub fn description(&self) -> &str {
        &self.description
    }
}

/// Scope section with in-scope and out-of-scope requirements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecScope {
    in_scope: Vec<SpecRequirement>,
    out_of_scope: Vec<SpecRequirement>,
}

impl SpecScope {
    /// Creates a new scope.
    #[must_use]
    pub fn new(in_scope: Vec<SpecRequirement>, out_of_scope: Vec<SpecRequirement>) -> Self {
        Self { in_scope, out_of_scope }
    }

    /// Returns in-scope requirements.
    #[must_use]
    pub fn in_scope(&self) -> &[SpecRequirement] {
        &self.in_scope
    }

    /// Returns out-of-scope requirements.
    #[must_use]
    pub fn out_of_scope(&self) -> &[SpecRequirement] {
        &self.out_of_scope
    }
}

/// An additional free-form section.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecSection {
    title: String,
    content: Vec<String>,
}

impl SpecSection {
    /// Creates a new section.
    ///
    /// # Errors
    ///
    /// Returns error if `title` is empty or whitespace-only.
    pub fn new(
        title: impl Into<String>,
        content: Vec<String>,
    ) -> Result<Self, SpecValidationError> {
        let title = title.into();
        if title.trim().is_empty() {
            return Err(SpecValidationError::EmptySectionTitle);
        }
        Ok(Self { title, content })
    }

    /// Returns the section title.
    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    /// Returns the section content lines.
    #[must_use]
    pub fn content(&self) -> &[String] {
        &self.content
    }
}

// ---------------------------------------------------------------------------
// Aggregate root
// ---------------------------------------------------------------------------

/// The aggregate root for a feature specification (spec.json SSoT).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecDocument {
    title: String,
    status: String,
    version: String,
    goal: Vec<String>,
    scope: SpecScope,
    constraints: Vec<SpecRequirement>,
    domain_states: Vec<DomainStateEntry>,
    acceptance_criteria: Vec<SpecRequirement>,
    additional_sections: Vec<SpecSection>,
    related_conventions: Vec<String>,
    signals: Option<SignalCounts>,
}

impl SpecDocument {
    /// Creates a new spec document.
    ///
    /// # Errors
    ///
    /// Returns error if `title`, `status`, or `version` is empty.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: impl Into<String>,
        status: impl Into<String>,
        version: impl Into<String>,
        goal: Vec<String>,
        scope: SpecScope,
        constraints: Vec<SpecRequirement>,
        domain_states: Vec<DomainStateEntry>,
        acceptance_criteria: Vec<SpecRequirement>,
        additional_sections: Vec<SpecSection>,
        related_conventions: Vec<String>,
        signals: Option<SignalCounts>,
    ) -> Result<Self, SpecValidationError> {
        let title = title.into();
        let status = status.into();
        let version = version.into();
        if title.trim().is_empty() {
            return Err(SpecValidationError::EmptyTitle);
        }
        if status.trim().is_empty() {
            return Err(SpecValidationError::EmptyStatus);
        }
        if version.trim().is_empty() {
            return Err(SpecValidationError::EmptyVersion);
        }
        Ok(Self {
            title,
            status,
            version,
            goal,
            scope,
            constraints,
            domain_states,
            acceptance_criteria,
            additional_sections,
            related_conventions,
            signals,
        })
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn status(&self) -> &str {
        &self.status
    }

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn goal(&self) -> &[String] {
        &self.goal
    }

    #[must_use]
    pub fn scope(&self) -> &SpecScope {
        &self.scope
    }

    #[must_use]
    pub fn constraints(&self) -> &[SpecRequirement] {
        &self.constraints
    }

    #[must_use]
    pub fn domain_states(&self) -> &[DomainStateEntry] {
        &self.domain_states
    }

    #[must_use]
    pub fn acceptance_criteria(&self) -> &[SpecRequirement] {
        &self.acceptance_criteria
    }

    #[must_use]
    pub fn additional_sections(&self) -> &[SpecSection] {
        &self.additional_sections
    }

    #[must_use]
    pub fn related_conventions(&self) -> &[String] {
        &self.related_conventions
    }

    #[must_use]
    pub fn signals(&self) -> Option<&SignalCounts> {
        self.signals.as_ref()
    }

    /// Updates the cached signal counts.
    pub fn set_signals(&mut self, signals: SignalCounts) {
        self.signals = Some(signals);
    }

    /// Evaluates signal counts from all evaluable requirements.
    ///
    /// Evaluable sections: scope (in + out), constraints, acceptance criteria.
    #[must_use]
    pub fn evaluate_signals(&self) -> SignalCounts {
        let mut blue: u32 = 0;
        let mut yellow: u32 = 0;
        let mut red: u32 = 0;

        let all_requirements = self
            .scope
            .in_scope
            .iter()
            .chain(self.scope.out_of_scope.iter())
            .chain(self.constraints.iter())
            .chain(self.acceptance_criteria.iter());

        for req in all_requirements {
            match req.signal() {
                ConfidenceSignal::Blue => blue += 1,
                ConfidenceSignal::Yellow => yellow += 1,
                ConfidenceSignal::Red => red += 1,
            }
        }

        SignalCounts::new(blue, yellow, red)
    }
}

// ---------------------------------------------------------------------------
// Multi-source signal evaluation
// ---------------------------------------------------------------------------

/// Evaluates the confidence signal for a requirement's sources.
///
/// Multi-source policy: the signal is the **highest** confidence among all sources
/// (`Blue > Yellow > Red`). Empty sources → `Red`.
#[must_use]
pub fn evaluate_requirement_signal(sources: &[String]) -> ConfidenceSignal {
    if sources.is_empty() {
        return ConfidenceSignal::Red;
    }

    sources
        .iter()
        .map(|tag| {
            classify_source_tag(tag).map(|basis| basis.signal()).unwrap_or(ConfidenceSignal::Red)
        })
        .max()
        .unwrap_or(ConfidenceSignal::Red)
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Validation errors for spec document construction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpecValidationError {
    #[error("spec title must not be empty")]
    EmptyTitle,
    #[error("spec status must not be empty")]
    EmptyStatus,
    #[error("spec version must not be empty")]
    EmptyVersion,
    #[error("requirement text must not be empty")]
    EmptyRequirementText,
    #[error("domain state name must not be empty")]
    EmptyDomainStateName,
    #[error("section title must not be empty")]
    EmptySectionTitle,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    // --- SpecRequirement ---

    #[test]
    fn test_requirement_with_valid_text_succeeds() {
        let req = SpecRequirement::new("Enable feature X", vec!["PRD §3.2".into()]).unwrap();
        assert_eq!(req.text(), "Enable feature X");
        assert_eq!(req.sources(), &["PRD §3.2"]);
    }

    #[test]
    fn test_requirement_with_empty_text_returns_error() {
        let result = SpecRequirement::new("", vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyRequirementText)));
    }

    #[test]
    fn test_requirement_with_whitespace_text_returns_error() {
        let result = SpecRequirement::new("   ", vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyRequirementText)));
    }

    // --- SpecRequirement signal evaluation ---

    #[test]
    fn test_requirement_signal_with_document_source_is_blue() {
        let req = SpecRequirement::new("req", vec!["PRD §3.2".into()]).unwrap();
        assert_eq!(req.signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_requirement_signal_with_empty_sources_is_red() {
        let req = SpecRequirement::new("req", vec![]).unwrap();
        assert_eq!(req.signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_requirement_signal_multi_source_takes_highest() {
        let req = SpecRequirement::new("req", vec!["inference — guess".into(), "PRD §3.2".into()])
            .unwrap();
        assert_eq!(req.signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_requirement_signal_multi_source_all_yellow() {
        let req =
            SpecRequirement::new("req", vec!["discussion".into(), "inference — guess".into()])
                .unwrap();
        assert_eq!(req.signal(), ConfidenceSignal::Yellow);
    }

    // --- DomainStateEntry ---

    #[test]
    fn test_domain_state_with_valid_name_succeeds() {
        let state = DomainStateEntry::new("Draft", "Initial state").unwrap();
        assert_eq!(state.name(), "Draft");
        assert_eq!(state.description(), "Initial state");
    }

    #[test]
    fn test_domain_state_with_empty_name_returns_error() {
        let result = DomainStateEntry::new("", "desc");
        assert!(matches!(result, Err(SpecValidationError::EmptyDomainStateName)));
    }

    // --- SpecSection ---

    #[test]
    fn test_section_with_valid_title_succeeds() {
        let section = SpecSection::new("Custom", vec!["line 1".into()]).unwrap();
        assert_eq!(section.title(), "Custom");
        assert_eq!(section.content(), &["line 1"]);
    }

    #[test]
    fn test_section_with_empty_title_returns_error() {
        let result = SpecSection::new("", vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptySectionTitle)));
    }

    // --- SpecDocument ---

    fn make_doc() -> SpecDocument {
        SpecDocument::new(
            "Feature X",
            "draft",
            "1.0",
            vec!["Goal line".into()],
            SpecScope::new(
                vec![SpecRequirement::new("in scope", vec!["PRD §1".into()]).unwrap()],
                vec![
                    SpecRequirement::new("out scope", vec!["inference — excluded".into()]).unwrap(),
                ],
            ),
            vec![SpecRequirement::new("constraint", vec!["convention — hex.md".into()]).unwrap()],
            vec![DomainStateEntry::new("Draft", "Initial").unwrap()],
            vec![SpecRequirement::new("AC 1", vec![]).unwrap()],
            vec![],
            vec!["project-docs/conventions/hex.md".into()],
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_document_creation_succeeds() {
        let doc = make_doc();
        assert_eq!(doc.title(), "Feature X");
        assert_eq!(doc.status(), "draft");
        assert_eq!(doc.version(), "1.0");
        assert_eq!(doc.domain_states().len(), 1);
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_document_with_empty_title_returns_error() {
        let result = SpecDocument::new(
            "",
            "draft",
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        );
        assert!(matches!(result, Err(SpecValidationError::EmptyTitle)));
    }

    #[test]
    fn test_document_evaluate_signals() {
        let doc = make_doc();
        let signals = doc.evaluate_signals();
        // in_scope: Blue (PRD), out_scope: Yellow (inference),
        // constraint: Blue (convention), AC1: Red (no sources)
        assert_eq!(signals.blue(), 2);
        assert_eq!(signals.yellow(), 1);
        assert_eq!(signals.red(), 1);
        assert_eq!(signals.total(), 4);
    }

    #[test]
    fn test_document_set_signals() {
        let mut doc = make_doc();
        doc.set_signals(SignalCounts::new(10, 2, 0));
        assert_eq!(doc.signals(), Some(&SignalCounts::new(10, 2, 0)));
    }

    // --- evaluate_requirement_signal ---

    #[test]
    fn test_evaluate_requirement_signal_empty_is_red() {
        assert_eq!(evaluate_requirement_signal(&[]), ConfidenceSignal::Red);
    }

    #[test]
    fn test_evaluate_requirement_signal_single_document_is_blue() {
        assert_eq!(evaluate_requirement_signal(&["PRD §3.2".into()]), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_requirement_signal_mixed_takes_highest() {
        assert_eq!(
            evaluate_requirement_signal(&["discussion".into(), "PRD §1".into()]),
            ConfidenceSignal::Blue
        );
    }

    #[test]
    fn test_evaluate_requirement_signal_all_inference_is_yellow() {
        assert_eq!(
            evaluate_requirement_signal(&["inference — a".into(), "inference — b".into()]),
            ConfidenceSignal::Yellow
        );
    }
}
