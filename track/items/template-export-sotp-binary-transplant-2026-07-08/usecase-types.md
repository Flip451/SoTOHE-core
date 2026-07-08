<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SelfBinaryTransplantError | error_type | add | SourcePathUnavailable, DestinationWriteFailure, PermissionSetFailure | 🟡 | 🔵 |
| TemplateExportError | error_type | modify | ManifestRead, Export, BinaryTransplant | 🟡 | 🔵 |
| TemplateExportPortError | error_type | reference | OutputDirExists, OverlayMissing, SourceMissing, UnclassifiedPath, Io | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SelfBinaryTransplantPort | secondary_port | add | fn transplant(&self, destination: &std::path::Path) -> Result<(), SelfBinaryTransplantError> | 🟡 | 🔵 |
| TemplateExportPort | secondary_port | reference | fn export(&self, command: &TemplateExportCommand, manifest: &domain::template_export::TemplateBoundaryManifest) -> Result<TemplateExportReport, TemplateExportPortError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportService | application_service | reference | fn export(&self, command: TemplateExportCommand) -> Result<TemplateExportReport, TemplateExportError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportInteractor | interactor | modify | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportReport | dto | reference | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportCommand | command | reference | — | 🔵 | 🔵 |

