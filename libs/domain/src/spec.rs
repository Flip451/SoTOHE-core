//! Domain types for structured spec documents (spec.json SSoT).
//!
//! `SpecDocument` is the aggregate root for a feature specification.
//! `spec.json` is the SSoT; `spec.md` is a read-only rendered view.

use std::collections::{HashMap, HashSet};
use std::fmt;

use crate::{ConfidenceSignal, SignalCounts, TaskId, Timestamp, classify_source_tag};

// ---------------------------------------------------------------------------
// SpecStatus enum
// ---------------------------------------------------------------------------

/// Approval status of a specification document.
///
/// - `Draft`: specification is being authored or was auto-demoted after content change.
/// - `Approved`: explicitly approved; valid only while `content_hash` matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpecStatus {
    Draft,
    Approved,
}

impl SpecStatus {
    /// Returns the canonical string representation.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Approved => "approved",
        }
    }
}

impl fmt::Display for SpecStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

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
    task_refs: Vec<TaskId>,
}

impl SpecRequirement {
    /// Creates a new requirement with empty task_refs.
    ///
    /// # Errors
    ///
    /// Returns error if `text` is empty or whitespace-only.
    pub fn new(text: impl Into<String>, sources: Vec<String>) -> Result<Self, SpecValidationError> {
        Self::with_task_refs(text, sources, vec![])
    }

    /// Creates a new requirement with explicit task references.
    ///
    /// # Errors
    ///
    /// Returns error if `text` is empty or whitespace-only.
    pub fn with_task_refs(
        text: impl Into<String>,
        sources: Vec<String>,
        task_refs: Vec<TaskId>,
    ) -> Result<Self, SpecValidationError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(SpecValidationError::EmptyRequirementText);
        }
        Ok(Self { text, sources, task_refs })
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

    /// Returns the task references linking this requirement to plan tasks.
    #[must_use]
    pub fn task_refs(&self) -> &[TaskId] {
        &self.task_refs
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
    status: SpecStatus,
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
    approved_at: Option<Timestamp>,
    content_hash: Option<String>,
    hearing_history: Vec<HearingRecord>,
}

