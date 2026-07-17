//! Domain vocabulary for append-only ADR baseline snapshots and freeze checks.
//!
//! This module deliberately models only pure state and validation. Snapshot I/O,
//! ledger persistence, and byte hashing belong to adapters and use cases.

use std::fmt;

use thiserror::Error;

use crate::tddd::test_obligation::ids::{DiagnosticMessage, unavailable_diagnostic_message};
use crate::{ContentHash, NonEmptyString, Timestamp};

/// Validated ADR source filename, constrained to one Markdown filename.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AdrSourceFileName {
    value: String,
}

impl AdrSourceFileName {
    /// Validates a direct child filename of `knowledge/adr/`.
    ///
    /// # Errors
    ///
    /// Returns [`AdrSourceFileNameError::InvalidFileName`] when `value` is not
    /// a non-empty Markdown filename without path separators or traversal.
    pub fn try_new(value: String) -> Result<Self, AdrSourceFileNameError> {
        if is_valid_adr_source_file_name(&value) {
            Ok(Self { value })
        } else {
            Err(AdrSourceFileNameError::InvalidFileName(diagnostic_for(value)))
        }
    }

    /// Borrows the validated filename.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }
}

impl fmt::Display for AdrSourceFileName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.value)
    }
}

/// Validation error returned by [`AdrSourceFileName::try_new`].
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrSourceFileNameError {
    /// The supplied value is not a direct Markdown filename in `knowledge/adr/`.
    #[error("invalid ADR source filename: {0:?}")]
    InvalidFileName(DiagnosticMessage),
}

fn is_valid_adr_source_file_name(value: &str) -> bool {
    !value.is_empty()
        && value.ends_with(".md")
        && value.len() > ".md".len()
        && !value.contains(['/', '\\'])
        && !value.contains("..")
        && !value.chars().any(char::is_control)
}

fn diagnostic_for(value: String) -> DiagnosticMessage {
    DiagnosticMessage::try_new(value).unwrap_or_else(|_| unavailable_diagnostic_message())
}

/// Snapshot operation kinds accepted by the ADR baseline workflow.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum AdrBaselineKind {
    /// The explicitly selected primary ADR at track initialization.
    Init,
    /// An existing ADR newly cited by a later phase.
    Cite,
    /// A track-born ADR promoted after receiving user approval.
    NewAdr,
    /// A diagnoser-approved non-semantic correction.
    NonSemanticFix,
    /// A planned ADR edit justified by a grounding escalation.
    Escalation,
}

impl AdrBaselineKind {
    /// Whether this kind requires a self-contained reason in the ledger.
    #[must_use]
    pub fn requires_reason(&self) -> bool {
        matches!(self, Self::NewAdr | Self::Escalation)
    }

    /// Whether the source bytes must be loaded from the track fork point.
    #[must_use]
    pub fn uses_fork_point(&self) -> bool {
        matches!(self, Self::Cite)
    }
}

/// A typed append-only ledger entry for one ADR baseline snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineLedgerEntry {
    /// Initial primary-ADR snapshot.
    Init { source: AdrSourceFileName, hash: ContentHash, timestamp: Timestamp },
    /// Snapshot of an ADR newly cited from the fork point.
    Cite { source: AdrSourceFileName, hash: ContentHash, timestamp: Timestamp },
    /// Snapshot of a promoted track-born ADR, with its mandatory rationale.
    NewAdr {
        source: AdrSourceFileName,
        hash: ContentHash,
        reason: NonEmptyString,
        timestamp: Timestamp,
    },
    /// Snapshot after a diagnoser-approved non-semantic correction.
    NonSemanticFix { source: AdrSourceFileName, hash: ContentHash, timestamp: Timestamp },
    /// Snapshot after a planned grounding escalation, with its mandatory rationale.
    Escalation {
        source: AdrSourceFileName,
        hash: ContentHash,
        reason: NonEmptyString,
        timestamp: Timestamp,
    },
}

impl AdrBaselineLedgerEntry {
    /// Returns the operation kind represented by this record.
    #[must_use]
    pub fn kind(&self) -> AdrBaselineKind {
        match self {
            Self::Init { .. } => AdrBaselineKind::Init,
            Self::Cite { .. } => AdrBaselineKind::Cite,
            Self::NewAdr { .. } => AdrBaselineKind::NewAdr,
            Self::NonSemanticFix { .. } => AdrBaselineKind::NonSemanticFix,
            Self::Escalation { .. } => AdrBaselineKind::Escalation,
        }
    }

    /// Returns the ADR source filename recorded by this entry.
    #[must_use]
    pub fn source(&self) -> &AdrSourceFileName {
        match self {
            Self::Init { source, .. }
            | Self::Cite { source, .. }
            | Self::NewAdr { source, .. }
            | Self::NonSemanticFix { source, .. }
            | Self::Escalation { source, .. } => source,
        }
    }

