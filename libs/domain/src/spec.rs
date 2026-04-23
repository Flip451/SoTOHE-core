//! Domain types for structured spec documents (spec.json SSoT).
//!
//! `SpecDocument` is the aggregate root for a feature specification.
//! `spec.json` is the SSoT; `spec.md` is a read-only rendered view.
//!
//! ADR 2026-04-19-1242 §D1.2 / §D2.1: the approved-lifecycle (`status` /
//! `approved_at` / `content_hash`) is removed; each requirement gains a
//! required `id: SpecElementId`; `sources: Vec<String>` is replaced by three
//! typed ref fields; `task_refs` moves to `task-coverage.json`.

use std::collections::HashSet;
use std::fmt;

use crate::plan_ref::{AdrRef, ConventionRef, InformalGroundRef, SpecElementId};
use crate::{ConfidenceSignal, SignalCounts, Timestamp};

// ---------------------------------------------------------------------------
// Value objects
// ---------------------------------------------------------------------------

/// A single requirement item with typed provenance references.
///
/// Used in goal-items, Scope (in/out), Constraints, and Acceptance Criteria.
///
/// # Errors
///
/// Returns `SpecValidationError::EmptyRequirementText` if text is empty.
/// Returns `SpecValidationError::MissingElementId` if id construction fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecRequirement {
    id: SpecElementId,
    text: String,
    adr_refs: Vec<AdrRef>,
    convention_refs: Vec<ConventionRef>,
    informal_grounds: Vec<InformalGroundRef>,
}

impl SpecRequirement {
    /// Creates a new requirement with typed provenance references.
    ///
    /// # Errors
    ///
    /// Returns error if `text` is empty or whitespace-only.
    pub fn new(
        id: SpecElementId,
        text: impl Into<String>,
        adr_refs: Vec<AdrRef>,
        convention_refs: Vec<ConventionRef>,
        informal_grounds: Vec<InformalGroundRef>,
    ) -> Result<Self, SpecValidationError> {
        let text = text.into();
        if text.trim().is_empty() {
            return Err(SpecValidationError::EmptyRequirementText);
        }
        Ok(Self { id, text, adr_refs, convention_refs, informal_grounds })
    }

    /// Returns the element identifier.
    #[must_use]
    pub fn id(&self) -> &SpecElementId {
        &self.id
    }

    /// Returns the requirement text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns the ADR references (formal grounding → Blue).
    #[must_use]
    pub fn adr_refs(&self) -> &[AdrRef] {
        &self.adr_refs
    }

    /// Returns the convention references (formal grounding → Blue).
    #[must_use]
    pub fn convention_refs(&self) -> &[ConventionRef] {
        &self.convention_refs
    }

    /// Returns the informal ground references (unpersisted → Yellow).
    #[must_use]
    pub fn informal_grounds(&self) -> &[InformalGroundRef] {
        &self.informal_grounds
    }

