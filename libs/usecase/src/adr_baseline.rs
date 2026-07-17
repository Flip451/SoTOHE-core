//! Application services and secondary ports for ADR baseline snapshots.

use std::collections::BTreeMap;
use std::sync::Arc;

pub use domain::adr_baseline::{AdrBaselineCheckOutcome, AdrBaselineKind, AdrSourceFileName};
use domain::adr_baseline::{
    AdrBaselineCheckViolation, AdrBaselineCheckViolations, AdrBaselineLedgerEntry,
    AdrBaselineRecordedCopyStatus, AdrBaselineSourceState, AdrSourceFileNameError,
    is_required_stamp_satisfied,
};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::{ContentHash, NonEmptyString, Timestamp, TrackId};
use sha2::{Digest as _, Sha256};
use thiserror::Error;

/// A write operation requested at the ADR baseline boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineCommand {
    /// Record a new append-only snapshot.
    Snapshot {
        track_id: TrackId,
        source: AdrSourceFileName,
        kind: AdrBaselineKind,
        reason: Option<NonEmptyString>,
        timestamp: Timestamp,
    },
    /// Restore the latest recorded snapshot to the ADR source file.
    Restore { track_id: TrackId, source: AdrSourceFileName },
}

/// Output returned after a successful write operation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineOutput {
    /// A ledger entry was durably recorded.
    SnapshotRecorded(AdrBaselineLedgerEntry),
    /// The requested ADR source was restored.
    Restored(AdrSourceFileName),
}

/// Freeze-check query requested at the ADR baseline boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineQuery {
    /// Verify the primary init snapshot before a review cycle.
    CheckReview { track_id: TrackId, primary_source: Option<AdrSourceFileName> },
    /// Verify every recorded and currently-required ADR before commit.
    CheckCommit { track_id: TrackId },
}

/// Output returned by a freeze check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AdrBaselineQueryOutput {
    /// The domain outcome of the check.
    Checked(AdrBaselineCheckOutcome),
}

/// Persistence failure from the mutation port.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrBaselineStoreError {
    /// A required persistence read failed.
    #[error("ADR baseline store read failed: {0:?}")]
    Read(DiagnosticMessage),
    /// A required persistence write failed.
    #[error("ADR baseline store write failed: {0:?}")]
    Write(DiagnosticMessage),
}

/// Persistence failure from the read-only port.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrBaselineStoreReadError {
    /// A required persistence read failed.
    #[error("ADR baseline store read failed: {0:?}")]
    Read(DiagnosticMessage),
}

/// Failure from the ADR source reader.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrBaselineSourceError {
    /// The source could not be read.
    #[error("ADR baseline source read failed: {0:?}")]
    Read(DiagnosticMessage),
    /// The named source is absent from the requested view.
    #[error("ADR baseline source unavailable: {0}")]
    Unavailable(AdrSourceFileName),
}

/// Invalid command data rejected by the command interactor.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrBaselineValidationError {
    /// The reason is present for a kind that forbids it or absent for one that requires it.
    #[error("ADR baseline reason is incompatible with the snapshot kind")]
    InvalidReason,
    /// A source filename failed domain validation at the outer boundary.
    #[error("invalid ADR baseline source filename: {0}")]
    InvalidSourceFileName(AdrSourceFileNameError),
}

/// Failure envelope for snapshot and restore commands.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrBaselineError {
    /// The persistence adapter failed.
    #[error(transparent)]
    Store(AdrBaselineStoreError),
    /// The source adapter failed.
    #[error(transparent)]
    Source(AdrBaselineSourceError),
    /// Command validation failed before any port was invoked.
    #[error(transparent)]
    Validation(AdrBaselineValidationError),
}

/// Operational failure for a read-only freeze check.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AdrBaselineQueryError {
    /// Source I/O failed; source absence is represented as a blocked outcome instead.
    #[error("ADR baseline query source read failed: {0:?}")]
    SourceRead(DiagnosticMessage),
    /// The read-only ledger adapter failed.
    #[error(transparent)]
    Store(AdrBaselineStoreReadError),
}

