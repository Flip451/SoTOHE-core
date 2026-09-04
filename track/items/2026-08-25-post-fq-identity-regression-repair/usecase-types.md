<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::pre_review_gate::PreReviewGateError | error_type | modify | TaskContractNotFound, TaskContractReadFailed, CatalogueReadFailed, CatalogueFreshnessMismatch, SignalReadFailed, ImplPlanReadFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AttestedCatalogueDocumentLoaderPort | secondary_port | add | fn load(&self, path: &std::path::Path) -> Result<domain::tddd::catalogue_v2::catalogue_impl_signals_ports::AttestedCatalogueDocument, domain::tddd::catalogue_v2::catalogue_impl_signals_ports::CatalogueDocumentLoaderError> | 🔵 | 🔵 |
| CatalogPort | secondary_port | reference | fn init(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path) -> Result<CatalogInitReport, CatalogError>, fn add(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, command: CatalogAddCommand) -> Result<CatalogWriteReport, CatalogError>, fn import(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, command: CatalogImportCommand) -> Result<CatalogWriteReport, CatalogError>, fn cite(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, command: CatalogCiteCommand) -> Result<CatalogWriteReport, CatalogError>, fn check(&self, track_id: &domain::ids::TrackId, items_dir: &std::path::Path, query: CatalogCheckQuery) -> Result<CatalogCheckReport, CatalogError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::catalogue_impl_signals::interactor::CatalogueImplSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |
| usecase::pre_review_gate::CoverageVerifyInteractor | interactor | modify | — | 🔵 | 🔵 |
| usecase::pre_review_gate::PreReviewGateInteractor | interactor | modify | — | 🔵 | 🔵 |
| usecase::test_obligation::check::CheckTestObligationsInteractor | interactor | modify | — | 🔵 | 🔵 |
| usecase::test_obligation::derive::DeriveTestObligationsInteractor | interactor | modify | — | 🔵 | 🔵 |
| usecase::test_obligation::evaluate::EvaluateTestObligationsInteractor | interactor | modify | — | 🔵 | 🔵 |
| usecase::test_obligation::results::TestObligationResultsInteractor | interactor | modify | — | 🔵 | 🔵 |