    /// Evaluates the confidence signal for this requirement.
    ///
    /// Signal rules (ADR 2026-04-19-1242 §D3.1):
    /// - `informal_grounds[]` non-empty → 🟡 Yellow (takes priority regardless of
    ///   `adr_refs[]`; any remaining informal ground requires promotion to a
    ///   formal ref before merge)
    /// - `informal_grounds[]` empty + `adr_refs[]` non-empty → 🔵 Blue
    /// - both empty → 🔴 Red
    ///
    /// `convention_refs[]` is outside the signal evaluation scope (D3.1: "signal 評価対象外").
    #[must_use]
    pub fn signal(&self) -> ConfidenceSignal {
        evaluate_requirement_signal(&self.adr_refs, &self.informal_grounds)
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
///
/// ADR 2026-04-19-1242 §D1.2: removed `status`, `approved_at`, `content_hash`.
/// `related_conventions` is now `Vec<ConventionRef>`. HearingRecord history
/// is retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecDocument {
    title: String,
    version: String,
    goal: Vec<SpecRequirement>,
    scope: SpecScope,
    constraints: Vec<SpecRequirement>,
    acceptance_criteria: Vec<SpecRequirement>,
    additional_sections: Vec<SpecSection>,
    related_conventions: Vec<ConventionRef>,
    signals: Option<SignalCounts>,
    hearing_history: Vec<HearingRecord>,
}

impl SpecDocument {
    /// Creates a new spec document.
    ///
    /// # Errors
    ///
    /// Returns error if `title` or `version` is empty, or if element ids are
    /// not unique across all requirement sections.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        title: impl Into<String>,
        version: impl Into<String>,
        goal: Vec<SpecRequirement>,
        scope: SpecScope,
        constraints: Vec<SpecRequirement>,
        acceptance_criteria: Vec<SpecRequirement>,
        additional_sections: Vec<SpecSection>,
        related_conventions: Vec<ConventionRef>,
        signals: Option<SignalCounts>,
    ) -> Result<Self, SpecValidationError> {
        let title = title.into();
        let version = version.into();
        if title.trim().is_empty() {
            return Err(SpecValidationError::EmptyTitle);
        }
        if version.trim().is_empty() {
            return Err(SpecValidationError::EmptyVersion);
        }

        // Validate element id uniqueness across all requirement sections.
        let mut seen_ids: HashSet<String> = HashSet::new();
        let all_reqs = goal
            .iter()
            .chain(scope.in_scope.iter())
            .chain(scope.out_of_scope.iter())
            .chain(constraints.iter())
            .chain(acceptance_criteria.iter());

        for req in all_reqs {
            let id_str = req.id().as_ref().to_owned();
            if !seen_ids.insert(id_str.clone()) {
                return Err(SpecValidationError::DuplicateElementId(id_str));
            }
        }

        Ok(Self {
            title,
            version,
            goal,
            scope,
            constraints,
            acceptance_criteria,
            additional_sections,
            related_conventions,
            signals,
            hearing_history: vec![],
        })
    }

    #[must_use]
    pub fn title(&self) -> &str {
        &self.title
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

    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    #[must_use]
    pub fn goal(&self) -> &[SpecRequirement] {
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
    pub fn related_conventions(&self) -> &[ConventionRef] {
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
    /// Evaluable sections: goal, scope (in + out), constraints, acceptance criteria.
    #[must_use]
    pub fn evaluate_signals(&self) -> SignalCounts {
        let mut blue: u32 = 0;
        let mut yellow: u32 = 0;
        let mut red: u32 = 0;

        let all_requirements = self
            .goal
            .iter()
            .chain(self.scope.in_scope.iter())
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

/// Evaluates the confidence signal for a requirement's typed references.
///
/// Rules (ADR 2026-04-19-1242 §D3.1):
/// - `informal_grounds[]` non-empty → 🟡 Yellow (unpersisted grounding; takes
///   priority regardless of `adr_refs[]` because any remaining informal ground
///   means the element still needs promotion to a formal ref before merge)
/// - `informal_grounds[]` empty + `adr_refs[]` non-empty → 🔵 Blue (formal
///   ADR grounding with no pending promotion)
/// - both empty → 🔴 Red
///
/// `convention_refs[]` is outside the signal evaluation scope per ADR D3.1
/// ("signal 評価対象外"). The presence of convention references does not
/// affect the signal; only `adr_refs` and `informal_grounds` are evaluated.
#[must_use]
pub fn evaluate_requirement_signal(
    adr_refs: &[AdrRef],
    informal_grounds: &[InformalGroundRef],
) -> ConfidenceSignal {
    if !informal_grounds.is_empty() {
        return ConfidenceSignal::Yellow;
    }
    if !adr_refs.is_empty() {
        return ConfidenceSignal::Blue;
    }
    ConfidenceSignal::Red
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
            "spec signals have red={} (source attribution missing — every requirement must carry typed refs)",
            counts.red()
        ))]);
    }

