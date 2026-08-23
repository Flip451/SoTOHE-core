<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::track_lifecycle::tddd::catalogue_lint_active::TrackCatalogueLintActivePort | secondary_port | reference | fn execute(&self, track_id: domain::ids::TrackId, command: TrackCatalogueLintActiveCommand) -> Result<TrackCatalogueLintActiveResult, TrackCatalogueLintActiveError> | 🔵 | 🔵 |
| usecase::track_lifecycle::tddd::lint::TrackLintPort | secondary_port | reference | fn execute(&self, track_id: domain::ids::TrackId, command: TrackLintCommand) -> Result<TrackLintResult, TrackLintError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLint | application_service | reference | fn execute(&self, cmd: RunCatalogueLintCommand) -> Result<Vec<domain::tddd::catalogue_linter::CatalogueLintViolation>, RunCatalogueLintError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::catalogue_lint_workflow::RunCatalogueLintInteractor | interactor | modify | — | 🔵 | 🔵 |

