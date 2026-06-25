<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchInput | enum | reference | Tree, TreeFull, Members, DirectChecks | 🔵 | 🔵 |
| ConventionsInput | enum | reference | Add, UpdateIndex, VerifyIndex | 🔵 | 🔵 |
| DemoInput | enum | reference | Run | 🔵 | 🔵 |
| DomainInput | enum | reference | ExportSchema | 🔵 | 🔵 |
| DryInput | enum | reference | Write, Results, CheckApproved, FixLocal | 🔵 | 🔵 |
| FileInput | enum | reference | WriteAtomic | 🔵 | 🔵 |
| GitInput | enum | reference | AddAll, AddFromFile, CommitFromFile, NoteFromFile, SwitchAndPull, Unstage, CurrentBranchTrackIdStrict | 🔵 | 🔵 |
| GuardInput | enum | reference | Check | 🔵 | 🔵 |
| HookInput | enum | reference | Dispatch | 🔵 | 🔵 |
| HookName | enum | reference | HooksPathSetup, BlockDirectGitOps, BlockTestFileDeletion, GitRefUpdate, GitPrePush, SkillCompliance | 🔵 | 🔵 |
| PlanInput | enum | add | RunCodexLocal | 🟡 | 🔵 |
| PrInput | enum | reference | Push, Ensure, Status, WaitAndMerge, TriggerReview, PollReview, ReviewCycle | 🔵 | 🔵 |
| RefVerifyInput | enum | reference | Run, CheckApproved | 🔵 | 🔵 |
| ReviewInput | enum | reference | RunCodex, RunClaude, RunLocal, RunFixLocal, CheckApproved, Results, Classify, Files, ValidateScope, GetBriefing, PersistCommitHash | 🔵 | 🔵 |
| SemanticDupInput | enum | reference | FindSimilar, IndexBuild, IndexMeasureQuality, DupCheck | 🔵 | 🔵 |
| SignalGateName | enum | reference | Commit, Merge | 🔵 | 🔵 |
| SignalInput | enum | reference | CalcAdrUser, CheckAdrUser, CalcSpecAdr, CheckSpecAdr, CalcCatalogSpec, CheckCatalogSpec, CalcImplCatalog, CheckImplCatalog, CheckGate | 🔵 | 🔵 |
| TelemetryInput | enum | reference | Report, EmitArchivedTrackSubcommand | 🔵 | 🔵 |
| TrackInput | enum | reference | Init, Transition, Resolve, BranchCreate, BranchSwitch, ViewsValidate, ViewsSync, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, Archive, DetectActive | 🔵 | 🔵 |
| VerifyInput | enum | reference | TechStack, LatestTrack, ArchDocs, Layers, HooksPath, SpecAttribution, SpecFrontmatter, CanonicalModules, ModuleSize, DomainPurity, DomainStrings, UsecasePurity, DocLinks, ViewFreshness, SpecSignals, PlanArtifactRefs, CatalogueSpecRefs | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CommandOutcome | dto | reference | — | 🔵 | 🔵 |
| ExportSchemaInput | dto | reference | — | 🔵 | 🔵 |
| RefVerifyCheckApprovedInput | dto | reference | — | 🔵 | 🔵 |
| RefVerifyRunInput | dto | reference | — | 🔵 | 🔵 |
| TelemetryReportInput | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli_driver::ref_verify::format_pair_status | free_function | reference | fn(claim_hex: &str, evidence_hex: &str, reason: &str) -> String | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchDriver | primary_adapter | modify | — | 🔵 | 🔵 |
| ConventionsDriver | primary_adapter | modify | — | 🟡 | 🔵 |
| DemoDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| DomainDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| DryDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| FileDriver | primary_adapter | modify | — | 🟡 | 🔵 |
| GitDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| GuardDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| HookDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| PlanDriver | primary_adapter | add | — | 🟡 | 🔵 |
| PrDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| RefVerifyDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| ReviewDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| SemanticDupDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| SignalDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| TelemetryDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| TrackDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| VerifyDriver | primary_adapter | modify | — | 🟡 | 🔵 |