    /// Returns the full SHA-256 hash recorded by this entry.
    #[must_use]
    pub fn hash(&self) -> &ContentHash {
        match self {
            Self::Init { hash, .. }
            | Self::Cite { hash, .. }
            | Self::NewAdr { hash, .. }
            | Self::NonSemanticFix { hash, .. }
            | Self::Escalation { hash, .. } => hash,
        }
    }

    /// Returns the snapshot timestamp recorded by this entry.
    #[must_use]
    pub fn timestamp(&self) -> &Timestamp {
        match self {
            Self::Init { timestamp, .. }
            | Self::Cite { timestamp, .. }
            | Self::NewAdr { timestamp, .. }
            | Self::NonSemanticFix { timestamp, .. }
            | Self::Escalation { timestamp, .. } => timestamp,
        }
    }

    /// Returns the required rationale for the kinds that carry one.
    #[must_use]
    pub fn reason(&self) -> Option<&NonEmptyString> {
        match self {
            Self::NewAdr { reason, .. } | Self::Escalation { reason, .. } => Some(reason),
            Self::Init { .. } | Self::Cite { .. } | Self::NonSemanticFix { .. } => None,
        }
    }
}

/// Provenance classification for an ADR source relative to a track fork point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineSourceState {
    /// The ADR existed at the track fork point.
    ExistingAtForkPoint,
    /// The ADR was created in this track and still lacks user approval.
    TrackBornDraft,
    /// The ADR was created in this track and has received user approval.
    TrackBornPromoted,
}

/// Returns whether the recorded snapshot kinds satisfy this source's stamp requirement.
///
/// Existing ADRs require an initial or cited snapshot. Track-born drafts are exempt until
/// promotion, after which an initial or `new-adr` snapshot is required.
#[must_use]
pub fn is_required_stamp_satisfied(
    source_state: &AdrBaselineSourceState,
    recorded_kinds: &[AdrBaselineKind],
) -> bool {
    match source_state {
        AdrBaselineSourceState::ExistingAtForkPoint => recorded_kinds
            .iter()
            .any(|kind| matches!(kind, AdrBaselineKind::Init | AdrBaselineKind::Cite)),
        AdrBaselineSourceState::TrackBornDraft => true,
        AdrBaselineSourceState::TrackBornPromoted => recorded_kinds
            .iter()
            .any(|kind| matches!(kind, AdrBaselineKind::Init | AdrBaselineKind::NewAdr)),
    }
}

/// Verification status for a recorded baseline-copy file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineRecordedCopyStatus {
    /// The copy exists and matches its ledger hash.
    Matches,
    /// The copy expected by the ledger does not exist.
    Missing,
    /// The copy exists but differs from its ledger hash.
    HashMismatch {
        /// Hash calculated from the existing copy.
        actual: ContentHash,
    },
}

/// A typed fail-closed reason reported by the ADR baseline check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineCheckViolation {
    /// The review check could not derive a primary ADR from an init snapshot.
    PrimaryInitUnavailable,
    /// The primary ADR has no required initial snapshot.
    MissingPrimaryInit(AdrSourceFileName),
    /// An ADR that must be protected has no snapshot record.
    MissingRequiredStamp(AdrSourceFileName),
    /// An ADR named by the ledger is absent from the source directory.
    SourceMissing(AdrSourceFileName),
    /// A baseline-copy file referenced by the ledger is absent.
    BaselineCopyMissing { source: AdrSourceFileName, expected: ContentHash },
    /// A baseline-copy file has an unexpected hash.
    BaselineCopyMismatch { source: AdrSourceFileName, expected: ContentHash, actual: ContentHash },
    /// The current ADR bytes differ from the latest ledger hash.
    ByteMismatch { source: AdrSourceFileName, expected: ContentHash, actual: ContentHash },
}

/// Non-empty collection of fail-closed ADR baseline check violations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdrBaselineCheckViolations {
    violations: Vec<AdrBaselineCheckViolation>,
}

impl AdrBaselineCheckViolations {
    /// Creates a non-empty violation collection.
    ///
    /// # Errors
    ///
    /// Returns [`AdrBaselineCheckOutcomeError::EmptyViolations`] when `violations` is empty.
    pub fn try_new(
        violations: Vec<AdrBaselineCheckViolation>,
    ) -> Result<Self, AdrBaselineCheckOutcomeError> {
        if violations.is_empty() {
            Err(AdrBaselineCheckOutcomeError::EmptyViolations)
        } else {
            Ok(Self { violations })
        }
    }

    /// Borrows the violations in their deterministic check order.
    #[must_use]
    pub fn as_slice(&self) -> &[AdrBaselineCheckViolation] {
        &self.violations
    }
}

/// Outcome of an ADR baseline freeze check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineCheckOutcome {
    /// No freeze-check violations were found.
    Passed,
    /// One or more violations block the guarded operation.
    Blocked { violations: AdrBaselineCheckViolations },
}

impl AdrBaselineCheckOutcome {
    /// Constructs a blocked outcome from the non-empty violations found.
    #[must_use]
    pub fn blocked(violations: AdrBaselineCheckViolations) -> Self {
        Self::Blocked { violations }
    }
}