impl SpecDocument {
    /// Creates a new spec document.
    ///
    /// # Errors
    ///
    /// Returns error if `title` or `version` is empty.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: impl Into<String>,
        status: SpecStatus,
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
        approved_at: Option<Timestamp>,
        content_hash: Option<String>,
    ) -> Result<Self, SpecValidationError> {
        let title = title.into();
        let version = version.into();
        if title.trim().is_empty() {
            return Err(SpecValidationError::EmptyTitle);
        }
        if version.trim().is_empty() {
            return Err(SpecValidationError::EmptyVersion);
        }
        // Enforce status/metadata invariant:
        // - Draft must NOT carry approval metadata
        // - Approved must have both approved_at and content_hash
        let (approved_at, content_hash) = match status {
            SpecStatus::Draft => (None, None),
            SpecStatus::Approved => {
                let hash = content_hash.as_deref().unwrap_or("");
                if approved_at.is_none() || hash.trim().is_empty() {
                    return Err(SpecValidationError::ApprovalMetadataMissing);
                }
                (approved_at, content_hash)
            }
        };
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
            approved_at,
            content_hash,
            hearing_history: vec![],
        })
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
    }

    #[must_use]
    pub fn status(&self) -> SpecStatus {
        self.status
    }

    /// Returns the approval timestamp, if approved.
    #[must_use]
    pub fn approved_at(&self) -> Option<&Timestamp> {
        self.approved_at.as_ref()
    }

    /// Returns the content hash recorded at approval time.
    #[must_use]
    pub fn content_hash(&self) -> Option<&str> {
        self.content_hash.as_deref()
    }

    /// Returns the hearing history (append-only audit trail).
    #[must_use]
    pub fn hearing_history(&self) -> &[HearingRecord] {
        &self.hearing_history
    }

    /// Appends a hearing record to the history.
    pub fn append_hearing_record(&mut self, record: HearingRecord) {
        self.hearing_history.push(record);
    }

    /// Sets hearing history (used by codec decode).
    pub fn set_hearing_history(&mut self, history: Vec<HearingRecord>) {
        self.hearing_history = history;
    }

    /// Marks this spec as approved with the given timestamp and content hash.
    ///
    /// # Errors
    ///
    /// Returns `SpecValidationError::ApprovalMetadataMissing` if `content_hash` is empty.
    pub fn approve(
        &mut self,
        timestamp: Timestamp,
        content_hash: String,
    ) -> Result<(), SpecValidationError> {
        if content_hash.trim().is_empty() {
            return Err(SpecValidationError::ApprovalMetadataMissing);
        }
        self.status = SpecStatus::Approved;
        self.approved_at = Some(timestamp);
        self.content_hash = Some(content_hash);
        Ok(())
    }

    /// Demotes this spec to draft, clearing approval metadata.
    pub fn demote(&mut self) {
        self.status = SpecStatus::Draft;
        self.approved_at = None;
        self.content_hash = None;
    }

    /// Checks whether the current approval is still valid given the current content hash.
    ///
    /// Returns `true` only if status is `Approved` AND the stored content hash
    /// matches `current_hash`.
    #[must_use]
    pub fn is_approval_valid(&self, current_hash: &str) -> bool {
        self.status == SpecStatus::Approved && self.content_hash.as_deref() == Some(current_hash)
    }

    /// Returns the effective status considering content hash integrity.
    ///
    /// If status is `Approved` but the content hash does not match, returns `Draft`.
    #[must_use]
    pub fn effective_status(&self, current_hash: &str) -> SpecStatus {
        if self.is_approval_valid(current_hash) { SpecStatus::Approved } else { SpecStatus::Draft }
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

    /// Evaluates requirement-to-task coverage for CI gate enforcement.
    ///
    /// Checks in_scope and acceptance_criteria requirements (enforced sections).
    /// Constraints are NOT enforced — their coverage is not checked here.
    ///
    /// Validates that:
    /// 1. Every enforced requirement has at least one task_ref.
    /// 2. Every task_ref references a valid task ID from the provided set.
    #[must_use]
    pub fn evaluate_coverage(&self, valid_task_ids: &HashSet<TaskId>) -> CoverageResult {
        let mut covered: u32 = 0;
        let mut uncovered: Vec<String> = Vec::new();
        let mut invalid_refs: Vec<String> = Vec::new();
        let mut seen_invalid: HashSet<String> = HashSet::new();

        let enforced = self.scope.in_scope.iter().chain(self.acceptance_criteria.iter());

        for req in enforced {
            let mut has_valid_ref = false;

            for task_ref in &req.task_refs {
                if valid_task_ids.contains(task_ref) {
                    has_valid_ref = true;
                } else {
                    let ref_str = task_ref.to_string();
                    if seen_invalid.insert(ref_str.clone()) {
                        invalid_refs.push(ref_str);
                    }
                }
            }

            if has_valid_ref {
                covered += 1;
            } else {
                uncovered.push(req.text.clone());
            }
        }

        CoverageResult::new(covered, uncovered, invalid_refs)
    }

    /// Validates referential integrity of task_refs across ALL sections.
    ///
    /// Returns task_ref IDs that do not exist in the provided task set.
    /// Unlike `evaluate_coverage()` which only checks enforced sections,
    /// this checks constraints and out_of_scope as well.
    #[must_use]
    pub fn validate_all_task_refs(&self, valid_task_ids: &HashSet<TaskId>) -> Vec<String> {
        let mut invalid: Vec<String> = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();

        let all_requirements = self
            .scope
            .in_scope
            .iter()
            .chain(self.scope.out_of_scope.iter())
            .chain(self.constraints.iter())
            .chain(self.acceptance_criteria.iter());

        for req in all_requirements {
            for task_ref in &req.task_refs {
                if !valid_task_ids.contains(task_ref) {
                    let ref_str = task_ref.to_string();
                    if seen.insert(ref_str.clone()) {
                        invalid.push(ref_str);
                    }
                }
            }
        }

        invalid
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
// Coverage evaluation
// ---------------------------------------------------------------------------

/// Result of evaluating requirement-to-task coverage.
///
/// Produced by `SpecDocument::evaluate_coverage()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CoverageResult {
    covered: u32,
    uncovered: Vec<String>,
    invalid_refs: Vec<String>,
}

impl CoverageResult {
    /// Creates a new coverage result.
    #[must_use]
    pub fn new(covered: u32, uncovered: Vec<String>, invalid_refs: Vec<String>) -> Self {
        Self { covered, uncovered, invalid_refs }
    }

    /// Returns the count of requirements that have at least one task_ref.
    #[must_use]
    pub fn covered(&self) -> u32 {
        self.covered
    }

    /// Returns the texts of requirements missing task_refs (in_scope + acceptance_criteria only).
    #[must_use]
    pub fn uncovered(&self) -> &[String] {
        &self.uncovered
    }

    /// Returns task_ref IDs that do not exist in the provided task set.
    #[must_use]
    pub fn invalid_refs(&self) -> &[String] {
        &self.invalid_refs
    }

    /// Returns `true` if all evaluable requirements are covered and no invalid refs exist.
    #[must_use]
    pub fn is_fully_covered(&self) -> bool {
        self.uncovered.is_empty() && self.invalid_refs.is_empty()
    }

    /// Returns the total number of evaluable requirements.
    #[must_use]
    pub fn total(&self) -> u32 {
        self.covered + self.uncovered.len() as u32
    }
}

// ---------------------------------------------------------------------------
// Hearing record types (TSUMIKI-07)
// ---------------------------------------------------------------------------

/// Workload mode for a hearing session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum HearingMode {
    Full,
    Focused,
    Quick,
}

