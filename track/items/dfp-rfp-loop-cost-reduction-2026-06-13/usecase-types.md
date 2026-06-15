<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckJudgeTier | enum | — | Fast, Final | 🔵 | 🔵 |
| RefVerifyGateStatus | enum | — | Approved, Blocked | 🔵 | 🔵 |
| ReviewGateStatus | enum | — | Approved, NeedsReview | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckConfig | value_object | — | — | 🟡 | 🔵 |
| DryCheckParallelism | value_object | — | — | 🟡 | 🔵 |
| DryCheckPercent | value_object | — | — | 🟡 | 🔵 |
| FixpointCurrentBranch | value_object | — | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckAgentError | error_type | modify | UserAbort, AgentAbort, Timeout, IllegalOutput, Unexpected | 🔵 | 🔵 |
| DryCheckCycleError | error_type | modify | Embedding, Index, Agent, Reader, Writer, Diff, Entry, CoveragePort, InvalidParallelism, InvalidPercent | 🟡 | 🔵 |
| FixpointResolveError | error_type | — | InvalidTrackId, InvalidCurrentBranch, TrackNotActive, GateQueryFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckAgentPort | secondary_port | modify | fn judge(&self, changed_fragment: &domain::semantic_dup::CodeFragment, candidate_fragment: &domain::semantic_dup::CodeFragment, tier: DryCheckJudgeTier) -> Result<DryCheckAgentJudgment, DryCheckAgentError> | 🔵 | 🔵 |
| DryCheckCoveragePort | secondary_port | — | fn read_coverage(&self, track_id: &domain::TrackId) -> Result<Option<domain::dry_check::DryCheckCoverageRecord>, DryCheckCycleError>, fn write_coverage(&self, track_id: &domain::TrackId, record: domain::dry_check::DryCheckCoverageRecord) -> Result<(), DryCheckCycleError> | 🔵 | 🔵 |
| RefVerifyGateStatePort | secondary_port | — | fn ref_verify_status(&self, track_id: &domain::TrackId) -> Result<RefVerifyGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| ReviewGateStatePort | secondary_port | — | fn review_status(&self, track_id: &domain::TrackId) -> Result<ReviewGateStatus, FixpointResolveError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckApprovalService | application_service | modify | fn check_approved(&self, track_id: &domain::TrackId, current_fragment_refs: &std::collections::BTreeSet<domain::dry_check::FragmentRef>) -> Result<domain::dry_check::DryCheckApprovalVerdict, DryCheckCycleError> | 🔵 | 🔵 |
| DryCheckService | application_service | reference | fn run_dry_check(&self) -> Result<(), DryCheckCycleError> | 🔵 | 🔵 |
| FixpointResolveService | application_service | — | fn resolve(&self, cmd: &FixpointResolveCommand) -> Result<domain::track_phase::FixpointStep, FixpointResolveError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckApprovalInteractor | interactor | modify | — | 🔵 | 🔵 |
| DryCheckInteractor | interactor | modify | — | 🟡 | 🔵 |
| FixpointResolveInteractor | interactor | — | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FixpointResolveCommand | command | — | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::dry_check::shared::fragment_ref_of | free_function | — | fn(fragment: &domain::semantic_dup::CodeFragment) -> Result<domain::dry_check::FragmentRef, String> | 🔵 | 🔵 |

