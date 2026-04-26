<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| MemberDeclaration | enum | reference | Variant, Field | 🔵 | 🔵 |
| TypeDefinitionKind | enum | modify | Typestate, Enum, ValueObject, ErrorType, SecondaryPort, ApplicationService, UseCase, Interactor, Dto, Command, Query, Factory, SecondaryAdapter, FreeFunction | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeCatalogueEntry | value_object | modify | — | 🔵 | 🔵 |
| LayerId | value_object | reference | — | 🔵 | 🔵 |
| ContractMapContent | value_object | reference | — | 🔵 | 🔵 |
| ContractMapRenderOptions | value_object | modify | — | 🔵 | 🔵 |
| TypeCatalogueDocument | value_object | reference | — | 🔵 | 🔵 |
| TrackId | value_object | reference | — | 🔵 | 🟡 |
| TaskId | value_object | reference | — | 🔵 | 🔵 |
| CommitHash | value_object | reference | — | 🔵 | 🔵 |
| TrackBranch | value_object | reference | — | 🔵 | 🔵 |
| NonEmptyString | value_object | reference | — | 🔵 | 🔵 |
| ReviewGroupName | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLoaderError | error_type | reference | CatalogueNotFound, LayerDiscoveryFailed, DecodeFailed, SymlinkRejected, IoError, TopologicalSortFailed | 🔵 | 🟡 |
| ContractMapWriterError | error_type | reference | IoError, SymlinkRejected, TrackNotFound | 🔵 | 🟡 |
| ValidationError | error_type | modify | EmptyString, InvalidTrackId, InvalidTaskId, InvalidCommitHash, InvalidTimestamp, InvalidTrackBranch, EmptyTrackTitle, EmptyTaskDescription, EmptyPlanSectionId, EmptyPlanSectionTitle, DuplicateTaskId, DuplicatePlanSectionId, UnknownTaskReference, DuplicateTaskReference, UnreferencedTask, OverrideIncompatibleWithResolvedTasks, TrackActivationRequiresPlanningOnly, TrackActivationRequiresSchemaV3, TrackAlreadyMaterialized, UnsupportedTargetStatus, SectionNotFound, NoSectionsAvailable, TaskDescriptionMutated, TaskRemoved, InvalidLayerId | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogueLoader | secondary_port | reference | fn load_all(&self, track_id: &TrackId) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), CatalogueLoaderError> | 🔵 | 🔵 |
| ContractMapWriter | secondary_port | reference | fn write(&self, track_id: &TrackId, content: &ContractMapContent) -> Result<(), ContractMapWriterError> | 🔵 | 🟡 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| render_contract_map | free_function | reference | fn (catalogues: &BTreeMap<LayerId, TypeCatalogueDocument>, layer_order: &[LayerId], opts: &ContractMapRenderOptions) -> ContractMapContent | 🟡 | 🟡 |

