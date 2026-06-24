<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CheckApprovedOutcome | enum | — | NoPairs, AllApproved, NotApproved | 🔵 | 🔵 |
| HookVerdictDecision | enum | modify | Allow, Block | 🔵 | 🔵 |
| PrReviewPollingOutput | enum | — | ReviewFound, ZeroFindings, Timeout | 🔵 | 🔵 |
| RefVerifyCheckApprovedOutcome | enum | — | NoPairs, AllApproved, NotApproved | 🔵 | 🔵 |
| RefVerifyRunOutcome | enum | — | Passed, SemanticFailuresConfirmed, HumanEscalationRequired | 🔵 | 🔵 |
| ReviewRoundType | enum | modify | Fast, Final | 🔵 | 🔵 |
| SignalGateName | enum | — | Commit, Merge | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchPortError | error_type | — | Unavailable | 🔵 | 🔵 |
| ArchivedTrackTelemetryError | error_type | — | Io, Serialize | 🟡 | 🔵 |
| ChainRunnerError | error_type | — | ExecutionFailed | 🔵 | 🔵 |
| CodeFragmentExtractorError | error_type | — | ExtractionFailed | 🔵 | 🔵 |
| ConventionsPortError | error_type | — | Unavailable | 🔵 | 🔵 |
| D4OrchestrationError | error_type | — | DiffFragment, DryGate, PrPolling | 🔵 | 🔵 |
| DemoPortError | error_type | — | Unavailable | 🔵 | 🔵 |
| DiffBaseResolverError | error_type | — | Unavailable | 🔵 | 🔵 |
| DryCheckSharedError | error_type | — | InvalidContentHash, InvalidSourcePath | 🔵 | 🔵 |
| DryCorpusMetaError | error_type | — | Unavailable | 🔵 | 🔵 |
| ExportSchemaError | error_type | modify | ExportFailed, SerializationFailed, FileWriteFailed | 🔵 | 🔵 |
| FilePortError | error_type | — | Unavailable | 🔵 | 🔵 |
| GitWorkflowError | error_type | modify | Validation, NoBranch, DetachedHead, BranchMismatch, Message, Unavailable | 🔵 | 🔵 |
| HookDispatchError | error_type | modify | UnknownHookName, HandlerFailed | 🔵 | 🔵 |
| PrGhApiError | error_type | — | ApiFailure | 🔵 | 🔵 |
| PrRepoNwoError | error_type | — | Unavailable | 🔵 | 🔵 |
| RefVerifyDriverError | error_type | — | Unavailable, Wiring, Usecase | 🔵 | 🔵 |
| ReviewAuxError | error_type | — | Failed | 🔵 | 🔵 |
| ReviewRoundTypeError | error_type | — | InvalidValue | 🔵 | 🔵 |
| ReviewWorkflowError | error_type | modify | Serialize, Validation | 🔵 | 🔵 |
| SchemaExporterError | error_type | — | ExportFailed | 🔵 | 🔵 |
| ShellParserError | error_type | — | ParseFailed | 🔵 | 🔵 |
| SignalGateError | error_type | — | ChainExecutionFailed, InvalidTrackId, StrictnessConfigLoad | 🔵 | 🔵 |
| SpecAdrSignalError | error_type | — | Read, Decode, Encode, Write | 🔵 | 🔵 |
| TelemetryAggregateServiceError | error_type | — | ReportUnavailable, EmitUnavailable | 🔵 | 🔵 |
| TelemetryEmitDynamicPortError | error_type | — | EmitUnavailable | 🔵 | 🔵 |
| TelemetryReportError | error_type | — | TrackNotFound, ReportUnavailable | 🔵 | 🔵 |
| TelemetryReportServiceError | error_type | — | Unavailable | 🔵 | 🔵 |
| VerifyPortError | error_type | — | Unavailable | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AdrChainRunnerPort | secondary_port | — | fn run_adr_chain(&self, project_root: std::path::PathBuf, strict: bool) -> Result<SignalChainOutput, ChainRunnerError> | 🔵 | 🔵 |
| ArchPort | secondary_port | — | fn render_tree(&self, project_root: &std::path::Path) -> Result<String, ArchPortError>, fn render_tree_full(&self, project_root: &std::path::Path) -> Result<String, ArchPortError>, fn render_members(&self, project_root: &std::path::Path) -> Result<String, ArchPortError>, fn render_direct_checks(&self, project_root: &std::path::Path) -> Result<String, ArchPortError> | 🔵 | 🔵 |
| ArchivedTrackTelemetryPort | secondary_port | — | fn emit(&self, track_id: String, subcommand: String, exit_code: i32, duration_ms: u64) -> Result<(), ArchivedTrackTelemetryError> | 🔵 | 🔵 |
| BranchReaderPort | secondary_port | reference | fn current_branch(&self) -> Result<Option<String>, BranchReadError> | 🔵 | 🔵 |
| CodeFragmentExtractorPort | secondary_port | — | fn extract(&self, workspace_root: &std::path::Path) -> Result<Vec<domain::semantic_dup::CodeFragment>, CodeFragmentExtractorError> | 🔵 | 🔵 |
| ConventionsPort | secondary_port | — | fn add_convention(&self, root: &std::path::Path, name: &str, slug: Option<&str>, title: Option<&str>, summary: Option<&str>) -> Result<String, ConventionsPortError>, fn update_index(&self, root: &std::path::Path) -> Result<String, ConventionsPortError>, fn verify_index(&self, root: &std::path::Path) -> Result<VerifyIndexResult, ConventionsPortError> | 🔵 | 🔵 |
| DemoPort | secondary_port | — | fn run(&self) -> Result<String, DemoPortError> | 🔵 | 🔵 |
| DiffBaseResolverPort | secondary_port | — | fn resolve_diff_base(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, repo_root: &std::path::Path) -> Result<domain::CommitHash, DiffBaseResolverError> | 🔵 | 🔵 |
| DryApprovalFactoryPort | secondary_port | — | fn build_approval(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, dry_config: usecase::dry_check::DryCheckConfig, config_fingerprint: domain::dry_check::DryCheckConfigFingerprint, corpus_fingerprint: domain::dry_check::DryCheckCorpusFingerprint) -> std::sync::Arc<dyn usecase::dry_check::DryCheckApprovalService + Send + Sync> | 🔵 | 🔵 |
| DryCheckAgentPort | secondary_port | reference | fn judge(&self, tier: DryCheckJudgeTier, low_path: std::path::PathBuf, high_path: std::path::PathBuf) -> Result<DryCheckAgentJudgment, DryCheckAgentError> | 🔵 | 🔵 |
| DryCheckApprovalService | secondary_port | reference | fn check_approved(&self, current_refs: std::collections::BTreeSet<domain::dry_check::FragmentRef>, approval: domain::dry_check::DryCheckApprovalVerdict) -> Result<DryCheckApprovalVerdict, DryCheckCycleError> | 🔵 | 🔵 |
| DryCheckDiffSource | secondary_port | modify | fn list_changed_hunks(&self, base: &domain::CommitHash, repo_root: &std::path::Path) -> Result<Vec<domain::dry_check::DiffFileHunks>, crate::dry_check::errors::DryCheckDiffError> | 🔵 | 🔵 |
| DryCorpusMetaPort | secondary_port | — | fn resolve_corpus_meta(&self, track_dir: &std::path::Path, canonical_root: &std::path::Path, repo_root: &std::path::Path) -> Result<(std::path::PathBuf, domain::dry_check::DryCheckCorpusFingerprint), DryCorpusMetaError> | 🔵 | 🔵 |
| DryDriverPort | secondary_port | — | fn dry_write(&self, input: DryWriteDriverInput) -> DryDriverOutcome, fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome, fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryDriverOutcome, fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome | 🔵 | 🔵 |
| FileWritePort | secondary_port | — | fn write_atomic(&self, path: &std::path::Path, content: &[u8]) -> Result<(), FilePortError> | 🔵 | 🔵 |
| GitWorkflowService | secondary_port | — | fn stage_all(&self) -> GitWorkflowResult<()>, fn stage_from_file(&self, path: &std::path::Path, cleanup: bool) -> GitWorkflowResult<()>, fn commit_from_file(&self, path: &std::path::Path, cleanup: bool, track_dir: Option<&std::path::Path>) -> GitWorkflowResult<()>, fn note_from_file(&self, path: &std::path::Path, cleanup: bool) -> GitWorkflowResult<()>, fn switch_and_pull(&self, branch: &str) -> GitWorkflowResult<String>, fn unstage(&self, paths: &[std::path::PathBuf]) -> GitWorkflowResult<()>, fn current_branch_track_id(&self) -> GitWorkflowResult<Option<String>> | 🔵 | 🔵 |
| LayerChainRunnerPort | secondary_port | — | fn run_catalog_spec_chain(&self, strict: bool, signal_reader: &dyn usecase::signal::SignalLayerReader) -> Result<SignalChainOutput, ChainRunnerError>, fn run_impl_catalog_chain(&self, strict: bool, signal_reader: &dyn usecase::signal::SignalLayerReader) -> Result<SignalChainOutput, ChainRunnerError> | 🔵 | 🔵 |
| PrListIssueCommentsPort | secondary_port | — | fn list_issue_comments(&self, repo_nwo: &str, pr: &str) -> Result<String, PrGhApiError> | 🔵 | 🔵 |
| PrListReactionsPort | secondary_port | — | fn list_reactions(&self, repo_nwo: &str, pr: &str) -> Result<String, PrGhApiError> | 🔵 | 🔵 |
| PrListReviewsPort | secondary_port | — | fn list_reviews(&self, repo_nwo: &str, pr: &str) -> Result<String, PrGhApiError> | 🔵 | 🔵 |
| PrRepoNwoPort | secondary_port | — | fn repo_nwo(&self) -> Result<String, PrRepoNwoError> | 🔵 | 🔵 |
| RefVerifyAggregateService | secondary_port | — | fn run(&self, track_id: &str, items_dir: &std::path::Path) -> Result<RefVerifyRunOutcome, RefVerifyDriverError>, fn check_approved(&self, track_id: &str, items_dir: &std::path::Path) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError> | 🔵 | 🔵 |
| RefVerifyCheckApprovedDriverService | secondary_port | — | fn check_approved(&self, track_id: &str, items_dir: &std::path::Path) -> Result<RefVerifyCheckApprovedOutcome, RefVerifyDriverError> | 🔵 | 🔵 |
| RefVerifyGateStatePort | secondary_port | reference | fn ref_verify_status(&self) -> Result<RefVerifyGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| RefVerifyRunService | secondary_port | — | fn run(&self, track_id: &str, items_dir: &std::path::Path) -> Result<RefVerifyRunOutcome, RefVerifyDriverError> | 🔵 | 🔵 |
| ReviewGateStatePort | secondary_port | reference | fn review_status(&self) -> Result<ReviewGateStatus, FixpointResolveError> | 🔵 | 🔵 |
| SchemaExporterPort | secondary_port | modify | fn export_as_json(&self, crate_name: &str) -> Result<String, SchemaExporterError> | 🔵 | 🔵 |
| SemanticDupDriverPort | secondary_port | — | fn find_similar(&self, input: FindSimilarDriverInput) -> SemanticDupDriverOutcome, fn index_build(&self, input: IndexBuildDriverInput) -> SemanticDupDriverOutcome, fn index_measure_quality(&self, input: IndexMeasureQualityDriverInput) -> SemanticDupDriverOutcome, fn dup_check(&self, input: DupCheckDriverInput) -> SemanticDupDriverOutcome | 🔵 | 🔵 |
| SemanticIndexPort | secondary_port | reference | fn insert(&self, path: std::path::PathBuf, content: String) -> Result<(), SemanticIndexError>, fn insert_batch(&self, items: Vec<(std::path::PathBuf, String)>) -> Result<(), SemanticIndexError>, fn delete_by_source_path(&self, path: std::path::PathBuf) -> Result<(), SemanticIndexError>, fn search(&self, query: String, limit: usize) -> Result<Vec<domain::semantic_dup::SearchResult>, SemanticIndexError> | 🔵 | 🔵 |
| ShellParserPort | secondary_port | modify | fn split_shell(&self, input: &str) -> Result<Vec<String>, ShellParserError> | 🔵 | 🔵 |
| SleepPort | secondary_port | — | fn sleep(&self, duration: std::time::Duration) -> () | 🔵 | 🔵 |
| SpecAdrChainRunnerPort | secondary_port | — | fn run_spec_adr_chain(&self, spec_json_path: std::path::PathBuf, strict: bool) -> Result<SignalChainOutput, ChainRunnerError> | 🔵 | 🔵 |
| SpecFileWriterPort | secondary_port | — | fn read_spec_json(&self, path: std::path::PathBuf) -> Result<domain::SpecDocument, SpecAdrSignalError>, fn write_spec_json(&self, path: std::path::PathBuf, doc: &domain::SpecDocument) -> Result<(), SpecAdrSignalError> | 🔵 | 🔵 |
| TelemetryEmitDynamicPort | secondary_port | — | fn emit_archived(&self, items_dir: &std::path::Path, track_id: &str, subcommand: String, exit_code: i32, duration_ms: u64) -> Result<(), TelemetryEmitDynamicPortError> | 🔵 | 🔵 |
| TelemetryReportPort | secondary_port | — | fn aggregate(&self, track_id: &str, items_dir: &std::path::Path) -> Result<TelemetryReportOutput, TelemetryReportError> | 🔵 | 🔵 |
| VerifyPort | secondary_port | — | fn verify_tech_stack(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_latest_track(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_arch_docs(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_layers(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_hooks_path(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_canonical_modules(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_module_size(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_domain_purity(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_domain_strings(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_usecase_purity(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_doc_links(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_view_freshness(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_plan_artifact_refs(&self, track_dir: Option<&std::path::Path>) -> VerifyOutcome, fn verify_catalogue_spec_refs(&self, track_id: Option<&str>, items_dir: &std::path::Path, workspace_root: &std::path::Path, skip_stale: bool) -> VerifyOutcome | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchService | application_service | — | fn render_tree(&self, project_root: &std::path::Path) -> Result<String, ArchPortError>, fn render_tree_full(&self, project_root: &std::path::Path) -> Result<String, ArchPortError>, fn render_members(&self, project_root: &std::path::Path) -> Result<String, ArchPortError>, fn render_direct_checks(&self, project_root: &std::path::Path) -> Result<String, ArchPortError> | 🟡 | 🔵 |
| ArchivedTrackTelemetryService | application_service | — | fn emit(&self, cmd: ArchivedTrackTelemetryCommand) -> Result<(), ArchivedTrackTelemetryError> | 🔵 | 🔵 |
| ConventionsService | application_service | — | fn add_convention(&self, root: &std::path::Path, name: &str, slug: Option<&str>, title: Option<&str>, summary: Option<&str>) -> Result<String, ConventionsPortError>, fn update_index(&self, root: &std::path::Path) -> Result<String, ConventionsPortError>, fn verify_index(&self, root: &std::path::Path) -> Result<VerifyIndexResult, ConventionsPortError> | 🟡 | 🔵 |
| DemoService | application_service | — | fn run(&self) -> Result<String, DemoPortError> | 🔵 | 🔵 |
| DryDriverService | application_service | — | fn dry_write(&self, input: DryWriteDriverInput) -> DryDriverOutcome, fn dry_results(&self, input: DryResultsDriverInput) -> DryDriverOutcome, fn dry_check_approved(&self, input: DryCheckApprovedDriverInput) -> DryDriverOutcome, fn dry_fix_local(&self, input: DryFixLocalDriverInput) -> DryDriverOutcome | 🔵 | 🔵 |
| DryFragmentPipelineService | application_service | — | fn derive_current_refs(&self, cmd: DryFragmentPipelineCommand) -> Result<DryFragmentPipelineOutput, D4OrchestrationError> | 🔵 | 🔵 |
| FixpointDryGateService | application_service | — | fn resolve_dry_gate(&self, cmd: FixpointDryGateCommand) -> Result<FixpointDryGateOutput, D4OrchestrationError> | 🔵 | 🔵 |
| HookDispatchService | application_service | modify | fn dispatch(&self, hook_name: String, command: HookDispatchCommand) -> Result<HookVerdictOutput, HookDispatchError>, fn check_skill_compliance(&self, prompt: &str) -> Option<String> | 🔵 | 🔵 |
| PrCommandService | application_service | — | fn push(&self, track_id: Option<String>) -> PrCommandOutput, fn ensure(&self, track_id: Option<String>, base: String) -> PrCommandOutput, fn status(&self, pr: String) -> PrCommandOutput, fn wait_and_merge(&self, pr: String, interval: u64, timeout: u64, method: String) -> PrCommandOutput, fn trigger_review(&self, pr: String) -> PrCommandOutput, fn poll_review(&self, pr: String, trigger_timestamp: String, interval: u64, timeout: u64) -> PrCommandOutput, fn review_cycle(&self, track_id: Option<String>, resume: bool) -> PrCommandOutput | 🔵 | 🔵 |
| PrReviewPollingService | application_service | — | fn poll(&self, cmd: PrReviewPollingCommand) -> Result<PrReviewPollingOutput, D4OrchestrationError> | 🔵 | 🔵 |
| RefVerifyCheckApprovedService | application_service | — | fn check_approved(&self, cmd: &usecase::ref_verify::RefVerifyCommand) -> Result<CheckApprovedOutcome, usecase::ref_verify::RefVerifyError> | 🔵 | 🔵 |
| ReviewClassifyService | application_service | — | fn classify(&self, paths: Vec<String>, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<(String, String)>, ReviewAuxError> | 🔵 | 🔵 |
| ReviewFilesService | application_service | — | fn files(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<String>, ReviewAuxError> | 🔵 | 🔵 |
| ReviewGetBriefingService | application_service | — | fn get_briefing(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Option<String>, ReviewAuxError> | 🔵 | 🔵 |
| ReviewResultsService | application_service | — | fn results(&self, track_id: Option<String>, items_dir: std::path::PathBuf, scope: Option<String>, all: bool, limit: u32, round_type: String, no_hint: bool) -> Result<String, ReviewAuxError> | 🔵 | 🔵 |
| ReviewRunLocalService | application_service | — | fn run_local(&self, model: Option<String>, timeout_seconds: u64, briefing_file: Option<std::path::PathBuf>, prompt: Option<String>, track_id: Option<String>, round_type: String, group: String, items_dir: std::path::PathBuf) -> ReviewRunLocalOutput | 🔵 | 🔵 |
| ReviewService | application_service | — | fn run_codex(&self, input: ReviewRunInput) -> Result<usecase::review_v2::RunReviewOutput, usecase::review_v2::RunReviewError>, fn run_claude(&self, input: ReviewRunInput) -> Result<usecase::review_v2::RunReviewOutput, usecase::review_v2::RunReviewError>, fn run_local(&self, model: Option<String>, timeout_seconds: u64, briefing_file: Option<std::path::PathBuf>, prompt: Option<String>, track_id: Option<String>, round_type: String, group: String, items_dir: std::path::PathBuf) -> ReviewRunLocalOutput, fn run_fix_local(&self, input: ReviewRunFixInput) -> Result<usecase::review_v2::RunReviewFixOutput, usecase::review_v2::RunReviewFixError>, fn check_approved(&self, track_id: String, items_dir: std::path::PathBuf) -> Result<usecase::review_v2::ReviewApprovalOutput, usecase::review_v2::ReviewCheckApprovedError>, fn results(&self, track_id: Option<String>, items_dir: std::path::PathBuf, scope: Option<String>, all: bool, limit: u32, round_type: String, no_hint: bool) -> Result<String, ReviewAuxError>, fn classify(&self, paths: Vec<String>, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<(String, String)>, ReviewAuxError>, fn files(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Vec<String>, ReviewAuxError>, fn validate_scope(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<(), ReviewAuxError>, fn get_briefing(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<Option<String>, ReviewAuxError>, fn persist_commit_hash(&self, track_id: String, workspace_root: std::path::PathBuf) -> Result<String, usecase::commit_hash_persistence::CommitHashPersistenceError> | 🔵 | 🔵 |
| ReviewValidateScopeService | application_service | — | fn validate_scope(&self, scope: String, track_id: Option<String>, items_dir: std::path::PathBuf) -> Result<(), ReviewAuxError> | 🔵 | 🔵 |
| SemanticDupDriverService | application_service | — | fn find_similar(&self, input: FindSimilarDriverInput) -> SemanticDupDriverOutcome, fn index_build(&self, input: IndexBuildDriverInput) -> SemanticDupDriverOutcome, fn index_measure_quality(&self, input: IndexMeasureQualityDriverInput) -> SemanticDupDriverOutcome, fn dup_check(&self, input: DupCheckDriverInput) -> SemanticDupDriverOutcome | 🔵 | 🔵 |
| SignalGateService | application_service | — | fn run_gate(&self, cmd: SignalGateCommand) -> Result<SignalGateOutput, SignalGateError> | 🔵 | 🔵 |
| SignalService | application_service | — | fn calc_adr_user(&self, project_root: std::path::PathBuf) -> SignalCommandOutput, fn check_adr_user(&self, project_root: std::path::PathBuf, strict_override: bool, gate: Option<SignalGateName>, workspace_root: Option<std::path::PathBuf>) -> SignalCommandOutput, fn calc_spec_adr(&self, spec_json_path: Option<std::path::PathBuf>, workspace_root: Option<std::path::PathBuf>) -> SignalCommandOutput, fn check_spec_adr(&self, spec_json_path: Option<std::path::PathBuf>, strict_override: bool, gate: Option<SignalGateName>, workspace_root: Option<std::path::PathBuf>) -> SignalCommandOutput, fn calc_catalog_spec(&self) -> SignalCommandOutput, fn check_catalog_spec(&self, strict_override: bool, gate: Option<SignalGateName>, workspace_root: Option<std::path::PathBuf>) -> SignalCommandOutput, fn calc_impl_catalog(&self) -> SignalCommandOutput, fn check_impl_catalog(&self, strict_override: bool, gate: Option<SignalGateName>, workspace_root: Option<std::path::PathBuf>) -> SignalCommandOutput, fn check_gate(&self, project_root: Option<std::path::PathBuf>, spec_json_path: Option<std::path::PathBuf>, gate: SignalGateName, workspace_root: Option<std::path::PathBuf>) -> SignalCommandOutput | 🔵 | 🔵 |
| SpecAdrSignalService | application_service | — | fn calc_and_persist(&self, cmd: SpecAdrSignalCommand) -> Result<SpecAdrSignalOutput, SpecAdrSignalError> | 🔵 | 🔵 |
| TelemetryAggregateService | application_service | — | fn report(&self, track_id: &str, items_dir: &std::path::Path) -> Result<String, TelemetryAggregateServiceError>, fn emit_archived(&self, items_dir: &std::path::Path, track_id: &str, subcommand: String, exit_code: i32, duration_ms: u64) -> Result<(), TelemetryAggregateServiceError> | 🔵 | 🔵 |
| TelemetryReportService | application_service | — | fn report(&self, track_id: &str, items_dir: &std::path::Path) -> Result<String, TelemetryReportServiceError> | 🔵 | 🔵 |
| TrackService | application_service | — | fn init(&self, items_dir: std::path::PathBuf, track_id: String, description: String) -> TrackCommandOutput, fn transition(&self, items_dir: std::path::PathBuf, track_id: Option<String>, task_id: String, target_status: String, commit_hash: Option<String>) -> TrackCommandOutput, fn resolve(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn views_sync(&self, project_root: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn archive(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn detect_active(&self, project_root: std::path::PathBuf) -> TrackCommandOutput | 🟡 | 🔵 |
| VerifyService | application_service | — | fn verify_tech_stack(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_latest_track(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_arch_docs(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_layers(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_hooks_path(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_canonical_modules(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_module_size(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_domain_purity(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_domain_strings(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_usecase_purity(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_doc_links(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_view_freshness(&self, project_root: &std::path::Path) -> VerifyOutcome, fn verify_plan_artifact_refs(&self, track_dir: Option<&std::path::Path>) -> VerifyOutcome, fn verify_catalogue_spec_refs(&self, track_id: Option<&str>, items_dir: &std::path::Path, workspace_root: &std::path::Path, skip_stale: bool) -> VerifyOutcome | 🟡 | 🔵 |

## Use Cases

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FixpointDryGateOutput | use_case | — | — | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchInteractor | interactor | — | — | 🟡 | 🔵 |
| ArchivedTrackTelemetryInteractor | interactor | — | — | 🔵 | 🔵 |
| ConventionsInteractor | interactor | — | — | 🟡 | 🔵 |
| DemoInteractor | interactor | — | — | 🟡 | 🔵 |
| DryDriverInteractor | interactor | — | — | 🟡 | 🔵 |
| DryFragmentPipelineInteractor | interactor | — | — | 🔵 | 🔵 |
| ExportSchemaInteractor | interactor | modify | — | 🔵 | 🔵 |
| FixpointDryGateInteractor | interactor | — | — | 🔵 | 🔵 |
| GitWorkflowInteractor | interactor | — | — | 🟡 | 🔵 |
| HookDispatchInteractor | interactor | modify | — | 🔵 | 🔵 |
| PrCommandInteractor | interactor | — | — | 🟡 | 🔵 |
| PrReviewPollingInteractor | interactor | — | — | 🔵 | 🔵 |
| RefVerifyAggregateInteractor | interactor | — | — | 🟡 | 🔵 |
| RefVerifyCheckApprovedInteractor | interactor | — | — | 🟡 | 🔵 |
| ReviewClassifyInteractor | interactor | — | — | 🟡 | 🔵 |
| ReviewFilesInteractor | interactor | — | — | 🟡 | 🔵 |
| ReviewGetBriefingInteractor | interactor | — | — | 🟡 | 🔵 |
| ReviewResultsInteractor | interactor | — | — | 🟡 | 🔵 |
| ReviewRunLocalInteractor | interactor | — | — | 🟡 | 🔵 |
| ReviewValidateScopeInteractor | interactor | — | — | 🟡 | 🔵 |
| SemanticDupDriverInteractor | interactor | — | — | 🟡 | 🔵 |
| SignalGateInteractor | interactor | — | — | 🔵 | 🔵 |
| SpecAdrSignalInteractor | interactor | — | — | 🔵 | 🔵 |
| TelemetryAggregateInteractor | interactor | — | — | 🟡 | 🔵 |
| TelemetryReportInteractor | interactor | — | — | 🟡 | 🔵 |
| VerifyInteractor | interactor | — | — | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryCheckApprovedDriverInput | dto | — | — | 🔵 | 🔵 |
| DryDriverOutcome | dto | — | — | 🟡 | 🔵 |
| DryFixLocalDriverInput | dto | — | — | 🔵 | 🔵 |
| DryFragmentPipelineOutput | dto | — | — | 🔵 | 🔵 |
| DryResultsDriverInput | dto | — | — | 🔵 | 🔵 |
| DryWriteDriverInput | dto | — | — | 🔵 | 🔵 |
| DupCheckDriverInput | dto | — | — | 🔵 | 🔵 |
| FindSimilarDriverInput | dto | — | — | 🔵 | 🔵 |
| GitWorkflowResult | dto | — | — | 🟡 | 🔵 |
| HookVerdictOutput | dto | modify | — | 🔵 | 🔵 |
| IndexBuildDriverInput | dto | — | — | 🔵 | 🔵 |
| IndexMeasureQualityDriverInput | dto | — | — | 🔵 | 🔵 |
| PrCommandOutput | dto | — | — | 🟡 | 🔵 |
| ReviewRunFixInput | dto | — | — | 🔵 | 🔵 |
| ReviewRunInput | dto | — | — | 🔵 | 🔵 |
| ReviewRunLocalOutput | dto | — | — | 🔵 | 🔵 |
| SemanticDupDriverOutcome | dto | — | — | 🟡 | 🔵 |
| SignalChainOutput | dto | — | — | 🔵 | 🔵 |
| SignalCommandOutput | dto | — | — | 🟡 | 🔵 |
| SignalGateOutput | dto | — | — | 🔵 | 🔵 |
| SpecAdrSignalOutput | dto | — | — | 🔵 | 🔵 |
| TelemetryErrorEntry | dto | — | — | 🔵 | 🔵 |
| TelemetryHookBlockEntry | dto | — | — | 🔵 | 🔵 |
| TelemetryPhaseDuration | dto | — | — | 🔵 | 🔵 |
| TelemetryReportOutput | dto | — | — | 🔵 | 🔵 |
| TrackCommandOutput | dto | — | — | 🟡 | 🔵 |
| VerifyIndexResult | dto | — | — | 🔵 | 🔵 |
| VerifyOutcome | dto | — | — | 🟡 | 🔵 |

## Commands

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchivedTrackTelemetryCommand | command | — | — | 🔵 | 🔵 |
| DryFragmentPipelineCommand | command | — | — | 🔵 | 🔵 |
| ExportSchemaCommand | command | modify | — | 🔵 | 🔵 |
| FixpointDryGateCommand | command | — | — | 🔵 | 🔵 |
| HookDispatchCommand | command | modify | — | 🔵 | 🔵 |
| PrReviewPollingCommand | command | — | — | 🔵 | 🔵 |
| SignalGateCommand | command | — | — | 🔵 | 🔵 |
| SpecAdrSignalCommand | command | — | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| usecase::dry_check::shared::fragment_ref_of | free_function | modify | fn(fragment: &domain::semantic_dup::CodeFragment) -> Result<domain::dry_check::FragmentRef, DryCheckSharedError> | 🔵 | 🔵 |
| usecase::telemetry::format_report | free_function | — | fn(track_id: &str, output: &TelemetryReportOutput) -> String | 🔵 | 🔵 |

