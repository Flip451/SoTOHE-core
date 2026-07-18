<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexRuntimeProvisionError | error_type | add | ProjectRootInvalid, NoUsableCandidate, NpmQueryFailed, LinkUpdateFailed | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CapabilityProviderPort | secondary_port | reference | fn provider(&self) -> &ProviderName, fn dispatch(&self, request: &CapabilityDispatchRequest) -> Result<CapabilityDispatchOutcome, CapabilityExecError> | 🔵 | 🔵 |
| CodexRuntimeProvisionPort | secondary_port | add | fn provision(&self, project_root: &std::path::Path) -> Result<(), CodexRuntimeProvisionError> | 🟡 | 🔵 |
| DryCheckAgentPort | secondary_port | reference | fn judge(&self, changed_fragment: &domain::semantic_dup::CodeFragment, candidate_fragment: &domain::semantic_dup::CodeFragment, tier: DryCheckJudgeTier) -> Result<DryCheckAgentJudgment, DryCheckAgentError> | 🔵 | 🔵 |
| ReviewFixRunner | secondary_port | reference | fn run_fix(&self, command: RunReviewFixCommand) -> Result<RunReviewFixOutput, ReviewFixRunnerError> | 🔵 | 🔵 |
| Reviewer | secondary_port | reference | fn review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::Verdict, domain::review_v2::LogInfo), ReviewerError>, fn fast_review(&self, target: &domain::review_v2::ReviewTarget) -> Result<(domain::review_v2::FastVerdict, domain::review_v2::LogInfo), ReviewerError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexRuntimeProvisionService | application_service | add | fn provision(&self, project_root: &std::path::Path) -> Result<(), CodexRuntimeProvisionError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CodexRuntimeProvisionInteractor | interactor | add | — | 🟡 | 🔵 |

