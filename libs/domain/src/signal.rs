//! Signal evaluation for spec requirement confidence (TSUMIKI-01).
//!
//! Two-stage signal architecture:
//! - Stage 1: spec signals — source tag provenance → `spec.md` frontmatter
//! - Stage 2: domain state signals → `metadata.json` (separate track)
//!
//! This module provides shared primitives (`ConfidenceSignal`, `SignalCounts`)
//! and Stage 1-specific types (`SignalBasis`, mapping functions).
//!
//! Traffic light levels:
//! - 🔵 Blue: confirmed with explicit evidence
//! - 🟡 Yellow: inferred or partially verified
//! - 🔴 Red: unverified or contradicted

/// Per-item confidence signal (traffic light level).
///
/// Shared between Stage 1 (spec signals) and Stage 2 (domain state signals).
///
/// # Examples
///
/// ```
/// use domain::ConfidenceSignal;
///
/// let signal = ConfidenceSignal::Blue;
/// assert!(signal > ConfidenceSignal::Yellow);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum ConfidenceSignal {
    /// 🔴 Unverified or contradicted.
    Red,
    /// 🟡 Inferred or partially verified.
    Yellow,
    /// 🔵 Confirmed with explicit evidence.
    Blue,
}

/// Basis (reason) behind a Stage 1 confidence signal assignment.
///
/// Each variant maps to a source tag pattern from the source-attribution convention.
/// Stage 2 does not use this enum — it models domain state entries directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SignalBasis {
    /// `[source: <doc> §<section>]` or `[source: <doc>]` — explicit document reference.
    Document,
    /// `[source: feedback — ...]` — user feedback or correction (undocumented, Yellow).
    Feedback,
    /// `[source: convention — ...]` — established project convention.
    Convention,
    /// `[source: discussion]` — agreed in discussion.
    Discussion,
    /// `[source: inference — ...]` — inferred from context.
    Inference,
    /// No source tag present.
    MissingSource,
}

impl SignalBasis {
    /// Returns the confidence signal level for this basis.
    ///
    /// # Examples
    ///
    /// ```
    /// use domain::{ConfidenceSignal, SignalBasis};
    ///
    /// assert_eq!(SignalBasis::Document.signal(), ConfidenceSignal::Blue);
    /// assert_eq!(SignalBasis::Inference.signal(), ConfidenceSignal::Yellow);
    /// assert_eq!(SignalBasis::MissingSource.signal(), ConfidenceSignal::Red);
    /// ```
    #[must_use]
    pub const fn signal(&self) -> ConfidenceSignal {
        match self {
            Self::Document | Self::Convention => ConfidenceSignal::Blue,
            Self::Feedback | Self::Discussion | Self::Inference => ConfidenceSignal::Yellow,
            Self::MissingSource => ConfidenceSignal::Red,
        }
    }
}

/// Classifies a single source tag body into a `SignalBasis`.
///
/// The input is the text between `[source: ` and `]`, already trimmed.
/// Returns `None` if the input is empty (caller should use `MissingSource`).
///
/// # Examples
///
/// ```
/// use domain::{SignalBasis, classify_source_tag};
///
/// assert_eq!(classify_source_tag("PRD §3.2"), Some(SignalBasis::Document));
/// assert_eq!(classify_source_tag("feedback — Rust-first"), Some(SignalBasis::Feedback));
/// assert_eq!(classify_source_tag("convention — security.md"), Some(SignalBasis::Convention));
/// assert_eq!(classify_source_tag("discussion"), Some(SignalBasis::Discussion));
/// assert_eq!(classify_source_tag("inference — best practice"), Some(SignalBasis::Inference));
/// assert_eq!(classify_source_tag(""), None);
/// ```
#[must_use]
pub fn classify_source_tag(tag_body: &str) -> Option<SignalBasis> {
    let tag_body = tag_body.trim();
    if tag_body.is_empty() {
        return None;
    }

    // Check prefixes in order of specificity
    if tag_body.starts_with("feedback") {
        Some(SignalBasis::Feedback)
    } else if tag_body.starts_with("convention") {
        Some(SignalBasis::Convention)
    } else if tag_body.starts_with("inference") {
        Some(SignalBasis::Inference)
    } else if tag_body == "discussion" || tag_body.starts_with("discussion ") {
        Some(SignalBasis::Discussion)
    } else {
        // Any other non-empty tag is a document reference
        Some(SignalBasis::Document)
    }
}

