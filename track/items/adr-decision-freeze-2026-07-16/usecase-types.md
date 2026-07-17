<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCommand | enum | add | Snapshot, Restore | 🔵 | 🔵 |
| AdrBaselineOutput | enum | add | SnapshotRecorded, Restored | 🔵 | 🔵 |
| AdrBaselineQuery | enum | add | CheckReview, CheckCommit | 🔵 | 🔵 |
| AdrBaselineQueryOutput | enum | add | Checked | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineError | error_type | add | Store, Source, Validation | 🔵 | 🔵 |
| AdrBaselineQueryError | error_type | add | SourceRead, Store | 🔵 | 🔵 |
| AdrBaselineSourceError | error_type | add | Read, Unavailable | 🔵 | 🔵 |
| AdrBaselineStoreError | error_type | add | Read, Write | 🔵 | 🔵 |
| AdrBaselineStoreReadError | error_type | add | Read | 🔵 | 🔵 |
| AdrBaselineValidationError | error_type | add | InvalidReason, InvalidSourceFileName | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineSourcePort | secondary_port | add | fn working_bytes(&self, source: &domain::adr_baseline::AdrSourceFileName) -> Result<Vec<u8>, AdrBaselineSourceError>, fn fork_point_bytes(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName) -> Result<Vec<u8>, AdrBaselineSourceError>, fn cited_sources(&self, track_id: &domain::TrackId) -> Result<Vec<domain::adr_baseline::AdrSourceFileName>, AdrBaselineSourceError>, fn source_state(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName) -> Result<domain::adr_baseline::AdrBaselineSourceState, AdrBaselineSourceError> | 🔵 | 🔵 |
| AdrBaselineStorePort | secondary_port | add | fn snapshot(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName, bytes: Vec<u8>, kind: domain::adr_baseline::AdrBaselineKind, reason: Option<domain::NonEmptyString>, timestamp: domain::Timestamp) -> Result<domain::adr_baseline::AdrBaselineLedgerEntry, AdrBaselineStoreError>, fn restore(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName) -> Result<(), AdrBaselineStoreError> | 🔵 | 🔵 |
| AdrBaselineStoreReadPort | secondary_port | add | fn read_entries(&self, track_id: &domain::TrackId) -> Result<Vec<domain::adr_baseline::AdrBaselineLedgerEntry>, AdrBaselineStoreReadError>, fn verify_recorded_copy(&self, track_id: &domain::TrackId, entry: &domain::adr_baseline::AdrBaselineLedgerEntry) -> Result<domain::adr_baseline::AdrBaselineRecordedCopyStatus, AdrBaselineStoreReadError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineQueryService | application_service | add | fn execute(&self, query: AdrBaselineQuery) -> Result<AdrBaselineQueryOutput, AdrBaselineQueryError> | 🔵 | 🔵 |
| AdrBaselineService | application_service | add | fn execute(&self, command: AdrBaselineCommand) -> Result<AdrBaselineOutput, AdrBaselineError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineInteractor | interactor | add | — | 🔵 | 🔵 |
| AdrBaselineQueryInteractor | interactor | add | — | 🔵 | 🔵 |

