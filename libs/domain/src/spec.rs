//! Domain types for structured spec documents (spec.json SSoT).
//!
//! `SpecDocument` is the aggregate root for a feature specification.
//! `spec.json` is the SSoT; `spec.md` is a read-only rendered view.

use std::collections::HashSet;
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
    acceptance_criteria: Vec<SpecRequirement>,
    additional_sections: Vec<SpecSection>,
    related_conventions: Vec<String>,
    signals: Option<SignalCounts>,
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
        acceptance_criteria: Vec<SpecRequirement>,
        additional_sections: Vec<SpecSection>,
        related_conventions: Vec<String>,
        signals: Option<SignalCounts>,
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
            acceptance_criteria,
            additional_sections,
            related_conventions,
            signals,
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

    /// Appends a hearing record to the history (append-only).
    pub fn append_hearing_record(&mut self, record: HearingRecord) {
        self.hearing_history.push(record);
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
// Stage 1 signal gate (check_spec_doc_signals)
// ---------------------------------------------------------------------------

/// Evaluates Stage 1 signal gate rules against a `SpecDocument`.
///
/// Shared pure function used by both the CI path (`verify_from_spec_json`)
/// and the merge gate (via `usecase::merge_gate::check_strict_merge_gate`).
///
/// # Rules
///
/// - `signals` is `None` → `VerifyFinding::error` (unevaluated; run `sotp track signals` first)
/// - `SignalCounts::total() == 0` → `VerifyFinding::error` (evaluated but empty — treated as unevaluated)
/// - `signals.red > 0` → `VerifyFinding::error` (red is always an error, regardless of mode)
/// - `signals.yellow > 0` and `strict = true` → `VerifyFinding::error` (merge gate rejects Yellow)
/// - `signals.yellow > 0` and `strict = false` → `VerifyFinding::warning` (interim mode visualizes Yellow, PASSes)
/// - All Blue → `VerifyOutcome::pass()` (no findings)
///
/// The `strict` parameter is:
/// - `true` for the merge gate (all Yellow must be upgraded to Blue before merge)
/// - `false` for CI interim mode (Yellow is allowed during iteration but visualized)
///
/// Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` §D2, §D8.6.
#[must_use]
pub fn check_spec_doc_signals(doc: &SpecDocument, strict: bool) -> crate::verify::VerifyOutcome {
    use crate::verify::{VerifyFinding, VerifyOutcome};

    let Some(counts) = doc.signals() else {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(
            "spec signals not yet evaluated — run `sotp track signals` first".to_owned(),
        )]);
    };

    if counts.total() == 0 {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(
            "spec signals are all-zero (blue=0, yellow=0, red=0) — treated as unevaluated"
                .to_owned(),
        )]);
    }

    if counts.has_red() {
        return VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "spec signals have red={} (source attribution missing — every requirement must carry a `[source: ...]` tag)",
            counts.red()
        ))]);
    }

    if counts.yellow() > 0 {
        let message = format!(
            "spec.json: {} yellow signal(s) detected — merge gate will block these until upgraded to Blue. Upgrade by creating an ADR or convention document and referencing it via `[source: ...]` tag.",
            counts.yellow()
        );
        if strict {
            return VerifyOutcome::from_findings(vec![VerifyFinding::error(message)]);
        }
        return VerifyOutcome::from_findings(vec![VerifyFinding::warning(message)]);
    }

    VerifyOutcome::pass()
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
            vec![SpecRequirement::new("AC 1", vec![]).unwrap()],
            vec![],
            vec!["knowledge/conventions/hex.md".into()],
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
            vec![SpecRequirement::new("uncovered AC", vec!["discussion".into()]).unwrap()],
            vec![],
            vec![],
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

    // --- check_spec_doc_signals (Stage 1 signal gate) ---
    //
    // These tests cover the pure function that both the CI path
    // (`verify_from_spec_json`) and the merge gate (via `usecase::merge_gate`)
    // delegate to. Cases mirror the D1–D6 rows in the ADR Test Matrix.

    fn doc_with_signals(signals: Option<SignalCounts>) -> SpecDocument {
        let mut doc = SpecDocument::new(
            "Feature",
            SpecStatus::Draft,
            "1.0",
            vec!["Goal line".to_owned()],
            SpecScope::new(Vec::new(), Vec::new()),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            None,
            None,
            None,
        )
        .unwrap();
        if let Some(counts) = signals {
            doc.set_signals(counts);
        }
        doc
    }

    #[test]
    fn test_check_spec_doc_signals_none_returns_error() {
        // D1: signals=None → BLOCKED (unevaluated)
        let doc = doc_with_signals(None);
        let outcome = check_spec_doc_signals(&doc, false);
        assert!(outcome.has_errors(), "None signals must be an error");
        assert!(
            outcome.findings()[0].message().contains("not yet evaluated"),
            "finding must mention unevaluated state: {:?}",
            outcome.findings()[0].message()
        );
    }

    #[test]
    fn test_check_spec_doc_signals_all_zero_returns_error() {
        // D2: signals=(0,0,0) → BLOCKED (evaluated but empty — treated as unevaluated)
        let doc = doc_with_signals(Some(SignalCounts::new(0, 0, 0)));
        let outcome = check_spec_doc_signals(&doc, false);
        assert!(outcome.has_errors(), "all-zero signals must be an error");
        assert!(outcome.findings()[0].message().contains("all-zero"));
    }

    #[test]
    fn test_check_spec_doc_signals_red_is_error_in_interim_mode() {
        // D3a: red>0, strict=false → BLOCKED (red is always an error)
        let doc = doc_with_signals(Some(SignalCounts::new(1, 0, 2)));
        let outcome = check_spec_doc_signals(&doc, false);
        assert!(outcome.has_errors(), "red>0 must be an error in interim mode");
        assert!(outcome.findings()[0].message().contains("red=2"));
    }

    #[test]
    fn test_check_spec_doc_signals_red_is_error_in_strict_mode() {
        // D3b: red>0, strict=true → BLOCKED
        let doc = doc_with_signals(Some(SignalCounts::new(1, 0, 2)));
        let outcome = check_spec_doc_signals(&doc, true);
        assert!(outcome.has_errors(), "red>0 must be an error in strict mode");
    }

    #[test]
    fn test_check_spec_doc_signals_yellow_is_warning_in_interim_mode() {
        // D4: yellow>0, strict=false → PASS with VerifyFinding::warning
        let doc = doc_with_signals(Some(SignalCounts::new(3, 2, 0)));
        let outcome = check_spec_doc_signals(&doc, false);
        assert!(!outcome.has_errors(), "yellow in interim mode must not be an error: {outcome:?}");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1, "expected exactly one warning finding");
        assert_eq!(findings[0].severity(), crate::verify::Severity::Warning);
        let msg = findings[0].message();
        assert!(msg.contains("2 yellow signal"), "message must mention yellow count: {msg}");
        assert!(msg.contains("merge gate will block"), "message must warn about merge gate: {msg}");
    }

    #[test]
    fn test_check_spec_doc_signals_yellow_is_error_in_strict_mode() {
        // D5: yellow>0, strict=true → BLOCKED with VerifyFinding::error
        let doc = doc_with_signals(Some(SignalCounts::new(3, 2, 0)));
        let outcome = check_spec_doc_signals(&doc, true);
        assert!(outcome.has_errors(), "yellow in strict mode must be an error");
        let findings = outcome.findings();
        assert_eq!(findings.len(), 1);
        assert_eq!(findings[0].severity(), crate::verify::Severity::Error);
        assert!(findings[0].message().contains("2 yellow signal"));
    }

    #[test]
    fn test_check_spec_doc_signals_all_blue_passes_in_both_modes() {
        // D6: all Blue → PASS (no findings) in both modes
        let doc = doc_with_signals(Some(SignalCounts::new(10, 0, 0)));

        let outcome_interim = check_spec_doc_signals(&doc, false);
        assert!(!outcome_interim.has_errors());
        assert!(
            outcome_interim.findings().is_empty(),
            "all-Blue must produce zero findings in interim mode"
        );

        let outcome_strict = check_spec_doc_signals(&doc, true);
        assert!(!outcome_strict.has_errors());
        assert!(
            outcome_strict.findings().is_empty(),
            "all-Blue must produce zero findings in strict mode"
        );
    }
}