/// Construction error for an invalid baseline check outcome.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrBaselineCheckOutcomeError {
    /// A blocked outcome must contain at least one violation.
    #[error("ADR baseline check violations must not be empty")]
    EmptyViolations,
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    fn source() -> AdrSourceFileName {
        AdrSourceFileName::try_new("2026-07-16-2001-adr-decision-freeze.md".to_owned()).unwrap()
    }

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn timestamp() -> Timestamp {
        Timestamp::new("2026-07-16T11:11:01Z").unwrap()
    }

    #[test]
    fn test_adr_source_file_name_accepts_direct_markdown_file_name() {
        let source = source();
        assert_eq!(source.as_str(), "2026-07-16-2001-adr-decision-freeze.md");
        assert_eq!(source.to_string(), source.as_str());
    }

    #[test]
    fn test_adr_source_file_name_rejects_paths_and_non_markdown_names() {
        for invalid in ["", "../escape.md", "knowledge/adr/a.md", "a.txt", ".md"] {
            assert!(matches!(
                AdrSourceFileName::try_new(invalid.to_owned()),
                Err(AdrSourceFileNameError::InvalidFileName(_))
            ));
        }
    }

    #[test]
    fn test_adr_baseline_kind_reason_and_fork_point_rules() {
        assert!(AdrBaselineKind::NewAdr.requires_reason());
        assert!(AdrBaselineKind::Escalation.requires_reason());
        assert!(!AdrBaselineKind::Init.requires_reason());
        assert!(AdrBaselineKind::Cite.uses_fork_point());
        assert!(!AdrBaselineKind::NonSemanticFix.uses_fork_point());
    }

    #[test]
    fn test_required_stamp_eligibility_applies_state_specific_recorded_kinds() {
        let cases = [
            (AdrBaselineSourceState::ExistingAtForkPoint, vec![], false),
            (AdrBaselineSourceState::ExistingAtForkPoint, vec![AdrBaselineKind::Init], true),
            (AdrBaselineSourceState::ExistingAtForkPoint, vec![AdrBaselineKind::Cite], true),
            (AdrBaselineSourceState::ExistingAtForkPoint, vec![AdrBaselineKind::NewAdr], false),
            (AdrBaselineSourceState::TrackBornDraft, vec![], true),
            (AdrBaselineSourceState::TrackBornDraft, vec![AdrBaselineKind::NonSemanticFix], true),
            (AdrBaselineSourceState::TrackBornPromoted, vec![], false),
            (AdrBaselineSourceState::TrackBornPromoted, vec![AdrBaselineKind::Init], true),
            (AdrBaselineSourceState::TrackBornPromoted, vec![AdrBaselineKind::NewAdr], true),
            (AdrBaselineSourceState::TrackBornPromoted, vec![AdrBaselineKind::Cite], false),
            (
                AdrBaselineSourceState::TrackBornPromoted,
                vec![AdrBaselineKind::NonSemanticFix],
                false,
            ),
        ];

        for (source_state, recorded_kinds, expected) in cases {
            assert_eq!(is_required_stamp_satisfied(&source_state, &recorded_kinds), expected);
        }
    }

    #[test]
    fn test_adr_baseline_ledger_entry_exposes_kind_and_reason_legality() {
        let entry = AdrBaselineLedgerEntry::NewAdr {
            source: source(),
            hash: hash(1),
            reason: NonEmptyString::try_new("user approval recorded".to_owned()).unwrap(),
            timestamp: timestamp(),
        };

        assert_eq!(entry.kind(), AdrBaselineKind::NewAdr);
        assert_eq!(entry.source(), &source());
        assert_eq!(entry.hash(), &hash(1));
        assert_eq!(entry.timestamp(), &timestamp());
        assert_eq!(entry.reason().map(NonEmptyString::as_ref), Some("user approval recorded"));

        let init = AdrBaselineLedgerEntry::Init {
            source: source(),
            hash: hash(2),
            timestamp: timestamp(),
        };
        assert_eq!(init.kind(), AdrBaselineKind::Init);
        assert_eq!(init.reason(), None);
    }

    #[test]
    fn test_adr_baseline_check_violations_reject_empty_collection() {
        assert_eq!(
            AdrBaselineCheckViolations::try_new(Vec::new()),
            Err(AdrBaselineCheckOutcomeError::EmptyViolations)
        );
    }

    #[test]
    fn test_adr_baseline_check_outcome_aggregates_fail_closed_violations() {
        let violations = AdrBaselineCheckViolations::try_new(vec![
            AdrBaselineCheckViolation::MissingPrimaryInit(source()),
            AdrBaselineCheckViolation::ByteMismatch {
                source: source(),
                expected: hash(3),
                actual: hash(4),
            },
        ])
        .unwrap();

        let outcome = AdrBaselineCheckOutcome::blocked(violations);
        assert!(matches!(
            outcome,
            AdrBaselineCheckOutcome::Blocked { violations } if violations.as_slice().len() == 2
        ));
    }
}
