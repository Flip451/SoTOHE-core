<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeClassification | enum | — | Named, Other, Excluded | 🟡 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PathClassification | value_object | — | — | 🟡 | 🟡 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeQueryError | error_type | — | DiffGet, UnknownScope | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiffGetter | secondary_port | reference | fn list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeQueryService | application_service | — | fn classify(&self, paths: Vec<FilePath>) -> Result<Vec<PathClassification>, ScopeQueryError>, fn files(&self, scope: ScopeName) -> Result<Vec<FilePath>, ScopeQueryError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ScopeQueryInteractor | interactor | — | — | 🟡 | 🔵 |

