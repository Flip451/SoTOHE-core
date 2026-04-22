//! `InformalGroundRef` and its supporting newtypes.

use std::fmt;

use crate::ValidationError;

/// Finite classification of an unpersisted ground.
///
/// An informal ground is a rationale that has not yet been promoted to a
/// formal document (ADR / convention / spec / type catalogue). Non-empty
/// `informal_grounds[]` on a spec element or catalogue entry drives a 🟡
/// signal per ADR 2026-04-19-1242 §D3.1 / §D3.2.
///
/// Enum-first per `.claude/rules/04-coding-principles.md` § Enum-first:
/// there are no state transitions, just a closed set of variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum InformalGroundKind {
    /// In-session discussion between author and reviewer.
    Discussion,
    /// Recorded feedback captured outside a formal document.
    Feedback,
    /// Notes persisted to an auto-memory store.
    Memory,
    /// A user-issued directive recorded inline during the track session.
    UserDirective,
}

impl InformalGroundKind {
    /// Machine-readable string form used by codecs / signal evaluators.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Discussion => "discussion",
            Self::Feedback => "feedback",
            Self::Memory => "memory",
            Self::UserDirective => "user_directive",
        }
    }
}

impl fmt::Display for InformalGroundKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Validated newtype for a single-line summary of an informal ground.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct InformalGroundSummary(String);

impl InformalGroundSummary {
    /// Validate and wrap `value` as an [`InformalGroundSummary`].
    ///
    /// # Errors
    ///
    /// * [`ValidationError::EmptyInformalGroundSummary`] when `value` is
    ///   empty or contains only whitespace.
    /// * [`ValidationError::MultiLineInformalGroundSummary`] when `value`
    ///   contains any line break character (`\n` or `\r`).
    pub fn try_new(value: impl Into<String>) -> Result<Self, ValidationError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(ValidationError::EmptyInformalGroundSummary);
        }
        if value.contains('\n') || value.contains('\r') {
            return Err(ValidationError::MultiLineInformalGroundSummary);
        }
        Ok(Self(value))
    }
}

impl AsRef<str> for InformalGroundSummary {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for InformalGroundSummary {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Structured reference to an unpersisted ground (discussion / feedback /
/// memory / user directive).
///
/// No file path: by definition the ground is not yet persisted. The pair
/// `(kind, summary)` is enough context for a reviewer to assess whether
/// the ground should be promoted to a formal ref before merge.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InformalGroundRef {
    pub kind: InformalGroundKind,
    pub summary: InformalGroundSummary,
}

impl InformalGroundRef {
    pub fn new(kind: InformalGroundKind, summary: InformalGroundSummary) -> Self {
        Self { kind, summary }
    }
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn informal_ground_kind_string_forms() {
        assert_eq!(InformalGroundKind::Discussion.as_str(), "discussion");
        assert_eq!(InformalGroundKind::Feedback.as_str(), "feedback");
        assert_eq!(InformalGroundKind::Memory.as_str(), "memory");
        assert_eq!(InformalGroundKind::UserDirective.as_str(), "user_directive");
    }

    #[test]
    fn informal_ground_summary_accepts_non_empty() {
        let s = InformalGroundSummary::try_new("user asked to defer Q15 anchor semantics").unwrap();
        assert_eq!(s.as_ref(), "user asked to defer Q15 anchor semantics");
    }

    #[test]
    fn informal_ground_summary_rejects_empty() {
        let err = InformalGroundSummary::try_new("").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyInformalGroundSummary));
    }

    #[test]
    fn informal_ground_summary_rejects_whitespace_only() {
        let err = InformalGroundSummary::try_new("   \t  ").unwrap_err();
        assert!(matches!(err, ValidationError::EmptyInformalGroundSummary));
    }

    #[test]
    fn informal_ground_summary_rejects_newline() {
        let err = InformalGroundSummary::try_new("line one\nline two").unwrap_err();
        assert!(matches!(err, ValidationError::MultiLineInformalGroundSummary));
    }

    #[test]
    fn informal_ground_summary_rejects_carriage_return() {
        let err = InformalGroundSummary::try_new("line one\rline two").unwrap_err();
        assert!(matches!(err, ValidationError::MultiLineInformalGroundSummary));
    }

    #[test]
    fn informal_ground_ref_constructs() {
        let summary = InformalGroundSummary::try_new("Q15 deferred per user directive").unwrap();
        let r = InformalGroundRef::new(InformalGroundKind::UserDirective, summary.clone());
        assert_eq!(r.kind, InformalGroundKind::UserDirective);
        assert_eq!(r.summary, summary);
    }
}
