//! Domain types for structured spec documents (spec.json SSoT).
//!
//! `SpecDocument` is the aggregate root for a feature specification.
//! `spec.json` is the SSoT; `spec.md` is a read-only rendered view.

use std::collections::{HashMap, HashSet};

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
///
/// `transitions_to` semantics:
/// - `None`: undeclared (maximum Yellow signal)
/// - `Some(vec![])`: terminal state (Blue if type exists)
/// - `Some(vec!["StateB", ...])`: declared transitions to verify
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainStateEntry {
    name: String,
    description: String,
    transitions_to: Option<Vec<String>>,
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
        transitions_to: Option<Vec<String>>,
    ) -> Result<Self, SpecValidationError> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err(SpecValidationError::EmptyDomainStateName);
        }
        Ok(Self { name, description: description.into(), transitions_to })
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

    /// Returns the declared transitions, if any.
    ///
    /// - `None`: transitions not declared (undeclared)
    /// - `Some(&[])`: terminal state (no outgoing transitions)
    /// - `Some(&["B", "C"])`: transitions to states B and C
    #[must_use]
    pub fn transitions_to(&self) -> Option<&[String]> {
        self.transitions_to.as_deref()
    }
}

/// Per-state signal evaluation result for a domain state entry.
///
/// Produced by evaluating a `DomainStateEntry` against a `CodeScanResult`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DomainStateSignal {
    state_name: String,
    signal: ConfidenceSignal,
    found_type: bool,
    found_transitions: Vec<String>,
    missing_transitions: Vec<String>,
}

impl DomainStateSignal {
    /// Creates a new domain state signal result.
    #[must_use]
    pub fn new(
        state_name: impl Into<String>,
        signal: ConfidenceSignal,
        found_type: bool,
        found_transitions: Vec<String>,
        missing_transitions: Vec<String>,
    ) -> Self {
        Self {
            state_name: state_name.into(),
            signal,
            found_type,
            found_transitions,
            missing_transitions,
        }
    }

    /// Returns the state name.
    #[must_use]
    pub fn state_name(&self) -> &str {
        &self.state_name
    }

    /// Returns the evaluated signal.
    #[must_use]
    pub fn signal(&self) -> ConfidenceSignal {
        self.signal
    }

    /// Returns whether the type was found in domain code.
    #[must_use]
    pub fn found_type(&self) -> bool {
        self.found_type
    }

    /// Returns the transition target states that were found in code.
    #[must_use]
    pub fn found_transitions(&self) -> &[String] {
        &self.found_transitions
    }

    /// Returns the transition target states that were NOT found in code.
    #[must_use]
    pub fn missing_transitions(&self) -> &[String] {
        &self.missing_transitions
    }
}

/// Result of scanning domain code for type names and transition functions.
///
/// Produced by the infrastructure-layer syn AST scanner, consumed by
/// domain-layer evaluation logic.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeScanResult {
    found_types: HashSet<String>,
    transition_map: HashMap<String, HashSet<String>>,
}

impl CodeScanResult {
    /// Creates a new code scan result.
    #[must_use]
    pub fn new(
        found_types: HashSet<String>,
        transition_map: HashMap<String, HashSet<String>>,
    ) -> Self {
        Self { found_types, transition_map }
    }

    /// Returns the set of type names found in domain code.
    #[must_use]
    pub fn found_types(&self) -> &HashSet<String> {
        &self.found_types
    }

    /// Returns the transition map: from_state → set of to_states.
    #[must_use]
    pub fn transition_map(&self) -> &HashMap<String, HashSet<String>> {
        &self.transition_map
    }

    /// Returns whether a type name was found.
    #[must_use]
    pub fn has_type(&self, name: &str) -> bool {
        self.found_types.contains(name)
    }

