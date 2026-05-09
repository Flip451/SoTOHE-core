<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueSpecSignalsWriter | secondary_port | reference | fn write_catalogue_spec_signals(&self, track_id: &TrackId, layer_id: &str, doc: &CatalogueSpecSignalsDocument) -> Result<(), RepositoryError> | 🔵 | 🟡 |
| SchemaExporterPort | secondary_port | reference | fn export_as_json(&self, crate_name: &str) -> Result<String, String> | 🔵 | 🟡 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsService | application_service | reference | fn run(&self, track_id: String, workspace_root: PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

