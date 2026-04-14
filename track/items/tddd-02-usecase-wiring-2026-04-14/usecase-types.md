<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| ReviewerError | error_type | reference | UserAbort, ReviewerAbort, Timeout, IllegalVerdict, Unexpected | 🔵 |
| DiffGetError | error_type | reference | Failed | 🔵 |
| ReviewHasherError | error_type | reference | Failed | 🔵 |
| ReviewCycleError | error_type | reference | UnknownScope, FileChangedDuringReview, Diff, Hash, Reviewer, Reader | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| TrackBlobReader | secondary_port | reference | fn read_spec_document(&self, branch: &str, track_id: &str) -> BlobFetchResult<SpecDocument>, fn read_type_catalogue(&self, branch: &str, track_id: &str, layer_id: &str) -> BlobFetchResult<TypeCatalogueDocument>, fn read_track_metadata(&self, branch: &str, track_id: &str) -> BlobFetchResult<TrackMetadata>, fn read_enabled_layers(&self, branch: &str) -> BlobFetchResult<Vec<String>> | 🔵 |
| Reviewer | secondary_port | reference | fn review(&self, target: &ReviewTarget) -> Result<(Verdict, LogInfo), ReviewerError>, fn fast_review(&self, target: &ReviewTarget) -> Result<(FastVerdict, LogInfo), ReviewerError> | 🔵 |
| DiffGetter | secondary_port | reference | fn list_diff_files(&self, base: &CommitHash) -> Result<Vec<FilePath>, DiffGetError> | 🔵 |
| ReviewHasher | secondary_port | reference | fn calc(&self, target: &ReviewTarget) -> Result<ReviewHash, ReviewHasherError> | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| HookHandler | application_service | reference | fn handle(&self, ctx: &HookContext, input: &HookInput) -> Result<HookVerdict, HookError> | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal |
|------|------|--------|---------|--------|
| SaveTrackUseCase | use_case | reference | — | 🔵 |
| LoadTrackUseCase | use_case | reference | — | 🔵 |

