<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckAgentJudgment | enum | — | NotAViolation, Accepted, Violation | 🟡 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckAgentError | error_type | — | UserAbort, AgentAbort, Timeout, IllegalOutput, Unexpected | 🟡 | 🔵 |
| DryCheckCycleError | error_type | — | Embedding, Index, Agent, Reader, Writer, Diff, Entry | 🟡 | 🔵 |
| DryCheckDiffError | error_type | — | Failed | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckAgentPort | secondary_port | — | fn judge(&self, changed_fragment: &domain::semantic_dup::CodeFragment, candidate_fragment: &domain::semantic_dup::CodeFragment) -> Result<DryCheckAgentJudgment, DryCheckAgentError> | 🟡 | 🔵 |
| DryCheckDiffSource | secondary_port | — | fn list_changed_hunks(&self, base: &domain::CommitHash) -> Result<Vec<domain::dry_check::DiffFileHunks>, DryCheckDiffError> | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckApprovalService | application_service | — | fn check_approved(&self, corpus_fragments: Vec<domain::semantic_dup::CodeFragment>, diff_fragments: &[domain::semantic_dup::CodeFragment], threshold: domain::semantic_dup::SimilarityThreshold) -> Result<domain::dry_check::DryCheckApprovalVerdict, DryCheckCycleError> | 🟡 | 🔵 |
| DryCheckResultsService | application_service | — | fn get_results(&self, filter: domain::dry_check::VerdictFilter) -> Result<DryCheckResults, domain::dry_check::DryCheckReaderError> | 🟡 | 🔵 |
| DryCheckService | application_service | — | fn run_dry_check(&self, corpus_fragments: Vec<domain::semantic_dup::CodeFragment>, diff_fragments: Vec<domain::semantic_dup::CodeFragment>, threshold: domain::semantic_dup::SimilarityThreshold, base_commit: domain::CommitHash) -> Result<Vec<domain::dry_check::DryCheckFinding>, DryCheckCycleError> | 🟡 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckApprovalInteractor | interactor | — | — | 🟡 | 🔵 |
| DryCheckInteractor | interactor | — | — | 🟡 | 🔵 |
| DryCheckResultsInteractor | interactor | — | — | 🟡 | 🔵 |

## Queries

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckResults | query | — | — | 🟡 | 🔵 |

