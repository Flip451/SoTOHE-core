<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogPort | secondary_port | reference | fn init(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path) -> Result<CatalogInitReport, CatalogError>, fn add(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, command: CatalogAddCommand) -> Result<CatalogWriteReport, CatalogError>, fn import(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, command: CatalogImportCommand) -> Result<CatalogWriteReport, CatalogError>, fn cite(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, command: CatalogCiteCommand) -> Result<CatalogWriteReport, CatalogError>, fn check(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, query: CatalogCheckQuery) -> Result<CatalogCheckReport, CatalogError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::test_obligation::derive::DeriveTestObligationsInteractor | interactor | modify | — | 🔵 | 🔵 |

