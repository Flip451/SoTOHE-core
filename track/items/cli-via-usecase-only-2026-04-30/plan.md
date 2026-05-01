<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# CLI→domain 直接参照禁止と usecase 経由への一本化

## Tasks (14/14 resolved)

### S1 — Usecase Boundary — Guard and Hook

> Introduce the usecase-owned boundary types and application service traits covering the guard shell-command check and hook dispatch concerns. Both concerns are enforcement-time checks and are grouped in one section, but they use distinct secondary ports: T002 adds ShellParserPort (lossy Vec<String>, used by GuardCheckInteractor) and T003 adds HookShellParserPort (full-fidelity SimpleCommand, used by HookDispatchInteractor) so that hook guard policy can enforce block-direct-git-ops without security regression.
> T002 covers GuardDecision, GuardCheckOutput, GuardCheckService, GuardCheckInteractor, and ShellParserPort.
> T003 covers HookDispatchCommand, HookVerdictOutput, HookVerdictDecision, HookDispatchError, HookDispatchService, HookDispatchInteractor, and HookShellParserPort.

- [x] **T002**: Add guard boundary types and service to usecase layer: GuardDecision, GuardCheckOutput, GuardCheckService, GuardCheckInteractor, ShellParserPort; also add ShellParserPort impl to ConchShellParser (infrastructure) so the CLI composition root can wire Arc<dyn ShellParserPort> (`94ec2339cc32504b0eb92615706480f454ec02c3`)
- [x] **T003**: Add hook dispatch boundary types and service to usecase layer: HookDispatchCommand, HookVerdictOutput, HookVerdictDecision, HookDispatchError, HookDispatchService, HookDispatchInteractor, HookShellParserPort; also add HookShellParserPort impl to ConchShellParser (infrastructure) so the CLI composition root can wire Arc<dyn HookShellParserPort> into HookDispatchInteractor (`686690866f5d8fbf7c1dd1fbffe67226db3e5a73`)

### S2 — Usecase Boundary — Domain Schema Export

> Introduce the ExportSchema family (command, error, service, interactor) plus the SchemaExporterPort secondary port so that the CLI domain export-schema subcommand can be migrated away from importing domain::schema::SchemaExporter directly.
> SchemaExporterPort is a usecase-owned driven port whose export_as_json method returns a serialized JSON string. RustdocSchemaExporter (infrastructure) implements SchemaExporterPort in addition to domain::schema::SchemaExporter. The CLI composition root wires RustdocSchemaExporter as Arc<dyn SchemaExporterPort> into ExportSchemaInteractor without importing the domain trait.

- [x] **T004**: Add domain schema export boundary types and service to usecase layer: ExportSchemaCommand, ExportSchemaError, ExportSchemaService, ExportSchemaInteractor, SchemaExporterPort; also add SchemaExporterPort impl to RustdocSchemaExporter (infrastructure) so the CLI composition root can wire Arc<dyn SchemaExporterPort> (`9d895b683c071409282e9e85af812268a17d91ed`)

### S3 — Usecase Boundary — Review

> Introduce the review boundary types covering check-approved (ReviewApprovalDecision, ReviewApprovalOutput, ReviewCheckApprovedService, ReviewCheckApprovedError) and run-review (RunReviewCommand, RunReviewOutput, RunReviewError, RunReviewService, ReviewRoundType). All of these protect the CLI from importing domain::review_v2::* directly.

- [x] **T005**: Add review boundary types, services, and interactors to usecase layer: ReviewApprovalDecision, ReviewApprovalOutput, ReviewCheckApprovedService, ReviewCheckApprovedError, ReviewCheckApprovedInteractor, RunReviewCommand, RunReviewOutput, RunReviewError, RunReviewService, RunReviewInteractor, ReviewRoundType (`9d895b683c071409282e9e85af812268a17d91ed`)

### S4 — Usecase Boundary — Track Task Operations

> Introduce boundary types for all track task mutation and query operations: TaskTransitionCommand, AddTaskCommand, SetOverrideCommand, ClearOverrideCommand, TaskOperationError, TaskOperationOutput, TaskOperationService, TrackStatusOutput, TaskQueryService, NextTaskOutput, TaskCountsOutput.
> TaskOperationService is the application service trait for mutation operations (transition_task, add_task, set_override, clear_override), accepting the command objects and returning TaskOperationOutput.
> This section eliminates the largest cluster of domain type imports in the CLI (domain::TrackId, domain::CommitHash, domain::TrackMetadata, domain::derive_track_status, domain::DomainError, domain::TaskStatusKind, domain::ImplPlanReader).