/// Evaluates a source tag string (the content between `[source: ` and `]`).
///
/// Returns the confidence signal and basis for the tag.
/// Returns `(ConfidenceSignal::Red, SignalBasis::MissingSource)` if the tag body is empty.
///
/// Multi-source (comma-separated) tags are deferred to the JSON SSoT track.
/// Currently the entire tag body is classified as a single source.
///
/// # Examples
///
/// ```
/// use domain::{ConfidenceSignal, SignalBasis, evaluate_source_tag};
///
/// let (signal, basis) = evaluate_source_tag("PRD §3.2");
/// assert_eq!(signal, ConfidenceSignal::Blue);
/// assert_eq!(basis, SignalBasis::Document);
///
/// let (signal, basis) = evaluate_source_tag("");
/// assert_eq!(signal, ConfidenceSignal::Red);
/// assert_eq!(basis, SignalBasis::MissingSource);
/// ```
#[must_use]
pub fn evaluate_source_tag(tag_body: &str) -> (ConfidenceSignal, SignalBasis) {
    let tag_body = tag_body.trim();
    if tag_body.is_empty() {
        return (ConfidenceSignal::Red, SignalBasis::MissingSource);
    }

    // Single-source evaluation. Multi-source (comma-separated) is deferred to
    // JSON SSoT track where sources are modeled as a JSON array.
    match classify_source_tag(tag_body) {
        Some(basis) => (basis.signal(), basis),
        None => (ConfidenceSignal::Red, SignalBasis::MissingSource),
    }
}

/// Aggregate signal counts for a spec document.
///
/// All counts are non-negative by construction (`u32`).
///
/// # Examples
///
/// ```
/// use domain::SignalCounts;
///
/// let signals = SignalCounts::new(12, 1, 0);
/// assert_eq!(signals.blue(), 12);
/// assert_eq!(signals.total(), 13);
/// assert!(!signals.has_red());
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SignalCounts {
    blue: u32,
    yellow: u32,
    red: u32,
}

impl SignalCounts {
    /// Creates a new `SignalCounts`.
    #[must_use]
    pub const fn new(blue: u32, yellow: u32, red: u32) -> Self {
        Self { blue, yellow, red }
    }

    /// Returns the blue (confirmed) count.
    #[must_use]
    pub const fn blue(&self) -> u32 {
        self.blue
    }

    /// Returns the yellow (inferred) count.
    #[must_use]
    pub const fn yellow(&self) -> u32 {
        self.yellow
    }

    /// Returns the red (unverified) count.
    #[must_use]
    pub const fn red(&self) -> u32 {
        self.red
    }

    /// Returns the total number of signals.
    #[must_use]
    pub const fn total(&self) -> u32 {
        self.blue + self.yellow + self.red
    }

