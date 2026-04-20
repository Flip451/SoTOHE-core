<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| ImplPlanCodecError | error_type | — | Json, UnsupportedSchemaVersion, Validation | 🔵 |
| TaskCoverageCodecError | error_type | — | Json, UnsupportedSchemaVersion, Validation | 🔵 |
| PlanArtifactRefsError | error_type | — | Json, Io, UnresolvedSpecRef, SpecHashMismatch, InvalidAnchor, CoverageViolation | 🟡 |
| SpecCodecError | error_type | modify | Json, Validation, UnsupportedSchemaVersion, InvalidField, DomainValidation | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| ImplPlanDocumentDto | dto | — | — | 🔵 |
| TaskCoverageDocumentDto | dto | — | — | 🔵 |
| TrackDocumentV2 | dto | modify | — | 🔵 |
| ImplPlanTaskDto | dto | — | — | 🔵 |
| ImplPlanPlanDto | dto | — | — | 🔵 |
| ImplPlanSectionDto | dto | — | — | 🔵 |