    if counts.yellow() > 0 {
        let message = format!(
            "spec.json: {} yellow signal(s) detected — merge gate will block these until upgraded to Blue. Upgrade by creating an ADR and referencing it via adr_refs[] (convention_refs[] are outside signal evaluation scope per ADR D3.1 and do not affect the signal).",
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
    #[error("requirement text must not be empty")]
    EmptyRequirementText,
    #[error("domain state name must not be empty")]
    EmptyDomainStateName,
    #[error("section title must not be empty")]
    EmptySectionTitle,
    #[error("requirement element id must not be empty")]
    MissingElementId,
    #[error("duplicate element id '{0}' — ids must be unique across all requirement sections")]
    DuplicateElementId(String),
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::indexing_slicing, clippy::expect_used, clippy::panic)]
mod tests {
    use std::path::PathBuf;

    use crate::plan_ref::{
        AdrAnchor, AdrRef, ConventionAnchor, ConventionRef, InformalGroundKind, InformalGroundRef,
        InformalGroundSummary, SpecElementId,
    };

    use super::*;

    // --- helpers ---

    fn make_adr_ref(file: &str, anchor: &str) -> AdrRef {
        AdrRef::new(PathBuf::from(file), AdrAnchor::try_new(anchor).unwrap())
    }

    fn make_conv_ref(file: &str, anchor: &str) -> ConventionRef {
        ConventionRef::new(PathBuf::from(file), ConventionAnchor::try_new(anchor).unwrap())
    }

    fn make_informal(kind: InformalGroundKind, summary: &str) -> InformalGroundRef {
        InformalGroundRef::new(kind, InformalGroundSummary::try_new(summary).unwrap())
    }

    fn id(s: &str) -> SpecElementId {
        SpecElementId::try_new(s).unwrap()
    }

    fn req_blue(id_s: &str, text: &str) -> SpecRequirement {
        SpecRequirement::new(
            id(id_s),
            text,
            vec![make_adr_ref("knowledge/adr/2026-04-19-1242.md", "D1.2")],
            vec![],
            vec![],
        )
        .unwrap()
    }

    fn req_blue_conv(id_s: &str, text: &str) -> SpecRequirement {
        SpecRequirement::new(
            id(id_s),
            text,
            vec![],
            vec![make_conv_ref(".claude/rules/04-coding-principles.md", "newtype-pattern")],
            vec![],
        )
        .unwrap()
    }

    fn req_yellow(id_s: &str, text: &str) -> SpecRequirement {
        SpecRequirement::new(
            id(id_s),
            text,
            vec![],
            vec![],
            vec![make_informal(InformalGroundKind::Discussion, "agreed in spec review")],
        )
        .unwrap()
    }

    fn req_red(id_s: &str, text: &str) -> SpecRequirement {
        SpecRequirement::new(id(id_s), text, vec![], vec![], vec![]).unwrap()
    }

    fn make_doc() -> SpecDocument {
        SpecDocument::new(
            "Feature X",
            "1.0",
            vec![],
            SpecScope::new(
                vec![req_blue("IN-01", "in scope")],
                vec![req_yellow("OS-01", "out scope")],
            ),
            vec![req_blue("CO-01", "constraint")],
            vec![req_red("AC-01", "AC 1")],
            vec![],
            vec![],
            None,
        )
        .unwrap()
    }

    // --- SpecRequirement ---

    #[test]
    fn test_requirement_with_valid_text_succeeds() {
        let req = req_blue("IN-01", "Enable feature X");
        assert_eq!(req.text(), "Enable feature X");
        assert_eq!(req.adr_refs().len(), 1);
        assert!(req.convention_refs().is_empty());
        assert!(req.informal_grounds().is_empty());
    }

