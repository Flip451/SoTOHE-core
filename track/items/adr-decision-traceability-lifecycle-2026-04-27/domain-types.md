<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Typestates

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ProposedDecision | typestate | — | → AcceptedDecision, → DeprecatedDecision | 🔵 | 🔵 |
| AcceptedDecision | typestate | — | → ImplementedDecision, → SupersededDecision, → DeprecatedDecision | 🔵 | 🔵 |
| ImplementedDecision | typestate | — | → SupersededDecision, → DeprecatedDecision | 🔵 | 🔵 |
| SupersededDecision | typestate | — | ∅ (terminal) | 🔵 | 🔵 |
| DeprecatedDecision | typestate | — | ∅ (terminal) | 🔵 | 🔵 |

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrDecisionEntry | enum | modify | ProposedDecision, AcceptedDecision, ImplementedDecision, SupersededDecision, DeprecatedDecision | 🔵 | 🔵 |
| DecisionGrounds | enum | — | UserDecisionRef, ReviewFindingRef, Grandfathered, NoGrounds | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrFrontMatter | value_object | — | — | 🔵 | 🔵 |
| AdrDecisionCommon | value_object | — | — | 🔵 | 🔵 |
| AdrVerifyReport | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrFrontMatterError | error_type | — | EmptyAdrId | 🔵 | 🔵 |
| AdrDecisionCommonError | error_type | — | EmptyId, EmptyImplementedIn, EmptySupersededBy | 🔵 | 🔵 |
| AdrFilePortError | error_type | — | ListPaths, ReadFile | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrFilePort | secondary_port | — | fn list_adr_paths(&self) -> Result<Vec<PathBuf>, AdrFilePortError>, fn read_adr_frontmatter(&self, path: PathBuf) -> Result<AdrFrontMatter, AdrFilePortError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| evaluate_adr_decision | free_function | — | — | 🔵 | 🔵 |

