<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
flowchart LR
    classDef secondary_adapter fill:#fafafa,stroke:#999,stroke-dasharray: 4 4
    classDef command fill:#e3f2fd,stroke:#1976d2
    classDef query fill:#f3e5f5,stroke:#8e24aa
    classDef factory fill:#fff8e1,stroke:#f9a825
    classDef free_function fill:#f1f8e9,stroke:#558b2f
    classDef domain_service fill:#fce4ec,stroke:#c62828
    subgraph domain [domain]
        L6_domain_TrackId(TrackId)
        L6_domain_CommitHash(CommitHash)
        L6_domain_CatalogueLintViolation(CatalogueLintViolation)
        L6_domain_TrackReader[[TrackReader]]
        L6_domain_TrackWriteError>TrackWriteError]
        L6_domain_TypeSignal(TypeSignal)
        L6_domain_Decision{{Decision}}
    end
    subgraph usecase [usecase]
        L7_usecase_TrackStatusOutput[TrackStatusOutput]
        L7_usecase_GuardDecision{{GuardDecision}}
        L7_usecase_GuardCheckOutput[GuardCheckOutput]
        L7_usecase_GuardCheckService[/GuardCheckService\]
        L7_usecase_GuardCheckInteractor[\GuardCheckInteractor/]
        L7_usecase_ShellParserPort[[ShellParserPort]]
        L7_usecase_SchemaExporterPort[[SchemaExporterPort]]
        L7_usecase_ExportSchemaService[/ExportSchemaService\]
        L7_usecase_ExportSchemaCommand[ExportSchemaCommand]:::command
        L7_usecase_ExportSchemaError>ExportSchemaError]
        L7_usecase_ExportSchemaInteractor[\ExportSchemaInteractor/]
        L7_usecase_TaskOperationOutput[TaskOperationOutput]
        L7_usecase_ReviewApprovalOutput[ReviewApprovalOutput]
        L7_usecase_ReviewApprovalDecision{{ReviewApprovalDecision}}
        L7_usecase_ReviewCheckApprovedService[/ReviewCheckApprovedService\]
        L7_usecase_ReviewCheckApprovedError>ReviewCheckApprovedError]
        L7_usecase_ReviewCheckApprovedInteractor[\ReviewCheckApprovedInteractor/]
        L7_usecase_HookDispatchCommand[HookDispatchCommand]:::command
        L7_usecase_HookVerdictOutput[HookVerdictOutput]
        L7_usecase_HookVerdictDecision{{HookVerdictDecision}}
        L7_usecase_HookDispatchService[/HookDispatchService\]
        L7_usecase_HookDispatchError>HookDispatchError]
        L7_usecase_HookDispatchInteractor[\HookDispatchInteractor/]
        L7_usecase_ReviewRoundType{{ReviewRoundType}}
        L7_usecase_TrackPhaseOutput[TrackPhaseOutput]
        L7_usecase_TrackPhaseService[/TrackPhaseService\]
        L7_usecase_TrackPhaseError>TrackPhaseError]
        L7_usecase_TrackPhaseInteractor[\TrackPhaseInteractor/]
        L7_usecase_VerifyCatalogueConsistencyService[/VerifyCatalogueConsistencyService\]
        L7_usecase_VerifyCatalogueConsistencyOutput[VerifyCatalogueConsistencyOutput]
        L7_usecase_VerifyCatalogueConsistencyError>VerifyCatalogueConsistencyError]
        L7_usecase_VerifyCatalogueConsistencyInteractor[\VerifyCatalogueConsistencyInteractor/]
        L7_usecase_VerifyCatalogueSpecSignalsService[/VerifyCatalogueSpecSignalsService\]
        L7_usecase_VerifySpecSignalsOutput[VerifySpecSignalsOutput]
        L7_usecase_VerifySpecSignalsError>VerifySpecSignalsError]
        L7_usecase_VerifyCatalogueSpecSignalsInteractor[\VerifyCatalogueSpecSignalsInteractor/]
        L7_usecase_TypeSignalsService[/TypeSignalsService\]
        L7_usecase_LayerSignalSummary[LayerSignalSummary]
        L7_usecase_TypeSignalsError>TypeSignalsError]
        L7_usecase_TypeSignalsInteractor[\TypeSignalsInteractor/]
        L7_usecase_VerifyAdrSignals[/VerifyAdrSignals\]
        L7_usecase_VerifyAdrSignalsInteractor[\VerifyAdrSignalsInteractor/]
        L7_usecase_AdrVerifyOutput[AdrVerifyOutput]
        L7_usecase_ScopeQueryService[/ScopeQueryService\]
        L7_usecase_ScopeClassificationOutput[ScopeClassificationOutput]
        L7_usecase_ScopeQueryInteractor[\ScopeQueryInteractor/]
        L7_usecase_TaskOperationError>TaskOperationError]
        L7_usecase_TaskOperationInteractor[\TaskOperationInteractor/]
        L7_usecase_TaskTransitionCommand[TaskTransitionCommand]:::command
        L7_usecase_AddTaskCommand[AddTaskCommand]:::command
        L7_usecase_SetOverrideCommand[SetOverrideCommand]:::command
        L7_usecase_ClearOverrideCommand[ClearOverrideCommand]:::command
        L7_usecase_TaskOperationService[/TaskOperationService\]
        L7_usecase_TaskQueryService[/TaskQueryService\]
        L7_usecase_TaskQueryInteractor[\TaskQueryInteractor/]
        L7_usecase_NextTaskOutput[NextTaskOutput]
        L7_usecase_TaskCountsOutput[TaskCountsOutput]
        L7_usecase_PreCommitTypeSignalsService[/PreCommitTypeSignalsService\]
        L7_usecase_PreCommitTypeSignalsOutput[PreCommitTypeSignalsOutput]
        L7_usecase_PreCommitTypeSignalsError>PreCommitTypeSignalsError]
        L7_usecase_PreCommitTypeSignalsInteractor[\PreCommitTypeSignalsInteractor/]
        L7_usecase_RunReviewCommand[RunReviewCommand]:::command
        L7_usecase_RunReviewOutput[RunReviewOutput]
        L7_usecase_RunReviewError>RunReviewError]
        L7_usecase_RunReviewInteractor[\RunReviewInteractor/]
        L7_usecase_RunReviewService[/RunReviewService\]
        L7_usecase_VerifyCatalogueSpecRefsService[/VerifyCatalogueSpecRefsService\]
        L7_usecase_VerifyCatalogueSpecRefsOutput[VerifyCatalogueSpecRefsOutput]
        L7_usecase_VerifyCatalogueSpecRefsError>VerifyCatalogueSpecRefsError]
        L7_usecase_VerifyCatalogueSpecRefsInteractor[\VerifyCatalogueSpecRefsInteractor/]
        L7_usecase_CommitHashPersistenceService[/CommitHashPersistenceService\]
        L7_usecase_CommitHashPersistenceError>CommitHashPersistenceError]
        L7_usecase_CommitHashPersistenceInteractor[\CommitHashPersistenceInteractor/]
        L7_usecase_HookShellParserPort[[HookShellParserPort]]
        L7_usecase_ScopeQueryError>ScopeQueryError]
        L7_usecase_ActivateTrackUseCase[/ActivateTrackUseCase/]
        L7_usecase_RenderContractMapCommand[RenderContractMapCommand]:::command
        L7_usecase_RenderContractMapError>RenderContractMapError]
        L7_usecase_RenderContractMapOutput[RenderContractMapOutput]
        L7_usecase_RenderContractMap[/RenderContractMap\]
        L7_usecase_RenderContractMapInteractor[\RenderContractMapInteractor/]
        L7_usecase_LintRuleKind{{LintRuleKind}}
        L7_usecase_LintRuleSpec[LintRuleSpec]
        L7_usecase_RunCatalogueLintCommand[RunCatalogueLintCommand]:::command
        L7_usecase_RunCatalogueLintError>RunCatalogueLintError]
        L7_usecase_RunCatalogueLintInteractor[\RunCatalogueLintInteractor/]
        L7_usecase_RunCatalogueLint[/RunCatalogueLint\]
        L7_usecase_check__compliance__render[check_compliance_render]:::free_function
        L7_usecase_has__skill__command[has_skill_command]:::free_function
        L7_usecase_reject__branchless__guard__by__str[reject_branchless_guard_by_str]:::free_function
        L7_usecase_reject__branchless__implementation__transition__by__str[reject_branchless_implementation_transition_by_str]:::free_function
    end
    subgraph infrastructure [infrastructure]
        L14_infrastructure_FsTrackStore[FsTrackStore]:::secondary_adapter
        L14_infrastructure_RustdocSchemaExporter[RustdocSchemaExporter]:::secondary_adapter
        L14_infrastructure_ConchShellParser[ConchShellParser]:::secondary_adapter
        L14_infrastructure_NullDiffGetter[NullDiffGetter]:::secondary_adapter
        L14_infrastructure_NullReviewer[NullReviewer]:::secondary_adapter
        L14_infrastructure_CodexReviewOutcome{{CodexReviewOutcome}}
        L14_infrastructure_ReviewV2Composition[ReviewV2Composition]
        L14_infrastructure_ReviewV2CompositionWithCodex[ReviewV2CompositionWithCodex]
        L14_infrastructure_CaptureBaselineError[CaptureBaselineError]
        L14_infrastructure_EvaluateSignalsError[EvaluateSignalsError]
        L14_infrastructure_TypeGraphExportError[TypeGraphExportError]
        L14_infrastructure_SignalSummary[SignalSummary]
        L14_infrastructure_run__example__demo[run_example_demo]:::free_function
        L14_infrastructure_persist__commit__hash__for__track[persist_commit_hash_for_track]:::free_function
        L14_infrastructure_append__scope__briefing__reference__str[append_scope_briefing_reference_str]:::free_function
        L14_infrastructure_build__check__approved__service[build_check_approved_service]:::free_function
        L14_infrastructure_build__review__v2[build_review_v2]:::free_function
        L14_infrastructure_build__review__v2__str[build_review_v2_str]:::free_function
        L14_infrastructure_build__review__v2__with__reviewer[build_review_v2_with_reviewer]:::free_function
        L14_infrastructure_build__review__v2__with__reviewer__str[build_review_v2_with_reviewer_str]:::free_function
        L14_infrastructure_build__run__review__service[build_run_review_service]:::free_function
        L14_infrastructure_build__scope__query__interactor__no__diff__str[build_scope_query_interactor_no_diff_str]:::free_function
        L14_infrastructure_build__scope__query__interactor__str[build_scope_query_interactor_str]:::free_function
        L14_infrastructure_check__approved__str[check_approved_str]:::free_function
        L14_infrastructure_get__briefing__for__scope__str[get_briefing_for_scope_str]:::free_function
        L14_infrastructure_load__scope__config__only[load_scope_config_only]:::free_function
        L14_infrastructure_load__scope__config__only__str[load_scope_config_only_str]:::free_function
        L14_infrastructure_render__review__results__str[render_review_results_str]:::free_function
        L14_infrastructure_resolve__diff__base__and__getter[resolve_diff_base_and_getter]:::free_function
        L14_infrastructure_run__codex__review__str[run_codex_review_str]:::free_function
        L14_infrastructure_validate__review__group__name__str[validate_review_group_name_str]:::free_function
        L14_infrastructure_validate__scope__for__track__str[validate_scope_for_track_str]:::free_function
        L14_infrastructure_validate__track__id__str[validate_track_id_str]:::free_function
        L14_infrastructure_capture__baseline__for__layer[capture_baseline_for_layer]:::free_function
        L14_infrastructure_refresh__one__layer[refresh_one_layer]:::free_function
        L14_infrastructure_execute__type__graph__for__layer[execute_type_graph_for_layer]:::free_function
        L14_infrastructure_evaluate__and__write__signals[evaluate_and_write_signals]:::free_function
        L14_infrastructure_execute__type__signals__for__layer[execute_type_signals_for_layer]:::free_function
        L14_infrastructure_validate__action__diagnostics[validate_action_diagnostics]:::free_function
        L14_infrastructure_validate__and__write__catalogue[validate_and_write_catalogue]:::free_function
        L14_infrastructure_read__track__status__str[read_track_status_str]:::free_function
        L14_infrastructure_execute__verify__adr__signals[execute_verify_adr_signals]:::free_function
        L14_infrastructure_any__enabled__catalogue__present[any_enabled_catalogue_present]:::free_function
        L14_infrastructure_format__finding[format_finding]:::free_function
        L14_infrastructure_read__spec__element__hashes[read_spec_element_hashes]:::free_function
        L14_infrastructure_verify__one__layer__formatted[verify_one_layer_formatted]:::free_function
        L14_infrastructure_execute__catalogue__spec__signals[execute_catalogue_spec_signals]:::free_function
        L14_infrastructure_execute__catalogue__spec__signals__check[execute_catalogue_spec_signals_check]:::free_function
        L14_infrastructure_consistency__report__to__findings[consistency_report_to_findings]:::free_function
        L14_infrastructure_evaluate__consistency__from__components[evaluate_consistency_from_components]:::free_function
        L14_infrastructure_execute__spec__code__consistency__str[execute_spec_code_consistency_str]:::free_function
    end
    L14_infrastructure_ConchShellParser -.impl.-> L7_usecase_HookShellParserPort
    L14_infrastructure_ConchShellParser -.impl.-> L7_usecase_ShellParserPort
    L14_infrastructure_FsTrackStore -.impl.-> L6_domain_TrackReader
    L14_infrastructure_RustdocSchemaExporter -.impl.-> L7_usecase_SchemaExporterPort
    L14_infrastructure_build__check__approved__service -->|"returns"| L7_usecase_ReviewCheckApprovedService
    L14_infrastructure_build__review__v2 -->|"returns"| L14_infrastructure_ReviewV2Composition
    L14_infrastructure_build__review__v2 -->|"track_id"| L6_domain_TrackId
    L14_infrastructure_build__review__v2__str -->|"returns"| L14_infrastructure_ReviewV2Composition
    L14_infrastructure_build__review__v2__with__reviewer -->|"returns"| L14_infrastructure_ReviewV2CompositionWithCodex
    L14_infrastructure_build__review__v2__with__reviewer -->|"track_id"| L6_domain_TrackId
    L14_infrastructure_build__review__v2__with__reviewer__str -->|"returns"| L14_infrastructure_ReviewV2CompositionWithCodex
    L14_infrastructure_build__run__review__service -->|"returns"| L7_usecase_RunReviewService
    L14_infrastructure_build__scope__query__interactor__no__diff__str -->|"returns"| L14_infrastructure_NullDiffGetter
    L14_infrastructure_build__scope__query__interactor__no__diff__str -->|"returns"| L7_usecase_ScopeQueryInteractor
    L14_infrastructure_build__scope__query__interactor__str -->|"returns"| L7_usecase_ScopeQueryInteractor
    L14_infrastructure_capture__baseline__for__layer -->|"returns"| L14_infrastructure_CaptureBaselineError
    L14_infrastructure_check__approved__str -->|"returns"| L7_usecase_ReviewApprovalOutput
    L14_infrastructure_check__approved__str -->|"returns"| L7_usecase_ReviewCheckApprovedError
    L14_infrastructure_evaluate__and__write__signals -->|"returns"| L14_infrastructure_EvaluateSignalsError
    L14_infrastructure_evaluate__and__write__signals -->|"returns"| L14_infrastructure_SignalSummary
    L14_infrastructure_execute__type__graph__for__layer -->|"returns"| L14_infrastructure_TypeGraphExportError
    L14_infrastructure_execute__type__signals__for__layer -->|"returns"| L14_infrastructure_EvaluateSignalsError
    L14_infrastructure_load__scope__config__only -->|"track_id"| L6_domain_TrackId
    L14_infrastructure_resolve__diff__base__and__getter -->|"returns"| L6_domain_CommitHash
    L14_infrastructure_resolve__diff__base__and__getter -->|"track_id"| L6_domain_TrackId
    L14_infrastructure_run__codex__review__str -->|"returns"| L14_infrastructure_CodexReviewOutcome
    L14_infrastructure_validate__action__diagnostics -->|"returns"| L14_infrastructure_EvaluateSignalsError
    L14_infrastructure_validate__and__write__catalogue -->|"returns"| L14_infrastructure_EvaluateSignalsError
    L6_domain_TrackReader -->|"find(id)"| L6_domain_TrackId
    L7_usecase_ActivateTrackUseCase -->|"execute_by_strings"| L6_domain_TrackWriteError
    L7_usecase_CommitHashPersistenceInteractor -.impl.-> L7_usecase_CommitHashPersistenceService
    L7_usecase_CommitHashPersistenceService -->|"persist"| L7_usecase_CommitHashPersistenceError
    L7_usecase_ExportSchemaInteractor -.impl.-> L7_usecase_ExportSchemaService
    L7_usecase_ExportSchemaService -->|"export"| L7_usecase_ExportSchemaError
    L7_usecase_ExportSchemaService -->|"export(command)"| L7_usecase_ExportSchemaCommand
    L7_usecase_GuardCheckInteractor -.impl.-> L7_usecase_GuardCheckService
    L7_usecase_GuardCheckService -->|"check"| L7_usecase_GuardCheckOutput
    L7_usecase_HookDispatchInteractor -.impl.-> L7_usecase_HookDispatchService
    L7_usecase_HookDispatchService -->|"dispatch"| L7_usecase_HookDispatchError
    L7_usecase_HookDispatchService -->|"dispatch"| L7_usecase_HookVerdictOutput
    L7_usecase_HookDispatchService -->|"dispatch(command)"| L7_usecase_HookDispatchCommand
    L7_usecase_PreCommitTypeSignalsInteractor -.impl.-> L7_usecase_PreCommitTypeSignalsService
    L7_usecase_PreCommitTypeSignalsService -->|"run"| L7_usecase_PreCommitTypeSignalsError
    L7_usecase_PreCommitTypeSignalsService -->|"run"| L7_usecase_PreCommitTypeSignalsOutput
    L7_usecase_RenderContractMap -->|"execute"| L7_usecase_RenderContractMapError
    L7_usecase_RenderContractMap -->|"execute"| L7_usecase_RenderContractMapOutput
    L7_usecase_RenderContractMap -->|"execute(cmd)"| L7_usecase_RenderContractMapCommand
    L7_usecase_RenderContractMapInteractor -.impl.-> L7_usecase_RenderContractMap
    L7_usecase_ReviewCheckApprovedInteractor -.impl.-> L7_usecase_ReviewCheckApprovedService
    L7_usecase_ReviewCheckApprovedService -->|"check_approved"| L7_usecase_ReviewApprovalOutput
    L7_usecase_ReviewCheckApprovedService -->|"check_approved"| L7_usecase_ReviewCheckApprovedError
    L7_usecase_RunCatalogueLint -->|"execute"| L6_domain_CatalogueLintViolation
    L7_usecase_RunCatalogueLint -->|"execute"| L7_usecase_RunCatalogueLintError
    L7_usecase_RunCatalogueLint -->|"execute(cmd)"| L7_usecase_RunCatalogueLintCommand
    L7_usecase_RunCatalogueLintInteractor -.impl.-> L7_usecase_RunCatalogueLint
    L7_usecase_RunReviewInteractor -.impl.-> L7_usecase_RunReviewService
    L7_usecase_RunReviewService -->|"run"| L7_usecase_RunReviewError
    L7_usecase_RunReviewService -->|"run"| L7_usecase_RunReviewOutput
    L7_usecase_RunReviewService -->|"run(command)"| L7_usecase_RunReviewCommand
    L7_usecase_ScopeQueryInteractor -.impl.-> L7_usecase_ScopeQueryService
    L7_usecase_ScopeQueryService -->|"classify_by_strings"| L7_usecase_ScopeClassificationOutput
    L7_usecase_ScopeQueryService -->|"classify_by_strings"| L7_usecase_ScopeQueryError
    L7_usecase_ScopeQueryService -->|"files_by_string"| L7_usecase_ScopeQueryError
    L7_usecase_TaskOperationInteractor -.impl.-> L7_usecase_TaskOperationService
    L7_usecase_TaskOperationService -->|"add_task"| L7_usecase_TaskOperationError
    L7_usecase_TaskOperationService -->|"add_task"| L7_usecase_TaskOperationOutput
    L7_usecase_TaskOperationService -->|"add_task(cmd)"| L7_usecase_AddTaskCommand
    L7_usecase_TaskOperationService -->|"clear_override"| L7_usecase_TaskOperationError
    L7_usecase_TaskOperationService -->|"clear_override"| L7_usecase_TaskOperationOutput
    L7_usecase_TaskOperationService -->|"clear_override(cmd)"| L7_usecase_ClearOverrideCommand
    L7_usecase_TaskOperationService -->|"set_override"| L7_usecase_TaskOperationError
    L7_usecase_TaskOperationService -->|"set_override"| L7_usecase_TaskOperationOutput
    L7_usecase_TaskOperationService -->|"set_override(cmd)"| L7_usecase_SetOverrideCommand
    L7_usecase_TaskOperationService -->|"transition_task"| L7_usecase_TaskOperationError
    L7_usecase_TaskOperationService -->|"transition_task"| L7_usecase_TaskOperationOutput
    L7_usecase_TaskOperationService -->|"transition_task(cmd)"| L7_usecase_TaskTransitionCommand
    L7_usecase_TaskQueryInteractor -.impl.-> L7_usecase_TaskQueryService
    L7_usecase_TaskQueryService -->|"next_task"| L7_usecase_NextTaskOutput
    L7_usecase_TaskQueryService -->|"next_task"| L7_usecase_TaskOperationError
    L7_usecase_TaskQueryService -->|"task_counts"| L7_usecase_TaskCountsOutput
    L7_usecase_TaskQueryService -->|"task_counts"| L7_usecase_TaskOperationError
    L7_usecase_TrackPhaseInteractor -.impl.-> L7_usecase_TrackPhaseService
    L7_usecase_TrackPhaseService -->|"resolve"| L7_usecase_TrackPhaseError
    L7_usecase_TrackPhaseService -->|"resolve"| L7_usecase_TrackPhaseOutput
    L7_usecase_TypeSignalsInteractor -.impl.-> L7_usecase_TypeSignalsService
    L7_usecase_TypeSignalsService -->|"evaluate"| L7_usecase_LayerSignalSummary
    L7_usecase_TypeSignalsService -->|"evaluate"| L7_usecase_TypeSignalsError
    L7_usecase_VerifyAdrSignals -->|"verify"| L7_usecase_AdrVerifyOutput
    L7_usecase_VerifyAdrSignalsInteractor -.impl.-> L7_usecase_VerifyAdrSignals
    L7_usecase_VerifyCatalogueConsistencyInteractor -.impl.-> L7_usecase_VerifyCatalogueConsistencyService
    L7_usecase_VerifyCatalogueConsistencyService -->|"verify"| L7_usecase_VerifyCatalogueConsistencyError
    L7_usecase_VerifyCatalogueConsistencyService -->|"verify"| L7_usecase_VerifyCatalogueConsistencyOutput
    L7_usecase_VerifyCatalogueSpecRefsInteractor -.impl.-> L7_usecase_VerifyCatalogueSpecRefsService
    L7_usecase_VerifyCatalogueSpecRefsService -->|"verify"| L7_usecase_VerifyCatalogueSpecRefsError
    L7_usecase_VerifyCatalogueSpecRefsService -->|"verify"| L7_usecase_VerifyCatalogueSpecRefsOutput
    L7_usecase_VerifyCatalogueSpecSignalsInteractor -.impl.-> L7_usecase_VerifyCatalogueSpecSignalsService
    L7_usecase_VerifyCatalogueSpecSignalsService -->|"verify"| L7_usecase_VerifySpecSignalsError
    L7_usecase_VerifyCatalogueSpecSignalsService -->|"verify"| L7_usecase_VerifySpecSignalsOutput
    L7_usecase_reject__branchless__guard__by__str -->|"reader"| L6_domain_TrackReader
```
