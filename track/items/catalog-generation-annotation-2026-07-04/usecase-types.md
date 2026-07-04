<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogCheckVerdict | enum | add | Pass, Interim, Blocked, Skipped | 🟡 | 🔵 |
| CatalogGateContext | enum | add | Phase2, Commit, Merge | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogError | error_type | add | FileExists, FileMissing, DuplicateEntry, AnchorNotFound, InvalidRole, ParseFragment, SchemaInvalid, DraftIncomplete, Port | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogPort | secondary_port | add | fn init(&self, track_id: &str, items_dir: &std::path::Path) -> Result<CatalogInitReport, CatalogError>, fn add(&self, track_id: &str, items_dir: &std::path::Path, command: CatalogAddCommand) -> Result<CatalogWriteReport, CatalogError>, fn import(&self, track_id: &str, items_dir: &std::path::Path, command: CatalogImportCommand) -> Result<CatalogWriteReport, CatalogError>, fn cite(&self, track_id: &str, items_dir: &std::path::Path, command: CatalogCiteCommand) -> Result<CatalogWriteReport, CatalogError>, fn check(&self, track_id: &str, items_dir: &std::path::Path, query: CatalogCheckQuery) -> Result<CatalogCheckReport, CatalogError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogService | application_service | add | fn init(&self, track_id: &str, items_dir: &std::path::Path) -> Result<CatalogInitReport, CatalogError>, fn add(&self, track_id: &str, items_dir: &std::path::Path, command: CatalogAddCommand) -> Result<CatalogWriteReport, CatalogError>, fn import(&self, track_id: &str, items_dir: &std::path::Path, command: CatalogImportCommand) -> Result<CatalogWriteReport, CatalogError>, fn cite(&self, track_id: &str, items_dir: &std::path::Path, command: CatalogCiteCommand) -> Result<CatalogWriteReport, CatalogError>, fn check(&self, track_id: &str, items_dir: &std::path::Path, query: CatalogCheckQuery) -> Result<CatalogCheckReport, CatalogError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogInteractor | interactor | add | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogCheckReport | dto | add | — | 🟡 | 🔵 |
| CatalogInitReport | dto | add | — | 🟡 | 🔵 |
| CatalogWriteReport | dto | add | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogAddCommand | command | add | — | 🟡 | 🔵 |
| CatalogCiteCommand | command | add | — | 🟡 | 🔵 |
| CatalogImportCommand | command | add | — | 🟡 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogCheckQuery | query | add | — | 🟡 | 🔵 |