    #[test]
    fn test_requirement_with_empty_text_returns_error() {
        let result = SpecRequirement::new(id("IN-01"), "", vec![], vec![], vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyRequirementText)));
    }

    #[test]
    fn test_requirement_with_whitespace_text_returns_error() {
        let result = SpecRequirement::new(id("IN-01"), "   ", vec![], vec![], vec![]);
        assert!(matches!(result, Err(SpecValidationError::EmptyRequirementText)));
    }

    // --- Signal evaluation ---

    #[test]
    fn test_requirement_signal_with_adr_refs_is_blue() {
        let req = req_blue("IN-01", "req");
        assert_eq!(req.signal(), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_requirement_signal_with_convention_refs_only_is_red() {
        // convention_refs are outside signal evaluation scope per ADR D3.1;
        // convention-only requirement must evaluate to Red.
        let req = req_blue_conv("IN-01", "req");
        assert_eq!(req.signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_requirement_signal_with_informal_grounds_only_is_yellow() {
        let req = req_yellow("IN-01", "req");
        assert_eq!(req.signal(), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_requirement_signal_with_no_refs_is_red() {
        let req = req_red("IN-01", "req");
        assert_eq!(req.signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_requirement_signal_informal_takes_priority_over_adr_refs() {
        // Per ADR 2026-04-19-1242 §D3.1, informal_grounds[] non-empty forces Yellow
        // regardless of adr_refs, because any remaining informal ground means the
        // element still needs promotion to a formal ref before merge.
        let req = SpecRequirement::new(
            id("IN-01"),
            "req",
            vec![make_adr_ref("knowledge/adr/x.md", "D1")],
            vec![],
            vec![make_informal(InformalGroundKind::Discussion, "fallback")],
        )
        .unwrap();
        assert_eq!(req.signal(), ConfidenceSignal::Yellow);
    }

    // --- evaluate_requirement_signal ---

    #[test]
    fn test_evaluate_requirement_signal_adr_refs_only_gives_blue() {
        let adr = vec![make_adr_ref("knowledge/adr/x.md", "D1")];
        assert_eq!(evaluate_requirement_signal(&adr, &[]), ConfidenceSignal::Blue);
    }

    #[test]
    fn test_evaluate_requirement_signal_adr_refs_plus_informal_gives_yellow() {
        // Regression guard for the informal-priority rule: even when adr_refs is
        // non-empty, a remaining informal_grounds entry yields Yellow (merge still
        // blocked until the informal ground is promoted to a formal ref).
        let adr = vec![make_adr_ref("knowledge/adr/x.md", "D1")];
        let informal = vec![make_informal(InformalGroundKind::Discussion, "pending")];
        assert_eq!(evaluate_requirement_signal(&adr, &informal), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_requirement_signal_convention_refs_gives_red() {
        // convention_refs are outside signal evaluation scope per ADR D3.1;
        // convention-only arguments must evaluate to Red (no adr_refs, no informal_grounds).
        // Note: convention_refs are not a parameter; this test verifies via req_blue_conv.
        let req = req_blue_conv("IN-01", "req");
        assert_eq!(req.signal(), ConfidenceSignal::Red);
    }

    #[test]
    fn test_evaluate_requirement_signal_informal_only_gives_yellow() {
        let informal = vec![make_informal(InformalGroundKind::Discussion, "summary")];
        assert_eq!(evaluate_requirement_signal(&[], &informal), ConfidenceSignal::Yellow);
    }

    #[test]
    fn test_evaluate_requirement_signal_empty_gives_red() {
        assert_eq!(evaluate_requirement_signal(&[], &[]), ConfidenceSignal::Red);
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

    #[test]
    fn test_document_creation_succeeds() {
        let doc = make_doc();
        assert_eq!(doc.title(), "Feature X");
        assert_eq!(doc.version(), "1.0");
        assert!(doc.signals().is_none());
    }

    #[test]
    fn test_document_with_empty_title_returns_error() {
        let result = SpecDocument::new(
            "",
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        );
        assert!(matches!(result, Err(SpecValidationError::EmptyTitle)));
    }

    #[test]
    fn test_document_with_empty_version_returns_error() {
        let result = SpecDocument::new(
            "T",
            "",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        );
        assert!(matches!(result, Err(SpecValidationError::EmptyVersion)));
    }

    #[test]
    fn test_document_duplicate_element_id_returns_error() {
        // Two requirements in different sections share the same id.
        let result = SpecDocument::new(
            "T",
            "1.0",
            vec![],
            SpecScope::new(vec![req_blue("IN-01", "in scope")], vec![]),
            vec![req_red("IN-01", "constraint with duplicate id")], // duplicate!
            vec![],
            vec![],
            vec![],
            None,
        );
        assert!(matches!(result, Err(SpecValidationError::DuplicateElementId(_))));
    }

    #[test]
    fn test_document_evaluate_signals() {
        let doc = make_doc();
        let signals = doc.evaluate_signals();
        // in_scope: Blue (adr_ref), out_scope: Yellow (informal),
        // constraint: Blue (convention_ref), AC1: Red (no refs)
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

    #[test]
    fn test_document_goal_is_included_in_signals() {
        let doc = SpecDocument::new(
            "T",
            "1.0",
            vec![req_blue("GL-01", "goal item")],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let signals = doc.evaluate_signals();
        assert_eq!(signals.blue(), 1);
    }

    #[test]
    fn test_document_related_conventions_is_convention_refs() {
        let doc = SpecDocument::new(
            "T",
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![make_conv_ref("knowledge/conventions/source-attribution.md", "intro")],
            None,
        )
        .unwrap();
        assert_eq!(doc.related_conventions().len(), 1);
    }

    // --- HearingRecord ---

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

    fn doc_with_signals(signals: Option<SignalCounts>) -> SpecDocument {
        let mut doc = SpecDocument::new(
            "Feature",
            "1.0",
            vec![],
            SpecScope::new(Vec::new(), Vec::new()),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
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
        let doc = doc_with_signals(Some(SignalCounts::new(0, 0, 0)));
        let outcome = check_spec_doc_signals(&doc, false);
        assert!(outcome.has_errors(), "all-zero signals must be an error");
        assert!(outcome.findings()[0].message().contains("all-zero"));
    }

    #[test]
    fn test_check_spec_doc_signals_red_is_error_in_interim_mode() {
        let doc = doc_with_signals(Some(SignalCounts::new(1, 0, 2)));
        let outcome = check_spec_doc_signals(&doc, false);
        assert!(outcome.has_errors(), "red>0 must be an error in interim mode");
        assert!(outcome.findings()[0].message().contains("red=2"));
    }

    #[test]
    fn test_check_spec_doc_signals_red_is_error_in_strict_mode() {
        let doc = doc_with_signals(Some(SignalCounts::new(1, 0, 2)));
        let outcome = check_spec_doc_signals(&doc, true);
        assert!(outcome.has_errors(), "red>0 must be an error in strict mode");
    }

    #[test]
    fn test_check_spec_doc_signals_yellow_is_warning_in_interim_mode() {
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

    // --- New signal tests: adr_refs → Blue, informal → Yellow, empty → Red ---

    #[test]
    fn test_signal_non_empty_adr_refs_gives_blue() {
        let req = req_blue("IN-01", "req with adr");
        assert_eq!(req.signal(), ConfidenceSignal::Blue, "non-empty adr_refs must give Blue");
    }

    #[test]
    fn test_signal_convention_refs_only_gives_red() {
        // convention_refs are outside signal evaluation scope per ADR D3.1.
        let req = req_blue_conv("IN-01", "req with conv");
        assert_eq!(
            req.signal(),
            ConfidenceSignal::Red,
            "convention_refs alone must give Red (outside signal evaluation scope per ADR D3.1)"
        );
    }

    #[test]
    fn test_signal_empty_adr_refs_non_empty_informal_gives_yellow() {
        let req = req_yellow("IN-01", "req with informal");
        assert_eq!(req.signal(), ConfidenceSignal::Yellow, "informal only must give Yellow");
    }

    #[test]
    fn test_signal_all_empty_gives_red() {
        let req = req_red("IN-01", "req with no refs");
        assert_eq!(req.signal(), ConfidenceSignal::Red, "all empty must give Red");
    }
}
