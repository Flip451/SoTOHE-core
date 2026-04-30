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
    end
    subgraph infrastructure [infrastructure]
        L14_infrastructure_FsTrackStore[FsTrackStore]:::secondary_adapter
        L14_infrastructure_RustdocSchemaExporter[RustdocSchemaExporter]:::secondary_adapter
        L14_infrastructure_ConchShellParser[ConchShellParser]:::secondary_adapter
    end
    L14_infrastructure_ConchShellParser -.impl.-> L7_usecase_HookShellParserPort
    L14_infrastructure_ConchShellParser -.impl.-> L7_usecase_ShellParserPort
    L14_infrastructure_RustdocSchemaExporter -.impl.-> L7_usecase_SchemaExporterPort
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
    L7_usecase_ReviewCheckApprovedInteractor -.impl.-> L7_usecase_ReviewCheckApprovedService
    L7_usecase_ReviewCheckApprovedService -->|"check_approved"| L7_usecase_ReviewApprovalOutput
    L7_usecase_ReviewCheckApprovedService -->|"check_approved"| L7_usecase_ReviewCheckApprovedError
    L7_usecase_RunReviewInteractor -.impl.-> L7_usecase_RunReviewService
    L7_usecase_RunReviewService -->|"run"| L7_usecase_RunReviewError
    L7_usecase_RunReviewService -->|"run"| L7_usecase_RunReviewOutput
    L7_usecase_RunReviewService -->|"run(command)"| L7_usecase_RunReviewCommand
    L7_usecase_ScopeQueryInteractor -.impl.-> L7_usecase_ScopeQueryService
    L7_usecase_ScopeQueryService -->|"classify_by_strings"| L7_usecase_ScopeClassificationOutput
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
```
