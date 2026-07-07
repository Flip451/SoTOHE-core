<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateBoundaryManifestReadError | error_type | add | NotFound, Parse, InvalidPattern, InvalidManifest, Io | 🔵 | 🔵 |
| TemplateExportError | error_type | add | ManifestRead, Export | 🔵 | 🔵 |
| TemplateExportPortError | error_type | add | OutputDirExists, OverlayMissing, SourceMissing, UnclassifiedPath, Io | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateBoundaryManifestPort | secondary_port | add | fn read(&self, manifest_path: &std::path::Path) -> Result<domain::template_export::TemplateBoundaryManifest, TemplateBoundaryManifestReadError> | 🔵 | 🔵 |
| TemplateExportPort | secondary_port | add | fn export(&self, command: &TemplateExportCommand, manifest: &domain::template_export::TemplateBoundaryManifest) -> Result<TemplateExportReport, TemplateExportPortError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportService | application_service | add | fn export(&self, command: TemplateExportCommand) -> Result<TemplateExportReport, TemplateExportError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportReport | dto | add | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportCommand | command | add | — | 🔵 | 🔵 |

