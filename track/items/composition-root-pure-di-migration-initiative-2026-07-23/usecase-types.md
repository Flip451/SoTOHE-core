<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineCommand | enum | modify | Snapshot, Restore | 🔵 | 🔵 |
| AdrBaselineSnapshotKind | enum | add | Init, Cite, NewAdr, NonSemanticFix, Escalation | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineError | error_type | modify | Store, Source | 🔵 | 🔵 |
| AdrBaselineTimestampError | error_type | add | InvalidTimestamp | 🔵 | 🔵 |
| AdrBaselineValidationError | error_type | reference | InvalidReason, InvalidSourceFileName | 🔵 | 🔵 |
| CatalogError | error_type | modify | FileExists, FileMissing, DuplicateEntry, AnchorNotFound, InvalidRole, ParseFragment, SchemaInvalid, Port | 🔵 | 🔵 |
| ImplCatalogSignalReadError | error_type | add | ReadFailed | 🔵 | 🔵 |
| ImplPlanReadError | error_type | add | ReadFailed | 🔵 | 🔵 |
| PreReviewGateError | error_type | modify | TaskContractNotFound, TaskContractReadFailed, SignalReadFailed, ImplPlanReadFailed | 🔵 | 🔵 |
| TaskContractReadError | error_type | add | NotFound, ReadFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineSourcePort | secondary_port | reference | fn working_bytes(&self, source: &domain::adr_baseline::AdrSourceFileName) -> core::result::Result<std::vec::Vec<u8>, AdrBaselineSourceError>, fn fork_point_bytes(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName) -> core::result::Result<std::vec::Vec<u8>, AdrBaselineSourceError>, fn cited_sources(&self, track_id: &domain::TrackId) -> core::result::Result<std::vec::Vec<domain::adr_baseline::AdrSourceFileName>, AdrBaselineSourceError>, fn source_state(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName) -> core::result::Result<domain::adr_baseline::AdrBaselineSourceState, AdrBaselineSourceError> | 🔵 | 🔵 |
| AdrBaselineStorePort | secondary_port | modify | fn snapshot(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName, bytes: std::vec::Vec<u8>, kind: AdrBaselineSnapshotKind, timestamp: domain::Timestamp) -> core::result::Result<domain::adr_baseline::AdrBaselineLedgerEntry, AdrBaselineStoreError>, fn restore(&self, track_id: &domain::TrackId, source: &domain::adr_baseline::AdrSourceFileName) -> core::result::Result<(), AdrBaselineStoreError> | 🔵 | 🔵 |
| AdrBaselineStoreReadPort | secondary_port | reference | fn read_entries(&self, track_id: &domain::TrackId) -> core::result::Result<std::vec::Vec<domain::adr_baseline::AdrBaselineLedgerEntry>, AdrBaselineStoreReadError>, fn verify_recorded_copy(&self, track_id: &domain::TrackId, entry: &domain::adr_baseline::AdrBaselineLedgerEntry) -> core::result::Result<domain::adr_baseline::AdrBaselineRecordedCopyStatus, AdrBaselineStoreReadError> | 🔵 | 🔵 |
| CatalogPort | secondary_port | modify | fn init(&self, track_id: &domain::TrackId, items_dir: &std::path::Path) -> core::result::Result<CatalogInitReport, CatalogError>, fn add(&self, track_id: &domain::TrackId, items_dir: &std::path::Path, command: CatalogAddCommand) -> core::result::Result<CatalogWriteReport, CatalogError>, fn import(&self, track_id: &domain::TrackId, items_dir: &std::path::Path, command: CatalogImportCommand) -> core::result::Result<CatalogWriteReport, CatalogError>, fn cite(&self, track_id: &domain::TrackId, items_dir: &std::path::Path, command: CatalogCiteCommand) -> core::result::Result<CatalogWriteReport, CatalogError>, fn check(&self, track_id: &domain::TrackId, items_dir: &std::path::Path, query: CatalogCheckQuery) -> core::result::Result<CatalogCheckReport, CatalogError> | 🔵 | 🔵 |
| ImplCatalogSignalReaderPort | secondary_port | modify | fn read_signals(&self, track_id: &domain::TrackId, layer: &domain::tddd::LayerId) -> core::result::Result<domain::TypeSignalsDocument, ImplCatalogSignalReadError>, fn read_optional_signals(&self, track_id: &domain::TrackId, layer: &domain::tddd::LayerId) -> core::result::Result<core::option::Option<domain::TypeSignalsDocument>, ImplCatalogSignalReadError> | 🔵 | 🔵 |
| ImplPlanReaderPort | secondary_port | modify | fn read_task_statuses(&self, track_id: &domain::TrackId) -> core::result::Result<std::collections::HashMap<domain::TaskId, domain::TaskStatusKind>, ImplPlanReadError> | 🔵 | 🔵 |
| SelfBinaryTransplantPort | secondary_port | reference | fn transplant(&self, destination: &std::path::Path) -> core::result::Result<(), SelfBinaryTransplantError> | 🔵 | 🔵 |
| TaskContractReaderPort | secondary_port | modify | fn read(&self, track_id: &domain::TrackId) -> core::result::Result<domain::task_contract::TaskContractDocument, TaskContractReadError> | 🔵 | 🔵 |
| TemplateBoundaryManifestPort | secondary_port | reference | fn read(&self, manifest_path: &std::path::Path) -> core::result::Result<domain::template_export::TemplateBoundaryManifest, TemplateBoundaryManifestReadError> | 🔵 | 🔵 |
| TemplateExportPort | secondary_port | reference | fn export(&self, command: &TemplateExportCommand, manifest: &domain::template_export::TemplateBoundaryManifest) -> core::result::Result<TemplateExportReport, TemplateExportPortError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineQueryService | application_service | reference | fn execute(&self, query: AdrBaselineQuery) -> core::result::Result<AdrBaselineQueryOutput, AdrBaselineQueryError> | 🔵 | 🔵 |
| AdrBaselineService | application_service | reference | fn execute(&self, command: AdrBaselineCommand) -> core::result::Result<AdrBaselineOutput, AdrBaselineError> | 🔵 | 🔵 |
| CatalogQueryService | application_service | add | fn check(&self, target: CatalogTarget, query: CatalogCheckQuery) -> core::result::Result<CatalogCheckReport, CatalogError> | 🔵 | 🔵 |
| CatalogService | application_service | modify | fn init(&self, target: CatalogTarget) -> core::result::Result<CatalogInitReport, CatalogError>, fn add(&self, target: CatalogTarget, command: CatalogAddCommand) -> core::result::Result<CatalogWriteReport, CatalogError>, fn import(&self, target: CatalogTarget, command: CatalogImportCommand) -> core::result::Result<CatalogWriteReport, CatalogError>, fn cite(&self, target: CatalogTarget, command: CatalogCiteCommand) -> core::result::Result<CatalogWriteReport, CatalogError> | 🔵 | 🔵 |
| CoverageVerifyService | application_service | reference | fn verify_coverage(&self, cmd: CoverageVerifyCommand) -> core::result::Result<domain::task_contract::CoverageVerifyOutcome, PreReviewGateError> | 🔵 | 🔵 |
| PreReviewGateService | application_service | reference | fn check(&self, cmd: PreReviewGateCommand) -> core::result::Result<domain::task_contract::PreReviewGateOutcome, PreReviewGateError> | 🔵 | 🔵 |
| TemplateExportService | application_service | reference | fn export(&self, command: TemplateExportCommand) -> core::result::Result<TemplateExportReport, TemplateExportError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrBaselineInteractor | interactor | reference | — | 🔵 | 🔵 |
| AdrBaselineQueryInteractor | interactor | reference | — | 🔵 | 🔵 |
| CatalogInteractor | interactor | modify | — | 🔵 | 🔵 |
| CoverageVerifyInteractor | interactor | reference | — | 🔵 | 🔵 |
| PreReviewGateInteractor | interactor | reference | — | 🔵 | 🔵 |
| TemplateExportInteractor | interactor | reference | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogTarget | dto | add | — | 🔵 | 🔵 |