impl HearingMode {
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Full => "full",
            Self::Focused => "focused",
            Self::Quick => "quick",
        }
    }
}

impl fmt::Display for HearingMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Signal counts snapshot for before/after comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HearingSignalSnapshot {
    blue: u32,
    yellow: u32,
    red: u32,
}

impl HearingSignalSnapshot {
    #[must_use]
    pub fn new(blue: u32, yellow: u32, red: u32) -> Self {
        Self { blue, yellow, red }
    }

    #[must_use]
    pub fn blue(&self) -> u32 {
        self.blue
    }
    #[must_use]
    pub fn yellow(&self) -> u32 {
        self.yellow
    }
    #[must_use]
    pub fn red(&self) -> u32 {
        self.red
    }
}

impl From<SignalCounts> for HearingSignalSnapshot {
    fn from(c: SignalCounts) -> Self {
        Self::new(c.blue(), c.yellow(), c.red())
    }
}

/// Before/after signal delta for a hearing session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HearingSignalDelta {
    before: HearingSignalSnapshot,
    after: HearingSignalSnapshot,
}

impl HearingSignalDelta {
    #[must_use]
    pub fn new(before: HearingSignalSnapshot, after: HearingSignalSnapshot) -> Self {
        Self { before, after }
    }

    #[must_use]
    pub fn before(&self) -> &HearingSignalSnapshot {
        &self.before
    }
    #[must_use]
    pub fn after(&self) -> &HearingSignalSnapshot {
        &self.after
    }
}

/// A single hearing session record (append-only audit trail).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HearingRecord {
    date: Timestamp,
    mode: HearingMode,
    signal_delta: HearingSignalDelta,
    questions_asked: u32,
    items_added: u32,
    items_modified: u32,
}

impl HearingRecord {
    #[must_use]
    pub fn new(
        date: Timestamp,
        mode: HearingMode,
        signal_delta: HearingSignalDelta,
        questions_asked: u32,
        items_added: u32,
        items_modified: u32,
    ) -> Self {
        Self { date, mode, signal_delta, questions_asked, items_added, items_modified }
    }