- [x] **T006**: Add track task operation boundary types, services, and interactors to usecase layer: TaskTransitionCommand, AddTaskCommand, SetOverrideCommand, ClearOverrideCommand, TaskOperationError, TaskOperationOutput, TaskOperationService, TaskOperationInteractor, TaskQueryService, TaskQueryInteractor, NextTaskOutput, TaskCountsOutput, TrackStatusOutput (`9d895b683c071409282e9e85af812268a17d91ed`)

### S5 — Usecase Boundary — Track Phase and Catalogue Verification

> Introduce boundary types for track phase resolution (TrackPhaseOutput, TrackPhaseError, TrackPhaseService, TrackPhaseInteractor) and all catalogue verification services (VerifyCatalogueConsistencyService, VerifyCatalogueSpecSignalsService, TypeSignalsService, VerifyCatalogueSpecRefsService and their associated Output/Error DTOs and LayerSignalSummary).
> These cover domain::ImplPlanReader, domain::ConfidenceSignal, domain::TypeCatalogueDocument, domain::ConsistencyReport, and related types.

- [x] **T007**: Add track phase resolution and catalogue verify boundary types, services, and interactors to usecase layer: TrackPhaseOutput, TrackPhaseError, TrackPhaseService, TrackPhaseInteractor, VerifyCatalogueConsistencyService, VerifyCatalogueConsistencyOutput, VerifyCatalogueConsistencyError, VerifyCatalogueConsistencyInteractor, VerifyCatalogueSpecSignalsService, VerifySpecSignalsOutput, VerifySpecSignalsError, VerifyCatalogueSpecSignalsInteractor, TypeSignalsService, LayerSignalSummary, TypeSignalsError, TypeSignalsInteractor, VerifyCatalogueSpecRefsService, VerifyCatalogueSpecRefsOutput, VerifyCatalogueSpecRefsError, VerifyCatalogueSpecRefsInteractor (`9d895b683c071409282e9e85af812268a17d91ed`)

### S6 — Usecase Boundary — Pre-commit and Commit Hash Persistence

> Introduce PreCommitTypeSignalsService (with Output and Error) and CommitHashPersistenceService (with Error) so that commands/make.rs can stop importing domain::ConfidenceSignal and domain::CommitHash directly.
> These are the final pieces of the usecase boundary surface that must be in place before the CLI migration tasks can be completed.

- [x] **T008**: Add pre-commit type-signals and commit-hash persistence boundary types, services, and interactors to usecase layer: PreCommitTypeSignalsService, PreCommitTypeSignalsOutput, PreCommitTypeSignalsError, PreCommitTypeSignalsInteractor, CommitHashPersistenceService, CommitHashPersistenceError, CommitHashPersistenceInteractor (`9d895b683c071409282e9e85af812268a17d91ed`)

### S7 — Remove Usecase pub-use Re-exports

> Scan the usecase crate public API for any remaining pub use domain:: re-exports (e.g. track_phase.rs) and remove them. This eliminates explicit re-export leakage and satisfies AC-04 before the CLI migration tasks begin touching the cli crate. Note: CN-01 as applied here targets `pub use domain::` re-exports only; certain domain types may still appear in usecase public signatures as accepted CN-01 exceptions documented in spec.json IN-08/IN-09/IN-10.

- [x] **T009**: Remove pub use domain:: re-exports from usecase layer public API (e.g. libs/usecase/src/track_phase.rs) to satisfy CN-01 and AC-04 (`9732b4b84b85c7f5a39d8395588825bf4c07c805`)

### S8 — CLI Migration — Guard, Hook, and Domain Export Commands

> Replace all use domain:: imports in commands/guard.rs, commands/hook.rs, and commands/domain.rs with the usecase boundary types introduced in S1 and S2. Wire the new interactors into the composition root for each subcommand.

- [x] **T010**: Migrate CLI guard, hook, and domain-export commands to usecase API: commands/guard.rs, commands/hook.rs, commands/domain.rs — replace all use domain:: imports with usecase boundary types (`3ec50dd3f4c9d8a7a5611c37219e5c794f2975dc`)

### S9 — CLI Migration — Track and Make Commands

> Replace all use domain:: imports in commands/track/ (activate.rs, resolve.rs, signals.rs, state_ops.rs, transition.rs, views.rs, tddd/) and commands/make.rs with the usecase boundary types introduced in S4, S5, and S6. Wire new interactors into the composition root.

