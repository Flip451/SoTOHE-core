<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCommand | enum | modify | Snapshot, Restore | 🟡 | 🔵 |
| LintRuleKind | enum | modify | FieldEmpty, FieldNonEmpty, KindLayerConstraint, ReferencedRoleConstraint, TraitImplRequired, NoRoleInMethodSignature, MethodReferenceSignature, AccessorSignatureRequired, FieldElementUniqueAcrossEntries, NoExternalReferenceInMethods, NoPublicField, ForbiddenMethodReceiver, ForbidPrimitiveInTypes, CompositionRootPureDi | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineError | error_type | modify | Store, Source, Clock | 🟡 | 🔵 |
| AdrBaselineTimestampError | error_type | reference | InvalidTimestamp | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineStorePort | secondary_port | reference | fn snapshot(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName, bytes: std::vec::Vec<u8>, kind: AdrBaselineSnapshotKind, timestamp: domain::Timestamp) -> core::result::Result<domain::adr_baseline::AdrBaselineLedgerEntry, AdrBaselineStoreError>, fn restore(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName) -> core::result::Result<(), AdrBaselineStoreError> | 🔵 | 🔵 |
| ClockPort | secondary_port | add | fn now(&self) -> core::result::Result<domain::Timestamp, AdrBaselineTimestampError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineService | application_service | reference | fn execute(&self, command: AdrBaselineCommand) -> core::result::Result<AdrBaselineOutput, AdrBaselineError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineInteractor | interactor | modify | — | 🟡 | 🔵 |

