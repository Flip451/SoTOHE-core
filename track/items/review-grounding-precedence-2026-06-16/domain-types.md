<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrDecisionEntry | enum | reference | ProposedDecision, AcceptedDecision, ImplementedDecision, SupersededDecision, DeprecatedDecision | 🔵 | 🔵 |
| DecisionGrounds | enum | modify | UserDecisionRef, ReviewFindingRef, Grandfathered, NoGrounds | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrDecisionCommon | value_object | modify | — | 🟡 | 🔵 |
| DecisionGroundRef | value_object | — | — | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrDecisionCommonError | error_type | reference | EmptyId, EmptyImplementedIn, EmptySupersededBy | 🔵 | 🔵 |
| DomainError | error_type | reference | Validation, Transition | 🔵 | 🔵 |
| ValidationError | error_type | modify | EmptyString, InvalidTrackId, InvalidTaskId, InvalidCommitHash, InvalidTimestamp, InvalidTrackBranch, BranchIdMismatch, StatusOverrideMismatch, EmptyTrackTitle, EmptyTaskDescription, EmptyPlanSectionId, EmptyPlanSectionTitle, DuplicateTaskId, DuplicatePlanSectionId, UnknownTaskReference, DuplicateTaskReference, UnreferencedTask, OverrideIncompatibleWithResolvedTasks, TrackActivationRequiresPlanningOnly, TrackActivationRequiresSchemaV3, TrackAlreadyMaterialized, UnsupportedTargetStatus, SectionNotFound, NoSectionsAvailable, TaskDescriptionMutated, TaskRemoved, DuplicateElementId, InvalidLayerId, InvalidSpecElementId, EmptyAdrAnchor, EmptyConventionAnchor, InvalidContentHash, EmptyInformalGroundSummary, MultiLineInformalGroundSummary, EmptyDecisionGroundRef | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrFilePort | secondary_port | reference | fn list_adr_paths(&self) -> Result<Vec<std::path::PathBuf>, AdrFilePortError>, fn read_adr_frontmatter(&self, path: std::path::PathBuf) -> Result<AdrFrontMatter, AdrFilePortError> | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::adr_decision::evaluator::evaluate_adr_decision | free_function | modify | fn(entry: AdrDecisionEntry) -> DecisionGrounds | 🔵 | 🔵 |