/// Secondary port for loading verbatim ADR bytes and track-relative source state.
pub trait AdrBaselineSourcePort: Send + Sync {
    /// Reads current working-tree bytes for one validated ADR source.
    fn working_bytes(&self, source: &AdrSourceFileName) -> Result<Vec<u8>, AdrBaselineSourceError>;
    /// Reads the source bytes at the track fork point.
    fn fork_point_bytes(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
    ) -> Result<Vec<u8>, AdrBaselineSourceError>;
    /// Lists the ADR source filenames cited by the current track specification.
    fn cited_sources(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<AdrSourceFileName>, AdrBaselineSourceError>;
    /// Classifies an ADR relative to a track fork point and promotion state.
    fn source_state(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
    ) -> Result<AdrBaselineSourceState, AdrBaselineSourceError>;
}

/// Secondary mutation port for append-only baseline recording and restoration.
pub trait AdrBaselineStorePort: Send + Sync {
    /// Atomically appends a snapshot record, adding a copy only when required.
    fn snapshot(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
        bytes: Vec<u8>,
        kind: AdrBaselineKind,
        reason: Option<NonEmptyString>,
        timestamp: Timestamp,
    ) -> Result<AdrBaselineLedgerEntry, AdrBaselineStoreError>;
    /// Restores the latest snapshot for a source.
    fn restore(
        &self,
        track_id: &TrackId,
        source: &AdrSourceFileName,
    ) -> Result<(), AdrBaselineStoreError>;
}

/// Secondary read-only port for ledger and recorded-copy verification.
pub trait AdrBaselineStoreReadPort: Send + Sync {
    /// Returns ledger entries in their append order.
    fn read_entries(
        &self,
        track_id: &TrackId,
    ) -> Result<Vec<AdrBaselineLedgerEntry>, AdrBaselineStoreReadError>;
    /// Verifies one copy named by a ledger entry.
    fn verify_recorded_copy(
        &self,
        track_id: &TrackId,
        entry: &AdrBaselineLedgerEntry,
    ) -> Result<AdrBaselineRecordedCopyStatus, AdrBaselineStoreReadError>;
}

/// Primary command service for ADR baseline snapshots and restores.
pub trait AdrBaselineService: Send + Sync {
    /// Executes a snapshot or restore command.
    fn execute(&self, command: AdrBaselineCommand) -> Result<AdrBaselineOutput, AdrBaselineError>;
}

/// Primary query service for ADR baseline freeze checks.
pub trait AdrBaselineQueryService: Send + Sync {
    /// Executes a review or commit freeze check.
    fn execute(
        &self,
        query: AdrBaselineQuery,
    ) -> Result<AdrBaselineQueryOutput, AdrBaselineQueryError>;
}

/// Command interactor; the only owner of kind/reason compatibility validation.
pub struct AdrBaselineInteractor {
    store: Arc<dyn AdrBaselineStorePort>,
    source: Arc<dyn AdrBaselineSourcePort>,
}

impl AdrBaselineInteractor {
    /// Creates the command interactor from its separately-owned secondary ports.
    #[must_use]
    pub fn new(
        store: Arc<dyn AdrBaselineStorePort>,
        source: Arc<dyn AdrBaselineSourcePort>,
    ) -> Self {
        Self { store, source }
    }
}

impl AdrBaselineService for AdrBaselineInteractor {
    fn execute(&self, command: AdrBaselineCommand) -> Result<AdrBaselineOutput, AdrBaselineError> {
        match command {
            AdrBaselineCommand::Snapshot { track_id, source, kind, reason, timestamp } => {
                if kind.requires_reason() != reason.is_some() {
                    return Err(AdrBaselineError::Validation(
                        AdrBaselineValidationError::InvalidReason,
                    ));
                }
                let bytes = if kind.uses_fork_point() {
                    self.source.fork_point_bytes(&track_id, &source)
                } else {
                    self.source.working_bytes(&source)
                }
                .map_err(AdrBaselineError::Source)?;
                let entry = self
                    .store
                    .snapshot(&track_id, &source, bytes, kind, reason, timestamp)
                    .map_err(AdrBaselineError::Store)?;
                Ok(AdrBaselineOutput::SnapshotRecorded(entry))
            }
            AdrBaselineCommand::Restore { track_id, source } => {
                self.store.restore(&track_id, &source).map_err(AdrBaselineError::Store)?;
                Ok(AdrBaselineOutput::Restored(source))
            }
        }
    }
}

/// Query interactor and owner of recorded-copy and byte verification.
pub struct AdrBaselineQueryInteractor {
    store: Arc<dyn AdrBaselineStoreReadPort>,
    source: Arc<dyn AdrBaselineSourcePort>,
}

impl AdrBaselineQueryInteractor {
    /// Creates the query interactor from read-only storage and source ports.
    #[must_use]
    pub fn new(
        store: Arc<dyn AdrBaselineStoreReadPort>,
        source: Arc<dyn AdrBaselineSourcePort>,
    ) -> Self {
        Self { store, source }
    }