    /// Returns `true` if any red signals exist.
    #[must_use]
    pub const fn has_red(&self) -> bool {
        self.red > 0
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use rstest::rstest;

    use super::*;

    // --- SignalCounts tests ---

    #[test]
    fn test_signal_counts_new_and_accessors() {
        let s = SignalCounts::new(12, 1, 0);
        assert_eq!(s.blue(), 12);
        assert_eq!(s.yellow(), 1);
        assert_eq!(s.red(), 0);
        assert_eq!(s.total(), 13);
        assert!(!s.has_red());
    }

    #[test]
    fn test_signal_counts_has_red() {
        let s = SignalCounts::new(0, 0, 1);
        assert!(s.has_red());
    }

    #[test]
    fn test_signal_counts_zero() {
        let s = SignalCounts::new(0, 0, 0);
        assert_eq!(s.total(), 0);
        assert!(!s.has_red());
    }

    // --- ConfidenceSignal ordering tests ---

    #[test]
    fn test_confidence_signal_ordering_blue_is_highest() {
        assert!(ConfidenceSignal::Blue > ConfidenceSignal::Yellow);
        assert!(ConfidenceSignal::Yellow > ConfidenceSignal::Red);
        assert!(ConfidenceSignal::Blue > ConfidenceSignal::Red);
    }

    // --- SignalBasis → ConfidenceSignal mapping tests ---

    #[rstest]
    #[case::document(SignalBasis::Document, ConfidenceSignal::Blue)]
    #[case::feedback(SignalBasis::Feedback, ConfidenceSignal::Yellow)]
    #[case::convention(SignalBasis::Convention, ConfidenceSignal::Blue)]
    #[case::discussion(SignalBasis::Discussion, ConfidenceSignal::Yellow)]
    #[case::inference(SignalBasis::Inference, ConfidenceSignal::Yellow)]
    #[case::missing_source(SignalBasis::MissingSource, ConfidenceSignal::Red)]
    fn test_signal_basis_maps_to_correct_signal(
        #[case] basis: SignalBasis,
        #[case] expected: ConfidenceSignal,
    ) {
        assert_eq!(basis.signal(), expected);
    }

    // --- classify_source_tag tests ---

    #[rstest]
    #[case::document_with_section("PRD §3.2", Some(SignalBasis::Document))]
    #[case::document_plain("track/tech-stack.md", Some(SignalBasis::Document))]
    #[case::document_with_section_jp(
        "tmp/TODO-PLAN-2026-03-22.md §Phase 2",
        Some(SignalBasis::Document)
    )]
    #[case::feedback("feedback — Rust-first policy", Some(SignalBasis::Feedback))]
    #[case::convention(
        "convention — knowledge/conventions/security.md",
        Some(SignalBasis::Convention)
    )]
    #[case::discussion("discussion", Some(SignalBasis::Discussion))]
    #[case::discussion_with_context(
        "discussion — PR #2, #36 実データ分析",
        Some(SignalBasis::Discussion)
    )]
    #[case::inference("inference — security best practice", Some(SignalBasis::Inference))]
    #[case::empty("", None)]
    #[case::whitespace_only("   ", None)]
    fn test_classify_source_tag(#[case] input: &str, #[case] expected: Option<SignalBasis>) {
        assert_eq!(classify_source_tag(input), expected);
    }

    // --- evaluate_source_tag tests ---

    #[test]
    fn test_evaluate_source_tag_single_document() {
        let (signal, basis) = evaluate_source_tag("PRD §3.2");
        assert_eq!(signal, ConfidenceSignal::Blue);
        assert_eq!(basis, SignalBasis::Document);
    }

    #[test]
    fn test_evaluate_source_tag_single_inference() {
        let (signal, basis) = evaluate_source_tag("inference — guess");
        assert_eq!(signal, ConfidenceSignal::Yellow);
        assert_eq!(basis, SignalBasis::Inference);
    }

    #[test]
    fn test_evaluate_source_tag_empty_returns_missing() {
        let (signal, basis) = evaluate_source_tag("");
        assert_eq!(signal, ConfidenceSignal::Red);
        assert_eq!(basis, SignalBasis::MissingSource);
    }

    #[test]
    fn test_evaluate_source_tag_discussion_with_comma_in_context() {
        // Commas in context text are NOT treated as multi-source separators
        let (signal, basis) = evaluate_source_tag("discussion — PR #2, #36 実データ分析");
        assert_eq!(signal, ConfidenceSignal::Yellow);
        assert_eq!(basis, SignalBasis::Discussion);
    }
}
