<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| LayerId | value_object | — | — | 🔵 |
| ContractMapContent | value_object | — | — | 🔵 |
| ContractMapRenderOptions | value_object | — | — | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| CatalogueLoaderError | error_type | — | CatalogueNotFound, LayerDiscoveryFailed, DecodeFailed, SymlinkRejected, IoError, TopologicalSortFailed | 🔵 |
| ContractMapWriterError | error_type | — | IoError, SymlinkRejected, TrackNotFound | 🔵 |
| ValidationError | error_type | modify | EmptyString, InvalidTrackId, InvalidTaskId, InvalidCommitHash, InvalidTimestamp, InvalidTrackBranch, EmptyTrackTitle, EmptyTaskDescription, EmptyPlanSectionId, EmptyPlanSectionTitle, DuplicateTaskId, DuplicatePlanSectionId, UnknownTaskReference, DuplicateTaskReference, UnreferencedTask, OverrideIncompatibleWithResolvedTasks, TrackActivationRequiresPlanningOnly, TrackActivationRequiresSchemaV3, TrackAlreadyMaterialized, UnsupportedTargetStatus, SectionNotFound, NoSectionsAvailable, TaskDescriptionMutated, TaskRemoved, InvalidLayerId | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| CatalogueLoader | secondary_port | — | fn load_all(&self, track_id: &TrackId) -> Result<(Vec<LayerId>, BTreeMap<LayerId, TypeCatalogueDocument>), CatalogueLoaderError> | 🔵 |
| ContractMapWriter | secondary_port | — | fn write(&self, track_id: &TrackId, content: &ContractMapContent) -> Result<(), ContractMapWriterError> | 🔵 |