    #[must_use]
    pub fn date(&self) -> &Timestamp {
        &self.date
    }
    #[must_use]
    pub fn mode(&self) -> HearingMode {
        self.mode
    }
    #[must_use]
    pub fn signal_delta(&self) -> &HearingSignalDelta {
        &self.signal_delta
    }
    #[must_use]
    pub fn questions_asked(&self) -> u32 {
        self.questions_asked
    }
    #[must_use]
    pub fn items_added(&self) -> u32 {
        self.items_added
    }
    #[must_use]
    pub fn items_modified(&self) -> u32 {
        self.items_modified
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Validation errors for spec document construction.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SpecValidationError {
    #[error("spec title must not be empty")]
    EmptyTitle,
    #[error("spec version must not be empty")]
    EmptyVersion,
    #[error("approved spec must have both approved_at and content_hash")]
    ApprovalMetadataMissing,
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

    // --- SpecRequirement task_refs ---

    #[test]
    fn test_requirement_new_has_empty_task_refs_by_default() {
        let req = SpecRequirement::new("Enable feature X", vec!["PRD §3.2".into()]).unwrap();
        assert!(req.task_refs().is_empty());
    }

    #[test]
    fn test_requirement_with_task_refs_stores_refs() {
        let req = SpecRequirement::with_task_refs(
            "Enable feature X",
            vec!["PRD §3.2".into()],
            vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T002").unwrap()],
        )
        .unwrap();
        assert_eq!(req.task_refs().len(), 2);
        assert_eq!(req.task_refs()[0].as_ref(), "T001");
        assert_eq!(req.task_refs()[1].as_ref(), "T002");
    }

    #[test]
    fn test_requirement_with_task_refs_empty_text_returns_error() {
        let result = SpecRequirement::with_task_refs("", vec![], vec![]);
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
            SpecStatus::Draft,
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
            vec!["knowledge/conventions/hex.md".into()],
            None,
            None,
            None,
            None,
        )
        .unwrap()
    }

    #[test]
    fn test_document_creation_succeeds() {
        let doc = make_doc();
        assert_eq!(doc.title(), "Feature X");
        assert_eq!(doc.status(), SpecStatus::Draft);
        assert_eq!(doc.version(), "1.0");
        assert_eq!(doc.domain_states().len(), 1);
        assert!(doc.signals().is_none());
        assert!(doc.approved_at().is_none());
        assert!(doc.content_hash().is_none());
    }

    // --- SpecStatus ---

    #[test]
    fn test_spec_status_display() {
        assert_eq!(SpecStatus::Draft.to_string(), "draft");
        assert_eq!(SpecStatus::Approved.to_string(), "approved");
    }

    #[test]
    fn test_spec_status_as_str() {
        assert_eq!(SpecStatus::Draft.as_str(), "draft");
        assert_eq!(SpecStatus::Approved.as_str(), "approved");
    }

    // --- SpecDocument approve / demote ---

    #[test]
    fn test_document_approve_sets_status_and_metadata() {
        let mut doc = make_doc();
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts.clone(), "sha256:abc123".into()).unwrap();

        assert_eq!(doc.status(), SpecStatus::Approved);
        assert_eq!(doc.approved_at().unwrap(), &ts);
        assert_eq!(doc.content_hash(), Some("sha256:abc123"));
    }

    #[test]
    fn test_document_demote_clears_approval() {
        let mut doc = make_doc();
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts, "sha256:abc123".into()).unwrap();
        doc.demote();

