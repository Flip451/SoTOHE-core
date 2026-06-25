<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchCommand | enum | reference | Tree, TreeFull, Members, DirectChecks | 🔵 | 🔵 |
| BranchAction | enum | reference | Create, Switch | 🔵 | 🔵 |
| CliHookName | enum | reference | HooksPathSetup, BlockDirectGitOps, BlockTestFileDeletion, GitRefUpdate, GitPrePush, SkillCompliance | 🔵 | 🔵 |
| CodexRoundTypeArg | enum | reference | Fast, Final | 🔵 | 🔵 |
| ConventionsCommand | enum | reference | Add, UpdateIndex, VerifyIndex | 🔵 | 🔵 |
| DomainCommand | enum | reference | ExportSchema | 🔵 | 🔵 |
| DryCommand | enum | reference | Write, Results, CheckApproved, FixLocal | 🔵 | 🔵 |
| DupIndexCommand | enum | reference | Build, MeasureQuality | 🔵 | 🔵 |
| FileCommand | enum | reference | WriteAtomic | 🔵 | 🔵 |
| GateArg | enum | reference | Commit, Merge | 🔵 | 🔵 |
| GitCommand | enum | reference | AddAll, AddPaths, CommitFromFile, SwitchAndPull, Unstage | 🔵 | 🔵 |
| GuardCommand | enum | reference | Check | 🔵 | 🔵 |
| HookCommand | enum | reference | Dispatch | 🔵 | 🔵 |
| PlanCommand | enum | reference | CodexLocal | 🔵 | 🔵 |
| PrCommand | enum | reference | Push, EnsurePr, Status, WaitAndMerge, TriggerReview, PollReview, ReviewCycle | 🔵 | 🔵 |
| RefVerifyCommand | enum | reference | Run, CheckApproved | 🔵 | 🔵 |
| ResultsLimit | enum | reference | Zero, Count, All | 🔵 | 🔵 |
| ReviewCommand | enum | reference | CodexLocal, ClaudeLocal, Local, FixLocal, CheckApproved, Results, Classify, Files | 🔵 | 🔵 |
| RoundTypeFilter | enum | reference | Fast, Final, Any | 🔵 | 🔵 |
| SignalCommand | enum | reference | CalcAdrUser, CheckAdrUser, CalcSpecAdr, CheckSpecAdr, CalcCatalogSpec, CheckCatalogSpec, CalcImplCatalog, CheckImplCatalog, Check | 🔵 | 🔵 |
| TelemetryCommand | enum | reference | Report | 🔵 | 🔵 |
| TrackCommand | enum | reference | Archive, Transition, Branch, Resolve, Views, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, TypeGraph, BaselineGraph, ContractMap, SpecElementHash, BaselineCapture, FixpointResolve, SetCommitHash, Lint, CatalogueImplSignals | 🔵 | 🔵 |
| VerdictFilterArg | enum | reference | All, NotAViolation, Accepted, Violation | 🔵 | 🔵 |
| VerifyCommand | enum | reference | TechStack, LatestTrack, ArchDocs, Layers, HooksPath, SpecAttribution, SpecFrontmatter, CanonicalModules, ModuleSize, DomainPurity, DomainStrings, UsecasePurity, DocLinks, ViewFreshness, SpecSignals, PlanArtifactRefs, CatalogueSpecRefs | 🔵 | 🔵 |
| ViewAction | enum | reference | Validate, Sync | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliError | error_type | reference | Message, Io | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BranchArgs | dto | reference | — | 🔵 | 🔵 |
| CalcAdrUserArgs | dto | reference | — | 🔵 | 🔵 |
| CalcCatalogSpecArgs | dto | reference | — | 🔵 | 🔵 |
| CalcImplCatalogArgs | dto | reference | — | 🔵 | 🔵 |
| CalcSpecAdrArgs | dto | reference | — | 🔵 | 🔵 |
| CatalogueSpecRefsArgs | dto | reference | — | 🔵 | 🔵 |
| CheckAdrUserArgs | dto | reference | — | 🔵 | 🔵 |
| CheckApprovedArgs | dto | reference | — | 🔵 | 🔵 |
| CheckCatalogSpecArgs | dto | reference | — | 🔵 | 🔵 |
| CheckFlags | dto | reference | — | 🔵 | 🔵 |
| CheckImplCatalogArgs | dto | reference | — | 🔵 | 🔵 |
| CheckSpecAdrArgs | dto | reference | — | 🔵 | 🔵 |
| ClaudeLocalArgs | dto | reference | — | 🔵 | 🔵 |
| CodexInvocation | dto | delete | — | 🔵 | 🔵 |
| CodexLocalArgs | dto | reference | — | 🔵 | 🔵 |
| CommitFromFileArgs | dto | reference | — | 🔵 | 🔵 |
| DryCheckApprovedArgs | dto | reference | — | 🔵 | 🔵 |
| DryFixLocalArgs | dto | reference | — | 🔵 | 🔵 |
| DryResultsArgs | dto | reference | — | 🔵 | 🔵 |
| DryWriteArgs | dto | reference | — | 🔵 | 🔵 |
| DupCheckArgs | dto | reference | — | 🔵 | 🔵 |
| DupIndexBuildArgs | dto | reference | — | 🔵 | 🔵 |
| DupIndexMeasureQualityArgs | dto | reference | — | 🔵 | 🔵 |
| EnsurePrArgs | dto | reference | — | 🔵 | 🔵 |
| ExportSchemaArgs | dto | reference | — | 🔵 | 🔵 |
| FileArgs | dto | reference | — | 🔵 | 🔵 |
| FindSimilarArgs | dto | reference | — | 🔵 | 🔵 |
| FixpointResolveArgs | dto | reference | — | 🔵 | 🔵 |
| PlanArtifactRefsArgs | dto | reference | — | 🔵 | 🔵 |
| PlanCodexLocalArgs | dto | reference | — | 🔵 | 🔵 |
| PlanRunResult | dto | delete | — | 🔵 | 🔵 |
| PollReviewArgs | dto | reference | — | 🔵 | 🔵 |
| PushArgs | dto | reference | — | 🔵 | 🔵 |
| ReportArgs | dto | reference | — | 🔵 | 🔵 |
| ResolveArgs | dto | reference | — | 🔵 | 🔵 |
| ResultsArgs | dto | reference | — | 🔵 | 🔵 |
| ReviewCycleArgs | dto | reference | — | 🔵 | 🔵 |
| RunArgs | dto | reference | — | 🔵 | 🔵 |
| SetCommitHashArgs | dto | reference | — | 🔵 | 🔵 |
| SignalCheckArgs | dto | reference | — | 🔵 | 🔵 |
| SpecVerifyArgs | dto | reference | — | 🔵 | 🔵 |
| StatusArgs | dto | reference | — | 🔵 | 🔵 |
| SwitchAndPullArgs | dto | reference | — | 🔵 | 🔵 |
| TriggerReviewArgs | dto | reference | — | 🔵 | 🔵 |
| UnstageArgs | dto | reference | — | 🔵 | 🔵 |
| VerifyArgs | dto | reference | — | 🔵 | 🔵 |
| WaitAndMergeArgs | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::plan::codex_local::build_codex_invocation | free_function | delete | fn() -> super::CodexInvocation | 🔵 | 🔵 |
| cli::commands::plan::codex_local::build_prompt | free_function | delete | fn() -> Result<String, String> | 🔵 | 🔵 |
| cli::commands::plan::codex_local::codex_bin | free_function | delete | fn() -> std::ffi::OsString | 🔵 | 🔵 |
| cli::commands::plan::codex_local::configure_child_process_group | free_function | delete | fn() -> () | 🔵 | 🔵 |
| cli::commands::plan::codex_local::plan_input_from_args | free_function | add | fn(args: &PlanCodexLocalArgs) -> Result<cli_driver::plan::PlanInput, crate::CliError> | 🔵 | 🔵 |
| cli::commands::plan::codex_local::prepare_session_log_path | free_function | delete | fn() -> Result<std::path::PathBuf, String> | 🔵 | 🔵 |
| cli::commands::plan::codex_local::run_codex_child | free_function | delete | fn() -> Result<super::PlanRunResult, String> | 🔵 | 🔵 |
| cli::commands::plan::codex_local::run_codex_local_invocation | free_function | delete | fn() -> Result<super::PlanRunResult, String> | 🔵 | 🔵 |
| cli::commands::plan::codex_local::run_execute_codex_local | free_function | add | fn(args: &PlanCodexLocalArgs, handle: impl FnOnce(cli_driver::plan::PlanInput) -> cli_driver::render::CommandOutcome) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::plan::codex_local::spawn_codex | free_function | delete | fn() -> Result<std::process::Child, String> | 🔵 | 🔵 |
| cli::commands::plan::codex_local::terminate_planner_child | free_function | delete | fn() -> Result<(), String> | 🔵 | 🔵 |
| cli::commands::review::codex_local::emit_outcome_output_to | free_function | add | fn(stdout_text: Option<&str>, stderr_text: Option<&str>, exit_code: u8, writer: &mut W) -> Result<u8, crate::CliError> | 🔵 | 🔵 |
| cli::commands::review::codex_local::review_input_from_args | free_function | add | fn(args: &CodexLocalArgs) -> Result<cli_driver::review::ReviewInput, crate::CliError> | 🔵 | 🔵 |
| cli::commands::review::codex_local::run_execute_codex_local | free_function | modify | fn(args: &CodexLocalArgs, handle: impl FnOnce(cli_driver::review::ReviewInput) -> cli_driver::render::CommandOutcome) -> std::process::ExitCode | 🔵 | 🔵 |

