<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandArgumentDto | dto | add | — | 🔵 | 🔵 |
| CommandArgvDto | dto | add | — | 🔵 | 🔵 |
| CommandConfigSchemaVersionDto | dto | add | — | 🔵 | 🔵 |
| CommandDeclarationIdDto | dto | add | — | 🟡 | 🔵 |
| CommandTimeoutSecondsDto | dto | add | — | 🔵 | 🔵 |
| ConfiguredCommandDto | dto | add | — | 🔵 | 🔵 |
| ContractedEntryRefDto | dto | modify | — | 🔵 | 🔵 |
| EntryKeyDto | dto | add | — | 🔵 | 🔵 |
| LayerIdDto | dto | add | — | 🔵 | 🔵 |
| PhaseCommandConfigDto | dto | add | — | 🟡 | 🔵 |
| PhaseCommandDeclarationDto | dto | add | — | 🟡 | 🔵 |
| PreReviewCommandConfigDto | dto | add | — | 🔵 | 🔵 |
| PreReviewScopeCommandDeclarationDto | dto | add | — | 🔵 | 🔵 |
| ReviewScopeNameDto | dto | add | — | 🔵 | 🔵 |
| TaskContractDocumentDto | dto | modify | — | 🔵 | 🔵 |
| TaskContractSchemaVersionDto | dto | add | — | 🔵 | 🔵 |
| TaskIdDto | dto | add | — | 🔵 | 🔵 |
| TrackIdDto | dto | add | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsPhaseCommandConfigLoader | secondary_adapter | add | impl Debug, impl Default, impl PhaseCommandConfigLoaderPort | 🟡 | 🔵 |
| FsPreReviewCommandConfigLoader | secondary_adapter | add | impl Debug, impl Default, impl PreReviewCommandConfigLoaderPort | 🔵 | 🔵 |
| GitCurrentReviewTrackResolver | secondary_adapter | add | impl Debug, impl Default, impl CurrentReviewTrackResolverPort | 🔵 | 🔵 |
| ProcessProgramRunner | secondary_adapter | add | impl Debug, impl Default, impl ProgramRunnerPort | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::operator_command_config::decode_phase_command_config | free_function | add | fn(dto: PhaseCommandConfigDto) -> Result<usecase::phase_command::PhaseCommandConfig, usecase::operator_command::CommandConfigValidationError> | 🟡 | 🔵 |
| infrastructure::operator_command_config::decode_pre_review_command_config | free_function | add | fn(dto: PreReviewCommandConfigDto) -> Result<usecase::pre_review_command::PreReviewCommandConfig, usecase::operator_command::CommandConfigValidationError> | 🔵 | 🔵 |

