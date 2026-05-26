<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsError | error_type | modify | GitDiscoverFailed, RulesFileMissing, RulesParseError, SymlinkRejected, BranchNotFound, BranchMismatch, TypeSignalsRecomputeFailed | 🔵 | 🔵 |
| TypeSignalsError | error_type | reference | InvalidTrackId, NonActiveTrack, BranchTrackMismatch, LayerBindingsLoad, NoLayers, EvaluationFailed, InconsistentRequest | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsService | application_service | reference | fn run(&self, track_id: String, workspace_root: std::path::PathBuf) -> Result<PreCommitTypeSignalsOutput, PreCommitTypeSignalsError> | 🔵 | 🔵 |
| TypeSignalsService | application_service | reference | fn run(&self, request: TypeSignalsRequest) -> Result<(), TypeSignalsError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsInteractor | interactor | modify | — | 🔵 | 🔵 |
| TypeSignalsInteractor | interactor | reference | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PreCommitTypeSignalsOutput | dto | modify | — | 🔵 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TypeSignalsRequest | command | reference | — | 🔵 | 🔵 |