    /// Returns the set of transition targets from a given state, if any.
    #[must_use]
    pub fn transitions_from(&self, state: &str) -> Option<&HashSet<String>> {
        self.transition_map.get(state)
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
    domain_state_signals: Option<Vec<DomainStateSignal>>,
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
        domain_state_signals: Option<Vec<DomainStateSignal>>,
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
            domain_state_signals,
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

    /// Updates the cached signal counts (Stage 1).
    pub fn set_signals(&mut self, signals: SignalCounts) {
        self.signals = Some(signals);
    }

    /// Returns the cached domain state signals (Stage 2), if evaluated.
    #[must_use]
    pub fn domain_state_signals(&self) -> Option<&[DomainStateSignal]> {
        self.domain_state_signals.as_deref()
    }

    /// Updates the cached domain state signals (Stage 2).
    pub fn set_domain_state_signals(&mut self, signals: Vec<DomainStateSignal>) {
        self.domain_state_signals = Some(signals);
    }

    /// Computes signal counts from the cached domain state signals.
    ///
    /// Returns `None` if domain state signals have not been evaluated yet.
    #[must_use]
    pub fn domain_state_signal_counts(&self) -> Option<SignalCounts> {
        let signals = self.domain_state_signals.as_ref()?;
        let mut blue: u32 = 0;
        let mut yellow: u32 = 0;
        let mut red: u32 = 0;
        for s in signals {
            match s.signal {
                ConfidenceSignal::Blue => blue += 1,
                ConfidenceSignal::Yellow => yellow += 1,
                ConfidenceSignal::Red => red += 1,
            }
        }
        Some(SignalCounts::new(blue, yellow, red))
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
// Domain state signal evaluation
// ---------------------------------------------------------------------------

/// Evaluates domain state signals by comparing spec entries against code scan results.
///
/// Signal criteria:
/// - Blue: type exists AND (terminal state OR all declared transitions found)
/// - Yellow: type exists but transitions not found, or transitions_to undeclared
/// - Red: type not found in domain code
#[must_use]
pub fn evaluate_domain_state_signals(
    entries: &[DomainStateEntry],
    scan: &CodeScanResult,
) -> Vec<DomainStateSignal> {
    entries.iter().map(|entry| evaluate_single_state(entry, scan)).collect()
}

fn evaluate_single_state(entry: &DomainStateEntry, scan: &CodeScanResult) -> DomainStateSignal {
    let name = entry.name();

    // Red: type not found in domain code
    if !scan.has_type(name) {
        return DomainStateSignal::new(name, ConfidenceSignal::Red, false, vec![], vec![]);
    }

    // Type exists — determine signal from transitions_to
    match entry.transitions_to() {
        // Yellow: transitions undeclared
        None => DomainStateSignal::new(name, ConfidenceSignal::Yellow, true, vec![], vec![]),

        // Blue: terminal state (no outgoing transitions declared)
        Some([]) => DomainStateSignal::new(name, ConfidenceSignal::Blue, true, vec![], vec![]),

        // Check each declared transition target against scan
        Some(targets) => {
            let found_in_scan = scan.transitions_from(name);
            let mut found_transitions: Vec<String> = Vec::new();
            let mut missing_transitions: Vec<String> = Vec::new();

            for target in targets {
                let is_found =
                    found_in_scan.map(|set| set.contains(target.as_str())).unwrap_or(false);
                if is_found {
                    found_transitions.push(target.clone());
                } else {
                    missing_transitions.push(target.clone());
                }
            }

            let signal = if missing_transitions.is_empty() {
                ConfidenceSignal::Blue
            } else {
                ConfidenceSignal::Yellow
            };

            DomainStateSignal::new(name, signal, true, found_transitions, missing_transitions)
        }
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
#[allow(clippy::unwrap_used, clippy::indexing_slicing)]
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
        let state = DomainStateEntry::new("Draft", "Initial state", None).unwrap();
        assert_eq!(state.name(), "Draft");
        assert_eq!(state.description(), "Initial state");
        assert_eq!(state.transitions_to(), None);
    }

    #[test]
    fn test_domain_state_with_empty_name_returns_error() {
        let result = DomainStateEntry::new("", "desc", None);
        assert!(matches!(result, Err(SpecValidationError::EmptyDomainStateName)));
    }

    #[test]
    fn test_domain_state_with_terminal_transitions() {
        let state = DomainStateEntry::new("Final", "Terminal state", Some(vec![])).unwrap();
        assert_eq!(state.transitions_to(), Some([].as_slice()));
    }

    #[test]
    fn test_domain_state_with_declared_transitions() {
        let state = DomainStateEntry::new(
            "Draft",
            "Initial",
            Some(vec!["Published".into(), "Archived".into()]),
        )
        .unwrap();
        assert_eq!(
            state.transitions_to(),
            Some(["Published".to_string(), "Archived".to_string()].as_slice())
        );
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
            vec![DomainStateEntry::new("Draft", "Initial", None).unwrap()],
            vec![SpecRequirement::new("AC 1", vec![]).unwrap()],
            vec![],
            vec!["project-docs/conventions/hex.md".into()],
            None,
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

    // --- DomainStateSignal ---

    #[test]
    fn test_domain_state_signal_accessors() {
        let sig = DomainStateSignal::new(
            "Draft",
            ConfidenceSignal::Blue,
            true,
            vec!["Published".into()],
            vec![],
        );
        assert_eq!(sig.state_name(), "Draft");
        assert_eq!(sig.signal(), ConfidenceSignal::Blue);
        assert!(sig.found_type());
        assert_eq!(sig.found_transitions(), &["Published"]);
        assert!(sig.missing_transitions().is_empty());
    }

    #[test]
    fn test_domain_state_signal_red_missing_type() {
        let sig = DomainStateSignal::new("Ghost", ConfidenceSignal::Red, false, vec![], vec![]);
        assert!(!sig.found_type());
        assert_eq!(sig.signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_domain_state_signal_yellow_missing_transitions() {
        let sig = DomainStateSignal::new(
            "Draft",
            ConfidenceSignal::Yellow,
            true,
            vec![],
            vec!["Published".into()],
        );
        assert_eq!(sig.signal(), ConfidenceSignal::Yellow);
        assert_eq!(sig.missing_transitions(), &["Published"]);
    }

    // --- CodeScanResult ---

    #[test]
    fn test_code_scan_result_has_type() {
        let types: HashSet<String> = ["Draft".into(), "Published".into()].into_iter().collect();
        let scan = CodeScanResult::new(types, HashMap::new());
        assert!(scan.has_type("Draft"));
        assert!(scan.has_type("Published"));
        assert!(!scan.has_type("Archived"));
    }

    #[test]
    fn test_code_scan_result_transitions_from() {
        let types: HashSet<String> = ["Draft".into()].into_iter().collect();
        let mut transitions = HashMap::new();
        transitions.insert("Draft".into(), ["Published".into()].into_iter().collect());
        let scan = CodeScanResult::new(types, transitions);
        let targets = scan.transitions_from("Draft").unwrap();
        assert!(targets.contains("Published"));
        assert!(scan.transitions_from("Unknown").is_none());
    }

    // --- SpecDocument domain_state_signals ---

    #[test]
    fn test_document_domain_state_signals_initially_none() {
        let doc = make_doc();
        assert!(doc.domain_state_signals().is_none());
        assert!(doc.domain_state_signal_counts().is_none());
    }

    #[test]
    fn test_document_set_domain_state_signals() {
        let mut doc = make_doc();
        let signals = vec![
            DomainStateSignal::new("Draft", ConfidenceSignal::Blue, true, vec![], vec![]),
            DomainStateSignal::new("Ghost", ConfidenceSignal::Red, false, vec![], vec![]),
        ];
        doc.set_domain_state_signals(signals);
        assert_eq!(doc.domain_state_signals().unwrap().len(), 2);
        let counts = doc.domain_state_signal_counts().unwrap();
        assert_eq!(counts.blue(), 1);
        assert_eq!(counts.red(), 1);
        assert_eq!(counts.yellow(), 0);
    }

    // --- evaluate_domain_state_signals ---

    fn make_scan_with_type(type_name: &str) -> CodeScanResult {
        let types: HashSet<String> = [type_name.to_string()].into_iter().collect();
        CodeScanResult::new(types, HashMap::new())
    }

    fn make_scan_with_type_and_transitions(
        type_name: &str,
        transitions: &[(&str, &[&str])],
    ) -> CodeScanResult {
        let types: HashSet<String> = [type_name.to_string()].into_iter().collect();
        let mut transition_map: HashMap<String, HashSet<String>> = HashMap::new();
        for (from, tos) in transitions {
            let to_set: HashSet<String> = tos.iter().map(|s| s.to_string()).collect();
            transition_map.insert(from.to_string(), to_set);
        }
        CodeScanResult::new(types, transition_map)
    }

    #[test]
    fn test_evaluate_red_when_type_not_found() {
        let entry = DomainStateEntry::new("Ghost", "A missing state", None).unwrap();
        let scan = CodeScanResult::new(HashSet::new(), HashMap::new());
        let results = evaluate_domain_state_signals(&[entry], &scan);
        assert_eq!(results.len(), 1);
        let sig = &results[0];
        assert_eq!(sig.state_name(), "Ghost");
        assert_eq!(sig.signal(), ConfidenceSignal::Red);
        assert!(!sig.found_type());
        assert!(sig.found_transitions().is_empty());
        assert!(sig.missing_transitions().is_empty());
    }

    #[test]
    fn test_evaluate_blue_for_terminal_state() {
        let entry = DomainStateEntry::new("Final", "Terminal state", Some(vec![])).unwrap();
        let scan = make_scan_with_type("Final");
        let results = evaluate_domain_state_signals(&[entry], &scan);
        assert_eq!(results.len(), 1);
        let sig = &results[0];
        assert_eq!(sig.state_name(), "Final");
        assert_eq!(sig.signal(), ConfidenceSignal::Blue);
        assert!(sig.found_type());
        assert!(sig.found_transitions().is_empty());
        assert!(sig.missing_transitions().is_empty());
    }

    #[test]
    fn test_evaluate_yellow_when_transitions_undeclared() {
        let entry = DomainStateEntry::new("Draft", "Initial state", None).unwrap();
        let scan = make_scan_with_type("Draft");
        let results = evaluate_domain_state_signals(&[entry], &scan);
        assert_eq!(results.len(), 1);
        let sig = &results[0];
        assert_eq!(sig.state_name(), "Draft");
        assert_eq!(sig.signal(), ConfidenceSignal::Yellow);
        assert!(sig.found_type());
        assert!(sig.found_transitions().is_empty());
        assert!(sig.missing_transitions().is_empty());
    }

    #[test]
    fn test_evaluate_blue_when_all_transitions_found() {
        let entry = DomainStateEntry::new("Draft", "Initial state", Some(vec!["Published".into()]))
            .unwrap();
        let scan = make_scan_with_type_and_transitions("Draft", &[("Draft", &["Published"])]);
        let results = evaluate_domain_state_signals(&[entry], &scan);
        assert_eq!(results.len(), 1);
        let sig = &results[0];
        assert_eq!(sig.state_name(), "Draft");
        assert_eq!(sig.signal(), ConfidenceSignal::Blue);
        assert!(sig.found_type());
        assert_eq!(sig.found_transitions(), &["Published"]);
        assert!(sig.missing_transitions().is_empty());
    }

    #[test]
    fn test_evaluate_yellow_when_some_transitions_missing() {
        let entry = DomainStateEntry::new(
            "Draft",
            "Initial state",
            Some(vec!["Published".into(), "Archived".into()]),
        )
        .unwrap();
        let scan = make_scan_with_type_and_transitions("Draft", &[("Draft", &["Published"])]);
        let results = evaluate_domain_state_signals(&[entry], &scan);
        assert_eq!(results.len(), 1);
        let sig = &results[0];
        assert_eq!(sig.state_name(), "Draft");
        assert_eq!(sig.signal(), ConfidenceSignal::Yellow);
        assert!(sig.found_type());
        assert_eq!(sig.found_transitions(), &["Published"]);
        assert_eq!(sig.missing_transitions(), &["Archived"]);
    }

    #[test]
    fn test_evaluate_yellow_when_all_transitions_missing() {
        let entry = DomainStateEntry::new("Draft", "Initial state", Some(vec!["Published".into()]))
            .unwrap();
        // Type exists but no transitions in scan
        let scan = make_scan_with_type("Draft");
        let results = evaluate_domain_state_signals(&[entry], &scan);
        assert_eq!(results.len(), 1);
        let sig = &results[0];
        assert_eq!(sig.state_name(), "Draft");
        assert_eq!(sig.signal(), ConfidenceSignal::Yellow);
        assert!(sig.found_type());
        assert!(sig.found_transitions().is_empty());
        assert_eq!(sig.missing_transitions(), &["Published"]);
    }

    #[test]
    fn test_evaluate_multiple_entries() {
        let entries = vec![
            // Red: type not found
            DomainStateEntry::new("Ghost", "Missing", None).unwrap(),
            // Blue: terminal
            DomainStateEntry::new("Final", "Terminal", Some(vec![])).unwrap(),
            // Yellow: transitions undeclared
            DomainStateEntry::new("Draft", "Initial", None).unwrap(),
        ];
        let mut types: HashSet<String> = HashSet::new();
        types.insert("Final".to_string());
        types.insert("Draft".to_string());
        let scan = CodeScanResult::new(types, HashMap::new());
        let results = evaluate_domain_state_signals(&entries, &scan);
        assert_eq!(results.len(), 3);
        assert_eq!(results[0].signal(), ConfidenceSignal::Red);
        assert_eq!(results[1].signal(), ConfidenceSignal::Blue);
        assert_eq!(results[2].signal(), ConfidenceSignal::Yellow);
    }
}