    fn check(
        &self,
        track_id: &TrackId,
        primary: Option<&AdrSourceFileName>,
        commit_gate: bool,
    ) -> Result<AdrBaselineQueryOutput, AdrBaselineQueryError> {
        let entries = self.store.read_entries(track_id).map_err(AdrBaselineQueryError::Store)?;
        let mut violations = Vec::new();
        let mut latest = BTreeMap::<AdrSourceFileName, AdrBaselineLedgerEntry>::new();
        let mut recorded_kinds = BTreeMap::<AdrSourceFileName, Vec<AdrBaselineKind>>::new();

        for entry in &entries {
            recorded_kinds.entry(entry.source().clone()).or_default().push(entry.kind());
            latest.insert(entry.source().clone(), entry.clone());
            match self
                .store
                .verify_recorded_copy(track_id, entry)
                .map_err(AdrBaselineQueryError::Store)?
            {
                AdrBaselineRecordedCopyStatus::Matches => {}
                AdrBaselineRecordedCopyStatus::Missing => {
                    violations.push(AdrBaselineCheckViolation::BaselineCopyMissing {
                        source: entry.source().clone(),
                        expected: entry.hash().clone(),
                    })
                }
                AdrBaselineRecordedCopyStatus::HashMismatch { actual } => {
                    violations.push(AdrBaselineCheckViolation::BaselineCopyMismatch {
                        source: entry.source().clone(),
                        expected: entry.hash().clone(),
                        actual,
                    })
                }
            }
        }

        match primary {
            Some(primary_source) => {
                let has_init = entries.iter().any(|entry| {
                    entry.source() == primary_source && entry.kind() == AdrBaselineKind::Init
                });
                if !has_init {
                    violations.push(AdrBaselineCheckViolation::MissingPrimaryInit(
                        primary_source.clone(),
                    ));
                }
            }
            None if !commit_gate
                && !entries.iter().any(|entry| entry.kind() == AdrBaselineKind::Init) =>
            {
                violations.push(AdrBaselineCheckViolation::PrimaryInitUnavailable);
            }
            None => {}
        }

        if commit_gate {
            let cited = self.source.cited_sources(track_id).map_err(source_read_error)?;
            for cited_source in cited {
                let source_state =
                    self.source.source_state(track_id, &cited_source).map_err(source_read_error)?;
                let source_recorded_kinds =
                    recorded_kinds.get(&cited_source).map_or(&[][..], Vec::as_slice);
                if !is_required_stamp_satisfied(&source_state, source_recorded_kinds) {
                    violations.push(AdrBaselineCheckViolation::MissingRequiredStamp(cited_source));
                }
            }
        }

        if commit_gate {
            for (source, entry) in latest {
                let bytes = match self.source.working_bytes(&source) {
                    Ok(bytes) => bytes,
                    Err(AdrBaselineSourceError::Unavailable(_)) => {
                        violations.push(AdrBaselineCheckViolation::SourceMissing(source));
                        continue;
                    }
                    Err(AdrBaselineSourceError::Read(message)) => {
                        return Err(AdrBaselineQueryError::SourceRead(message));
                    }
                };
                let actual = content_hash(&bytes);
                if actual != *entry.hash() {
                    violations.push(AdrBaselineCheckViolation::ByteMismatch {
                        source,
                        expected: entry.hash().clone(),
                        actual,
                    });
                }
            }
        }

        let outcome = match AdrBaselineCheckViolations::try_new(violations) {
            Ok(violations) => AdrBaselineCheckOutcome::blocked(violations),
            Err(_) => AdrBaselineCheckOutcome::Passed,
        };
        Ok(AdrBaselineQueryOutput::Checked(outcome))
    }
}

impl AdrBaselineQueryService for AdrBaselineQueryInteractor {
    fn execute(
        &self,
        query: AdrBaselineQuery,
    ) -> Result<AdrBaselineQueryOutput, AdrBaselineQueryError> {
        match query {
            AdrBaselineQuery::CheckReview { track_id, primary_source } => {
                self.check(&track_id, primary_source.as_ref(), false)
            }
            AdrBaselineQuery::CheckCommit { track_id } => self.check(&track_id, None, true),
        }
    }
}

fn source_read_error(error: AdrBaselineSourceError) -> AdrBaselineQueryError {
    match error {
        AdrBaselineSourceError::Read(message) => AdrBaselineQueryError::SourceRead(message),
        AdrBaselineSourceError::Unavailable(source) => {
            AdrBaselineQueryError::SourceRead(diagnostic(&format!("source unavailable: {source}")))
        }
    }
}

fn content_hash(bytes: &[u8]) -> ContentHash {
    ContentHash::from_bytes(Sha256::digest(bytes).into())
}

fn diagnostic(message: &str) -> DiagnosticMessage {
    let mut text = if message.trim().is_empty() {
        "ADR baseline operation failed".to_owned()
    } else {
        message.to_owned()
    };
    loop {
        match DiagnosticMessage::try_new(text) {
            Ok(message) => return message,
            Err(_) => text = "ADR baseline operation failed".to_owned(),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    fn source() -> AdrSourceFileName {
        AdrSourceFileName::try_new("decision.md".to_owned()).unwrap()
    }
    fn track() -> TrackId {
        TrackId::try_new("adr-baseline-test".to_owned()).unwrap()
    }
    fn hash(bytes: &[u8]) -> ContentHash {
        content_hash(bytes)
    }
    fn entry(kind: AdrBaselineKind) -> AdrBaselineLedgerEntry {
        let timestamp = domain::Timestamp::new("2026-07-16T00:00:00Z").unwrap();
        match kind {
            AdrBaselineKind::Init => {
                AdrBaselineLedgerEntry::Init { source: source(), hash: hash(b"current"), timestamp }
            }
            AdrBaselineKind::Cite => {
                AdrBaselineLedgerEntry::Cite { source: source(), hash: hash(b"current"), timestamp }
            }
            AdrBaselineKind::NewAdr => AdrBaselineLedgerEntry::NewAdr {
                source: source(),
                hash: hash(b"current"),
                reason: NonEmptyString::try_new("reason".to_owned()).unwrap(),
                timestamp,
            },
            AdrBaselineKind::NonSemanticFix => AdrBaselineLedgerEntry::NonSemanticFix {
                source: source(),
                hash: hash(b"current"),
                timestamp,
            },
            AdrBaselineKind::Escalation => AdrBaselineLedgerEntry::Escalation {
                source: source(),
                hash: hash(b"current"),
                reason: NonEmptyString::try_new("reason".to_owned()).unwrap(),
                timestamp,
            },
        }
    }

    struct Source {
        working: Vec<u8>,
        fork: Vec<u8>,
        cited: Vec<AdrSourceFileName>,
        state: AdrBaselineSourceState,
    }
    impl AdrBaselineSourcePort for Source {
        fn working_bytes(&self, _: &AdrSourceFileName) -> Result<Vec<u8>, AdrBaselineSourceError> {
            Ok(self.working.clone())
        }
        fn fork_point_bytes(
            &self,
            _: &TrackId,
            _: &AdrSourceFileName,
        ) -> Result<Vec<u8>, AdrBaselineSourceError> {
            Ok(self.fork.clone())
        }
        fn cited_sources(
            &self,
            _: &TrackId,
        ) -> Result<Vec<AdrSourceFileName>, AdrBaselineSourceError> {
            Ok(self.cited.clone())
        }
        fn source_state(
            &self,
            _: &TrackId,
            _: &AdrSourceFileName,
        ) -> Result<AdrBaselineSourceState, AdrBaselineSourceError> {
            Ok(self.state.clone())
        }
    }
    struct Store {
        entries: Mutex<Vec<AdrBaselineLedgerEntry>>,
    }
    impl AdrBaselineStorePort for Store {
        fn snapshot(
            &self,
            _: &TrackId,
            _: &AdrSourceFileName,
            bytes: Vec<u8>,
            kind: AdrBaselineKind,
            reason: Option<NonEmptyString>,
            timestamp: Timestamp,
        ) -> Result<AdrBaselineLedgerEntry, AdrBaselineStoreError> {
            let saved = match (kind, reason) {
                (AdrBaselineKind::Init, None) => {
                    AdrBaselineLedgerEntry::Init { source: source(), hash: hash(&bytes), timestamp }
                }
                (AdrBaselineKind::Cite, None) => {
                    AdrBaselineLedgerEntry::Cite { source: source(), hash: hash(&bytes), timestamp }
                }
                (AdrBaselineKind::NewAdr, Some(reason)) => AdrBaselineLedgerEntry::NewAdr {
                    source: source(),
                    hash: hash(&bytes),
                    reason,
                    timestamp,
                },
                (AdrBaselineKind::NonSemanticFix, None) => AdrBaselineLedgerEntry::NonSemanticFix {
                    source: source(),
                    hash: hash(&bytes),
                    timestamp,
                },
                (AdrBaselineKind::Escalation, Some(reason)) => AdrBaselineLedgerEntry::Escalation {
                    source: source(),
                    hash: hash(&bytes),
                    reason,
                    timestamp,
                },
                _ => return Err(AdrBaselineStoreError::Write(diagnostic("unexpected test input"))),
            };
            self.entries.lock().unwrap().push(saved.clone());
            Ok(saved)
        }
        fn restore(&self, _: &TrackId, _: &AdrSourceFileName) -> Result<(), AdrBaselineStoreError> {
            Ok(())
        }
    }
    impl AdrBaselineStoreReadPort for Store {
        fn read_entries(
            &self,
            _: &TrackId,
        ) -> Result<Vec<AdrBaselineLedgerEntry>, AdrBaselineStoreReadError> {
            Ok(self.entries.lock().unwrap().clone())
        }
        fn verify_recorded_copy(
            &self,
            _: &TrackId,
            _: &AdrBaselineLedgerEntry,
        ) -> Result<AdrBaselineRecordedCopyStatus, AdrBaselineStoreReadError> {
            Ok(AdrBaselineRecordedCopyStatus::Matches)
        }
    }

    struct MissingCopyStore;

    impl AdrBaselineStoreReadPort for MissingCopyStore {
        fn read_entries(
            &self,
            _: &TrackId,
        ) -> Result<Vec<AdrBaselineLedgerEntry>, AdrBaselineStoreReadError> {
            Ok(vec![entry(AdrBaselineKind::Init)])
        }

        fn verify_recorded_copy(
            &self,
            _: &TrackId,
            _: &AdrBaselineLedgerEntry,
        ) -> Result<AdrBaselineRecordedCopyStatus, AdrBaselineStoreReadError> {
            Ok(AdrBaselineRecordedCopyStatus::Missing)
        }
    }

    #[test]
    fn test_adr_baseline_snapshot_rejects_reason_for_init() {
        let store = Arc::new(Store { entries: Mutex::new(Vec::new()) });
        let interactor = AdrBaselineInteractor::new(
            store,
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: b"fork".to_vec(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );
        let result = interactor.execute(AdrBaselineCommand::Snapshot {
            track_id: track(),
            source: source(),
            kind: AdrBaselineKind::Init,
            reason: Some(NonEmptyString::try_new("no".to_owned()).unwrap()),
            timestamp: Timestamp::new("2026-07-16T00:00:00Z").unwrap(),
        });
        assert_eq!(
            result,
            Err(AdrBaselineError::Validation(AdrBaselineValidationError::InvalidReason))
        );
    }

    #[test]
    fn test_adr_baseline_snapshot_reads_fork_point_for_cite() {
        let store = Arc::new(Store { entries: Mutex::new(Vec::new()) });
        let timestamp = Timestamp::new("2026-07-16T00:00:00Z").unwrap();
        let interactor = AdrBaselineInteractor::new(
            store,
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: b"fork".to_vec(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );
        let result = interactor
            .execute(AdrBaselineCommand::Snapshot {
                track_id: track(),
                source: source(),
                kind: AdrBaselineKind::Cite,
                reason: None,
                timestamp: timestamp.clone(),
            })
            .unwrap();
        assert!(
            matches!(result, AdrBaselineOutput::SnapshotRecorded(saved) if saved.hash() == &hash(b"fork") && saved.timestamp() == &timestamp)
        );
    }

    #[test]
    fn test_adr_baseline_query_blocks_missing_primary_init() {
        let store = Arc::new(Store { entries: Mutex::new(vec![entry(AdrBaselineKind::Cite)]) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: Vec::new(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );
        let result = interactor
            .execute(AdrBaselineQuery::CheckReview {
                track_id: track(),
                primary_source: Some(source()),
            })
            .unwrap();
        assert!(
            matches!(result, AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Blocked { violations }) if matches!(violations.as_slice(), [AdrBaselineCheckViolation::MissingPrimaryInit(_)]))
        );
    }

    #[test]
    fn test_adr_baseline_query_passes_derived_review_check_with_init_record() {
        let store = Arc::new(Store { entries: Mutex::new(vec![entry(AdrBaselineKind::Init)]) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: Vec::new(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );

        assert_eq!(
            interactor
                .execute(AdrBaselineQuery::CheckReview { track_id: track(), primary_source: None })
                .unwrap(),
            AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Passed)
        );
    }

    #[test]
    fn test_adr_baseline_query_passes_explicit_review_check_with_matching_init_record() {
        let store = Arc::new(Store { entries: Mutex::new(vec![entry(AdrBaselineKind::Init)]) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: Vec::new(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );

        assert_eq!(
            interactor
                .execute(AdrBaselineQuery::CheckReview {
                    track_id: track(),
                    primary_source: Some(source()),
                })
                .unwrap(),
            AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Passed)
        );
    }

    #[test]
    fn test_adr_baseline_query_passes_review_check_with_draft_bytes() {
        let store = Arc::new(Store { entries: Mutex::new(vec![entry(AdrBaselineKind::Init)]) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: b"draft".to_vec(),
                fork: Vec::new(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );

        assert_eq!(
            interactor
                .execute(AdrBaselineQuery::CheckReview { track_id: track(), primary_source: None })
                .unwrap(),
            AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Passed)
        );
    }

    #[test]
    fn test_adr_baseline_query_blocks_review_check_when_init_copy_is_missing() {
        let interactor = AdrBaselineQueryInteractor::new(
            Arc::new(MissingCopyStore),
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: Vec::new(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );

        let result = interactor
            .execute(AdrBaselineQuery::CheckReview { track_id: track(), primary_source: None })
            .unwrap();

        assert!(
            matches!(result, AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Blocked { violations }) if matches!(violations.as_slice(), [AdrBaselineCheckViolation::BaselineCopyMissing { .. }]))
        );
    }

    #[test]
    fn test_adr_baseline_query_blocks_derived_review_check_without_init_record() {
        let store = Arc::new(Store { entries: Mutex::new(Vec::new()) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: Vec::new(),
                fork: Vec::new(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );

        let result = interactor
            .execute(AdrBaselineQuery::CheckReview { track_id: track(), primary_source: None })
            .unwrap();

        assert!(
            matches!(result, AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Blocked { violations }) if matches!(violations.as_slice(), [AdrBaselineCheckViolation::PrimaryInitUnavailable]))
        );
    }

    #[test]
    fn test_adr_baseline_query_emits_missing_required_stamp_from_domain_policy() {
        let store = Arc::new(Store { entries: Mutex::new(vec![entry(AdrBaselineKind::Cite)]) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: Vec::new(),
                cited: vec![source()],
                state: AdrBaselineSourceState::TrackBornPromoted,
            }),
        );

        let result =
            interactor.execute(AdrBaselineQuery::CheckCommit { track_id: track() }).unwrap();

        assert!(
            matches!(result, AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Blocked { violations }) if matches!(violations.as_slice(), [AdrBaselineCheckViolation::MissingRequiredStamp(missing)] if missing == &source()))
        );
    }

    #[test]
    fn test_adr_baseline_query_omits_violation_when_domain_policy_is_satisfied() {
        let store = Arc::new(Store { entries: Mutex::new(vec![entry(AdrBaselineKind::Cite)]) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: b"current".to_vec(),
                fork: Vec::new(),
                cited: vec![source()],
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );

        assert_eq!(
            interactor.execute(AdrBaselineQuery::CheckCommit { track_id: track() }).unwrap(),
            AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Passed)
        );
    }

    #[test]
    fn test_adr_baseline_query_blocks_commit_check_when_bytes_differ() {
        let store = Arc::new(Store { entries: Mutex::new(vec![entry(AdrBaselineKind::Init)]) });
        let interactor = AdrBaselineQueryInteractor::new(
            store,
            Arc::new(Source {
                working: b"draft".to_vec(),
                fork: Vec::new(),
                cited: Vec::new(),
                state: AdrBaselineSourceState::ExistingAtForkPoint,
            }),
        );

        let result =
            interactor.execute(AdrBaselineQuery::CheckCommit { track_id: track() }).unwrap();

        assert!(
            matches!(result, AdrBaselineQueryOutput::Checked(AdrBaselineCheckOutcome::Blocked { violations }) if matches!(violations.as_slice(), [AdrBaselineCheckViolation::ByteMismatch { .. }]))
        );
    }
}