        assert_eq!(doc.status(), SpecStatus::Draft);
        assert!(doc.approved_at().is_none());
        assert!(doc.content_hash().is_none());
    }

    #[test]
    fn test_is_approval_valid_with_matching_hash() {
        let mut doc = make_doc();
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts, "sha256:abc123".into()).unwrap();

        assert!(doc.is_approval_valid("sha256:abc123"));
    }

    #[test]
    fn test_is_approval_valid_with_mismatched_hash() {
        let mut doc = make_doc();
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts, "sha256:abc123".into()).unwrap();

        assert!(!doc.is_approval_valid("sha256:different"));
    }

    #[test]
    fn test_is_approval_valid_when_draft() {
        let doc = make_doc();
        assert!(!doc.is_approval_valid("sha256:abc123"));
    }

    #[test]
    fn test_effective_status_approved_with_matching_hash() {
        let mut doc = make_doc();
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts, "sha256:abc123".into()).unwrap();

        assert_eq!(doc.effective_status("sha256:abc123"), SpecStatus::Approved);
    }

    #[test]
    fn test_effective_status_draft_with_mismatched_hash() {
        let mut doc = make_doc();
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        doc.approve(ts, "sha256:abc123".into()).unwrap();

        assert_eq!(doc.effective_status("sha256:different"), SpecStatus::Draft);
    }

    #[test]
    fn test_effective_status_draft_when_never_approved() {
        let doc = make_doc();
        assert_eq!(doc.effective_status("sha256:anything"), SpecStatus::Draft);
    }

    #[test]
    fn test_document_with_empty_title_returns_error() {
        let result = SpecDocument::new(
            "",
            SpecStatus::Draft,
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
            None,
            None,
        );
        assert!(matches!(result, Err(SpecValidationError::EmptyTitle)));
    }

    #[test]
    fn test_document_approved_without_metadata_returns_error() {
        let result = SpecDocument::new(
            "T",
            SpecStatus::Approved,
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
            None, // no approved_at
            None, // no content_hash
        );
        assert!(matches!(result, Err(SpecValidationError::ApprovalMetadataMissing)));
    }

    #[test]
    fn test_document_approved_with_only_timestamp_returns_error() {
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        let result = SpecDocument::new(
            "T",
            SpecStatus::Approved,
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
            Some(ts),
            None, // no content_hash
        );
        assert!(matches!(result, Err(SpecValidationError::ApprovalMetadataMissing)));
    }

    #[test]
    fn test_document_approved_with_only_hash_returns_error() {
        let result = SpecDocument::new(
            "T",
            SpecStatus::Approved,
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
            None, // no approved_at
            Some("sha256:abc".into()),
        );
        assert!(matches!(result, Err(SpecValidationError::ApprovalMetadataMissing)));
    }

    #[test]
    fn test_document_approved_with_empty_hash_returns_error() {
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        let result = SpecDocument::new(
            "T",
            SpecStatus::Approved,
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
            Some(ts),
            Some("".into()),
        );
        assert!(matches!(result, Err(SpecValidationError::ApprovalMetadataMissing)));
    }

    #[test]
    fn test_approve_rejects_empty_hash() {
        let mut doc = make_doc();
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        let result = doc.approve(ts, "".into());
        assert!(matches!(result, Err(SpecValidationError::ApprovalMetadataMissing)));
        assert_eq!(doc.status(), SpecStatus::Draft, "should remain Draft on error");
    }

    #[test]
    fn test_document_draft_with_metadata_strips_it() {
        let ts = Timestamp::new("2026-03-24T10:00:00Z").unwrap();
        let doc = SpecDocument::new(
            "T",
            SpecStatus::Draft,
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
            Some(ts),
            Some("sha256:abc".into()),
        )
        .unwrap();
        // Draft silently strips approval metadata
        assert!(doc.approved_at().is_none());
        assert!(doc.content_hash().is_none());
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

    // --- CoverageResult ---

    #[test]
    fn test_coverage_result_accessors() {
        let result = CoverageResult::new(3, vec!["uncov".into()], vec!["T999".into()]);
        assert_eq!(result.covered(), 3);
        assert_eq!(result.uncovered(), &["uncov"]);
        assert_eq!(result.invalid_refs(), &["T999"]);
        assert!(!result.is_fully_covered());
        assert_eq!(result.total(), 4);
    }

    #[test]
    fn test_coverage_result_fully_covered() {
        let result = CoverageResult::new(5, vec![], vec![]);
        assert!(result.is_fully_covered());
        assert_eq!(result.total(), 5);
    }

    // --- SpecDocument::evaluate_coverage ---

    fn make_task_id_set(ids: &[&str]) -> HashSet<TaskId> {
        ids.iter().map(|id| TaskId::try_new(*id).unwrap()).collect()
    }

    #[test]
    fn test_evaluate_coverage_all_covered() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(
                vec![
                    SpecRequirement::with_task_refs(
                        "in scope item",
                        vec!["PRD §1".into()],
                        vec![TaskId::try_new("T001").unwrap()],
                    )
                    .unwrap(),
                ],
                vec![],
            ),
            vec![],
            vec![],
            vec![
                SpecRequirement::with_task_refs(
                    "AC item",
                    vec!["discussion".into()],
                    vec![TaskId::try_new("T002").unwrap()],
                )
                .unwrap(),
            ],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001", "T002"]);
        let result = doc.evaluate_coverage(&valid);
        assert!(result.is_fully_covered());
        assert_eq!(result.covered(), 2);
        assert_eq!(result.total(), 2);
    }

    #[test]
    fn test_evaluate_coverage_missing_task_refs() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(
                vec![SpecRequirement::new("uncovered in scope", vec!["PRD §1".into()]).unwrap()],
                vec![],
            ),
            vec![],
            vec![],
            vec![SpecRequirement::new("uncovered AC", vec!["discussion".into()]).unwrap()],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001"]);
        let result = doc.evaluate_coverage(&valid);
        assert!(!result.is_fully_covered());
        assert_eq!(result.covered(), 0);
        assert_eq!(result.uncovered().len(), 2);
        assert!(result.uncovered().contains(&"uncovered in scope".to_string()));
        assert!(result.uncovered().contains(&"uncovered AC".to_string()));
    }

    #[test]
    fn test_evaluate_coverage_invalid_task_ref() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(
                vec![
                    SpecRequirement::with_task_refs(
                        "in scope",
                        vec!["PRD §1".into()],
                        vec![TaskId::try_new("T999").unwrap()],
                    )
                    .unwrap(),
                ],
                vec![],
            ),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001"]);
        let result = doc.evaluate_coverage(&valid);
        assert!(!result.is_fully_covered());
        assert_eq!(result.covered(), 0); // only invalid refs → uncovered
        assert_eq!(result.uncovered(), &["in scope"]);
        assert_eq!(result.invalid_refs(), &["T999"]);
    }

    #[test]
    fn test_evaluate_coverage_mixed_valid_and_invalid_refs() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(
                vec![
                    SpecRequirement::with_task_refs(
                        "in scope",
                        vec!["PRD §1".into()],
                        vec![TaskId::try_new("T001").unwrap(), TaskId::try_new("T999").unwrap()],
                    )
                    .unwrap(),
                ],
                vec![],
            ),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001"]);
        let result = doc.evaluate_coverage(&valid);
        // Has at least one valid ref → covered, but also has invalid ref
        assert!(!result.is_fully_covered());
        assert_eq!(result.covered(), 1);
        assert!(result.uncovered().is_empty());
        assert_eq!(result.invalid_refs(), &["T999"]);
    }

    #[test]
    fn test_evaluate_coverage_constraints_not_enforced() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(vec![], vec![]),
            // Constraint with NO task_refs — should NOT appear in uncovered
            vec![SpecRequirement::new("constraint", vec!["convention — hex.md".into()]).unwrap()],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&[]);
        let result = doc.evaluate_coverage(&valid);
        assert!(result.is_fully_covered());
        assert_eq!(result.covered(), 0);
        assert_eq!(result.total(), 0);
    }

    #[test]
    fn test_evaluate_coverage_deduplicates_invalid_refs() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(
                vec![
                    SpecRequirement::with_task_refs(
                        "item 1",
                        vec!["PRD §1".into()],
                        vec![TaskId::try_new("T999").unwrap()],
                    )
                    .unwrap(),
                    SpecRequirement::with_task_refs(
                        "item 2",
                        vec!["PRD §2".into()],
                        vec![TaskId::try_new("T999").unwrap()],
                    )
                    .unwrap(),
                ],
                vec![],
            ),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001"]);
        let result = doc.evaluate_coverage(&valid);
        // T999 appears twice in different requirements but should be deduplicated
        assert_eq!(result.invalid_refs().len(), 1);
        assert_eq!(result.invalid_refs()[0], "T999");
    }

    // --- validate_all_task_refs ---

    #[test]
    fn test_validate_all_task_refs_catches_constraint_invalid_ref() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(vec![], vec![]),
            vec![
                SpecRequirement::with_task_refs(
                    "constraint",
                    vec!["convention — hex.md".into()],
                    vec![TaskId::try_new("T999").unwrap()],
                )
                .unwrap(),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001"]);
        let invalid = doc.validate_all_task_refs(&valid);
        assert_eq!(invalid, &["T999"]);
    }

    #[test]
    fn test_validate_all_task_refs_catches_out_of_scope_invalid_ref() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(
                vec![],
                vec![
                    SpecRequirement::with_task_refs(
                        "excluded",
                        vec!["inference — not needed".into()],
                        vec![TaskId::try_new("T888").unwrap()],
                    )
                    .unwrap(),
                ],
            ),
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001"]);
        let invalid = doc.validate_all_task_refs(&valid);
        assert_eq!(invalid, &["T888"]);
    }

    #[test]
    fn test_validate_all_task_refs_empty_when_all_valid() {
        let doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal".into()],
            SpecScope::new(
                vec![
                    SpecRequirement::with_task_refs(
                        "in scope",
                        vec!["PRD §1".into()],
                        vec![TaskId::try_new("T001").unwrap()],
                    )
                    .unwrap(),
                ],
                vec![],
            ),
            vec![
                SpecRequirement::with_task_refs(
                    "constraint",
                    vec!["convention".into()],
                    vec![TaskId::try_new("T001").unwrap()],
                )
                .unwrap(),
            ],
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
            None,
        )
        .unwrap();

        let valid = make_task_id_set(&["T001"]);
        let invalid = doc.validate_all_task_refs(&valid);
        assert!(invalid.is_empty());
    }

    // --- HearingRecord (TSUMIKI-07) ---

    #[test]
    fn test_hearing_mode_as_str() {
        assert_eq!(HearingMode::Full.as_str(), "full");
        assert_eq!(HearingMode::Focused.as_str(), "focused");
        assert_eq!(HearingMode::Quick.as_str(), "quick");
    }

    #[test]
    fn test_hearing_signal_snapshot_from_signal_counts() {
        let counts = SignalCounts::new(10, 3, 1);
        let snap = HearingSignalSnapshot::from(counts);
        assert_eq!(snap.blue(), 10);
        assert_eq!(snap.yellow(), 3);
        assert_eq!(snap.red(), 1);
    }

    #[test]
    fn test_hearing_record_accessors() {
        let ts = Timestamp::new("2026-04-01T10:00:00Z").unwrap();
        let delta = HearingSignalDelta::new(
            HearingSignalSnapshot::new(5, 3, 2),
            HearingSignalSnapshot::new(8, 2, 0),
        );
        let rec = HearingRecord::new(ts.clone(), HearingMode::Focused, delta, 4, 1, 3);
        assert_eq!(rec.date(), &ts);
        assert_eq!(rec.mode(), HearingMode::Focused);
        assert_eq!(rec.signal_delta().before().blue(), 5);
        assert_eq!(rec.signal_delta().after().blue(), 8);
        assert_eq!(rec.questions_asked(), 4);
        assert_eq!(rec.items_added(), 1);
        assert_eq!(rec.items_modified(), 3);
    }

    #[test]
    fn test_document_hearing_history_append() {
        let mut doc = make_doc();
        assert!(doc.hearing_history().is_empty());

        let ts = Timestamp::new("2026-04-01T10:00:00Z").unwrap();
        let delta = HearingSignalDelta::new(
            HearingSignalSnapshot::new(0, 0, 0),
            HearingSignalSnapshot::new(5, 1, 0),
        );
        let rec = HearingRecord::new(ts, HearingMode::Full, delta, 5, 3, 0);
        doc.append_hearing_record(rec);
        assert_eq!(doc.hearing_history().len(), 1);
        assert_eq!(doc.hearing_history()[0].mode(), HearingMode::Full);
    }
}