- [x] **T011**: Migrate CLI track and make commands to usecase API: commands/track/ (activate, resolve, signals, state_ops, transition, views, tddd/), commands/make.rs — replace all use domain:: imports with usecase boundary types (`3ec50dd3f4c9d8a7a5611c37219e5c794f2975dc`)

### S10 — CLI Migration — Review and Verify Commands

> Replace all use domain:: imports in commands/review/ (codex_local.rs, classify.rs, files.rs, results.rs), commands/verify.rs, commands/verify_catalogue_spec_refs.rs, and commands/plan/ with the usecase boundary types introduced in S3 and S5. For classify.rs and files.rs: replace the domain-typed ScopeQueryService methods with string-accepting classify_by_strings (returning Result<Vec<ScopeClassificationOutput>, ScopeQueryError>) and files_by_string (returning Result<Vec<String>, ScopeQueryError>), so CLI command handlers never import domain::review_v2::FilePath, domain::review_v2::ScopeName, or domain::review_v2::MainScopeName directly. For verify.rs: add AdrVerifyOutput DTO to the usecase layer and modify VerifyAdrSignals::verify (service trait and VerifyAdrSignalsInteractor impl) to return Result<AdrVerifyOutput, VerifyAdrSignalsError> instead of Result<domain::AdrVerifyReport, VerifyAdrSignalsError>, so commands/verify.rs never imports domain::AdrVerifyReport. Wire new interactors and updated VerifyAdrSignals service.

- [x] **T012**: Migrate CLI review and verify commands to usecase API: commands/review/ (codex_local, classify, files, results), commands/verify.rs, commands/verify_catalogue_spec_refs.rs, commands/plan/ — replace all use domain:: imports with usecase boundary types. Includes: (a) replacing the domain-typed ScopeQueryService::classify and ::files methods with string-accepting ScopeQueryService::classify_by_strings (accepting Vec<String>, returning Result<Vec<ScopeClassificationOutput>, ScopeQueryError>) and ScopeQueryService::files_by_string (accepting String scope name, returning Result<Vec<String>, ScopeQueryError>) in the ScopeQueryInteractor, so commands/review/classify.rs and commands/review/files.rs never import domain::review_v2::FilePath, domain::review_v2::ScopeName, or domain::review_v2::MainScopeName; (b) adding AdrVerifyOutput DTO (new usecase-owned struct wrapping blue_count, yellow_count, red_count, grandfathered_count as usize) to the usecase layer; (c) modifying VerifyAdrSignals::verify return type from Result<domain::AdrVerifyReport, VerifyAdrSignalsError> to Result<AdrVerifyOutput, VerifyAdrSignalsError> and updating VerifyAdrSignalsInteractor::verify implementation accordingly, so that commands/verify.rs can use AdrVerifyOutput instead of importing domain::AdrVerifyReport. (`3ec50dd3f4c9d8a7a5611c37219e5c794f2975dc`)

### S11 — CLI Error Type Cleanup

> Remove the five domain error #[from] conversion variants from CliError in cli/src/error.rs and replace them with CliError::Message conversions from usecase error types. Confirm no domain type reference remains anywhere in error.rs.

- [x] **T013**: Remove domain error #[from] variants from cli/src/error.rs CliError, replacing them with CliError::Message conversions from usecase error types; confirm no remaining domain type reference in error.rs (`3ec50dd3f4c9d8a7a5611c37219e5c794f2975dc`)

### S12 — Layer Policy Enforcement

> Update the architecture-rules.json SSoT and apps/cli/Cargo.toml to remove the domain dependency from the cli layer. Originally planned last (S12), T001 was bundled with T010-T013 because removing `use domain::` from CLI source before deleting the Cargo.toml domain dependency caused `cargo make deny` wrapper-match failures (CI required atomic removal). Once T001 is applied, the cargo make check-layers and cargo make deny gates are live and the build is clean because T002-T013 have replaced all domain imports.

- [x] **T001**: Update architecture-rules.json to remove domain from apps/cli may_depend_on, and delete domain dependency from apps/cli/Cargo.toml (`3ec50dd3f4c9d8a7a5611c37219e5c794f2975dc`)

### S13 — Final Integration Gate

> Run the full CI suite to confirm all enforcement gates pass: cargo make check-layers (no cli→domain dependency detected), cargo make deny (no disallowed crate dependency), bin/sotp verify layers, and cargo make ci (fmt-check + clippy + nextest + deny + check-layers + verify-*). Confirm AC-01 through AC-10 are met.

- [x] **T014**: Final integration gate: run cargo make check-layers, cargo make deny, bin/sotp verify layers, and cargo make ci — verify all pass with zero cli→domain direct references
