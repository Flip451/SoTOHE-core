<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
flowchart LR
classDef aggregate_root fill:#ede9fe,stroke:#4c1d95,stroke-width:2px
classDef app_service fill:#ecfdf5,stroke:#059669,stroke-width:2px
classDef command fill:#fff7ed,stroke:#c2410c,stroke-width:1px
classDef domain_service fill:#fee2e2,stroke:#991b1b,stroke-width:1px
classDef dto fill:#f8fafc,stroke:#64748b,stroke-width:1px
classDef entity fill:#dbeafe,stroke:#1e40af,stroke-width:2px
classDef error_type fill:#fef2f2,stroke:#b91c1c,stroke-width:1px,stroke-dasharray:4 2
classDef factory fill:#e0f2fe,stroke:#0369a1,stroke-width:1px
classDef free_function fill:#f5f3ff,stroke:#7c3aed,stroke-width:1px
classDef function_node fill:#f5f3ff,stroke:#a78bfa,stroke-width:1px
classDef interactor fill:#f0fdfa,stroke:#0d9488,stroke-width:1px
classDef method_node fill:#f8fafc,stroke:#cbd5e1,stroke-width:1px
classDef query fill:#f0f9ff,stroke:#0369a1,stroke-width:1px
classDef secondary_adapter fill:#fafaf9,stroke:#57534e,stroke-width:1px
classDef secondary_port fill:#fafaf9,stroke:#78716c,stroke-width:1px,stroke-dasharray:4 2
classDef specification fill:#fdf4ff,stroke:#6b21a8,stroke-width:1px
classDef specification_port fill:#fdf4ff,stroke:#9333ea,stroke-width:1px,stroke-dasharray:4 2
classDef typestate_overlay stroke:#dc2626,stroke-width:3px
classDef use_case fill:#ecfeff,stroke:#0e7490,stroke-width:1px
classDef use_case_function fill:#eef2ff,stroke:#4338ca,stroke-width:1px
classDef value_object fill:#d1fae5,stroke:#065f46,stroke-width:1px
classDef variant_node fill:#fafaf9,stroke:#d6d3d1,stroke-width:1px
subgraph domain["domain"]
  direction TB
  subgraph domain_domain_module_track_phase["domain::track_phase"]
    direction TB
  subgraph T25_domain_domain_NextCommand["track_phase::NextCommand"]
    direction TB
    T25_domain_domain_NextCommand__self[NextCommand]
    T25_domain_domain_NextCommand_Implement[Implement]
    T25_domain_domain_NextCommand_Done[Done]
    T25_domain_domain_NextCommand_PlanNewFeature[PlanNewFeature]
    T25_domain_domain_NextCommand_Status[Status]
  end
  subgraph T24_domain_domain_TrackPhase["track_phase::TrackPhase"]
    direction TB
    T24_domain_domain_TrackPhase__self[TrackPhase]
    T24_domain_domain_TrackPhase_Planning[Planning]
    T24_domain_domain_TrackPhase_InProgress[InProgress]
    T24_domain_domain_TrackPhase_ReadyToShip[ReadyToShip]
    T24_domain_domain_TrackPhase_Blocked[Blocked]
    T24_domain_domain_TrackPhase_Cancelled[Cancelled]
    T24_domain_domain_TrackPhase_Archived[Archived]
  end
  F47_domain_domain_domain__track_phase__next_command[[next_command]]
  F48_domain_domain_domain__track_phase__resolve_phase[[resolve_phase]]
  F60_domain_domain_domain__track_phase__resolve_phase_from_record[[resolve_phase_from_record]]
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_pr_workflow["usecase::pr_workflow"]
    direction TB
  subgraph T31_usecase_usecase_PrBranchContext["pr_workflow::PrBranchContext"]
    direction TB
    T31_usecase_usecase_PrBranchContext__self[PrBranchContext]
  end
  end
  subgraph usecase_usecase_module_pre_commit_type_signals["usecase::pre_commit_type_signals"]
    direction TB
  subgraph T41_usecase_usecase_PreCommitTypeSignalsError["pre_commit_type_signals::PreCommitTypeSignalsError"]
    direction TB
    T41_usecase_usecase_PreCommitTypeSignalsError__self[PreCommitTypeSignalsError]
    T41_usecase_usecase_PreCommitTypeSignalsError_GitDiscoverFailed[GitDiscoverFailed]
    T41_usecase_usecase_PreCommitTypeSignalsError_RulesFileMissing[RulesFileMissing]
    T41_usecase_usecase_PreCommitTypeSignalsError_RulesParseError[RulesParseError]
    T41_usecase_usecase_PreCommitTypeSignalsError_SymlinkRejected[SymlinkRejected]
    T41_usecase_usecase_PreCommitTypeSignalsError_BranchNotFound[BranchNotFound]
    T41_usecase_usecase_PreCommitTypeSignalsError_BranchMismatch[BranchMismatch]
    T41_usecase_usecase_PreCommitTypeSignalsError_TypeSignalsRecomputeFailed[TypeSignalsRecomputeFailed]
  end
  subgraph T46_usecase_usecase_PreCommitTypeSignalsInteractor["pre_commit_type_signals::PreCommitTypeSignalsInteractor"]
    direction TB
    T46_usecase_usecase_PreCommitTypeSignalsInteractor__self[PreCommitTypeSignalsInteractor]
    T46_usecase_usecase_PreCommitTypeSignalsInteractor_new([new])
  end
  subgraph T42_usecase_usecase_PreCommitTypeSignalsOutput["pre_commit_type_signals::PreCommitTypeSignalsOutput"]
    direction TB
    T42_usecase_usecase_PreCommitTypeSignalsOutput__self[PreCommitTypeSignalsOutput]
  end
  subgraph R43_usecase_usecase_PreCommitTypeSignalsService["pre_commit_type_signals::PreCommitTypeSignalsService"]
    direction TB
    R43_usecase_usecase_PreCommitTypeSignalsService__self[PreCommitTypeSignalsService]
    R43_usecase_usecase_PreCommitTypeSignalsService_run([run])
  end
  end
  subgraph usecase_usecase_module_task_ops["usecase::task_ops"]
    direction TB
  subgraph T39_usecase_usecase_TaskOperationInteractor["task_ops::TaskOperationInteractor"]
    direction TB
    T39_usecase_usecase_TaskOperationInteractor__self[TaskOperationInteractor]
    T39_usecase_usecase_TaskOperationInteractor_new([new])
  end
  end
  subgraph usecase_usecase_module_track_phase["usecase::track_phase"]
    direction TB
  subgraph T36_usecase_usecase_TrackPhaseInteractor["track_phase::TrackPhaseInteractor"]
    direction TB
    T36_usecase_usecase_TrackPhaseInteractor__self[TrackPhaseInteractor]
    T36_usecase_usecase_TrackPhaseInteractor_new([new])
  end
  end
  subgraph usecase_usecase_module_track_resolution["usecase::track_resolution"]
    direction TB
  subgraph T36_usecase_usecase_TrackResolutionError["track_resolution::TrackResolutionError"]
    direction TB
    T36_usecase_usecase_TrackResolutionError__self[TrackResolutionError]
    T36_usecase_usecase_TrackResolutionError_DetachedHead[DetachedHead]
    T36_usecase_usecase_TrackResolutionError_NotTrackBranch[NotTrackBranch]
    T36_usecase_usecase_TrackResolutionError_NoBranch[NoBranch]
    T36_usecase_usecase_TrackResolutionError_InvalidTrackId[InvalidTrackId]
    T36_usecase_usecase_TrackResolutionError_UnsupportedTargetStatus[UnsupportedTargetStatus]
    T36_usecase_usecase_TrackResolutionError_TrackNotFound[TrackNotFound]
    T36_usecase_usecase_TrackResolutionError_ReadError[ReadError]
  end
  end
  subgraph usecase_usecase_module_type_signals["usecase::type_signals"]
    direction TB
  subgraph T32_usecase_usecase_TypeSignalsError["type_signals::TypeSignalsError"]
    direction TB
    T32_usecase_usecase_TypeSignalsError__self[TypeSignalsError]
    T32_usecase_usecase_TypeSignalsError_InvalidTrackId[InvalidTrackId]
    T32_usecase_usecase_TypeSignalsError_NonActiveTrack[NonActiveTrack]
    T32_usecase_usecase_TypeSignalsError_BranchTrackMismatch[BranchTrackMismatch]
    T32_usecase_usecase_TypeSignalsError_LayerBindingsLoad[LayerBindingsLoad]
    T32_usecase_usecase_TypeSignalsError_NoLayers[NoLayers]
    T32_usecase_usecase_TypeSignalsError_EvaluationFailed[EvaluationFailed]
    T32_usecase_usecase_TypeSignalsError_InconsistentRequest[InconsistentRequest]
  end
  subgraph T37_usecase_usecase_TypeSignalsInteractor["type_signals::TypeSignalsInteractor"]
    direction TB
    T37_usecase_usecase_TypeSignalsInteractor__self[TypeSignalsInteractor]
    T37_usecase_usecase_TypeSignalsInteractor_new([new])
  end
  subgraph T34_usecase_usecase_TypeSignalsRequest["type_signals::TypeSignalsRequest"]
    direction TB
    T34_usecase_usecase_TypeSignalsRequest__self[TypeSignalsRequest]
  end
  subgraph R34_usecase_usecase_TypeSignalsService["type_signals::TypeSignalsService"]
    direction TB
    R34_usecase_usecase_TypeSignalsService__self[TypeSignalsService]
    R34_usecase_usecase_TypeSignalsService_run([run])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_track["infrastructure::track"]
    direction TB
  subgraph T41_infrastructure_infrastructure_RenderError["track::render::RenderError"]
    direction TB
    T41_infrastructure_infrastructure_RenderError__self[RenderError]
    T41_infrastructure_infrastructure_RenderError_Io[Io]
    T41_infrastructure_infrastructure_RenderError_InvalidMetadata[InvalidMetadata]
    T41_infrastructure_infrastructure_RenderError_OutOfSync[OutOfSync]
    T41_infrastructure_infrastructure_RenderError_UnsupportedSchemaVersion[UnsupportedSchemaVersion]
    T41_infrastructure_infrastructure_RenderError_InvalidTrackMetadata[InvalidTrackMetadata]
  end
  F80_infrastructure_infrastructure_infrastructure__track__render__sync_rendered_views[[sync_rendered_views]]
  end
end
F47_domain_domain_domain__track_phase__next_command --> T25_domain_domain_NextCommand__self
T46_usecase_usecase_PreCommitTypeSignalsInteractor_new --> T46_usecase_usecase_PreCommitTypeSignalsInteractor__self
R43_usecase_usecase_PreCommitTypeSignalsService_run --> T41_usecase_usecase_PreCommitTypeSignalsError__self
R43_usecase_usecase_PreCommitTypeSignalsService_run --> T42_usecase_usecase_PreCommitTypeSignalsOutput__self
T39_usecase_usecase_TaskOperationInteractor_new --> T39_usecase_usecase_TaskOperationInteractor__self
T36_usecase_usecase_TrackPhaseInteractor_new --> T36_usecase_usecase_TrackPhaseInteractor__self
T37_usecase_usecase_TypeSignalsInteractor_new --> T37_usecase_usecase_TypeSignalsInteractor__self
R34_usecase_usecase_TypeSignalsService_run --o T34_usecase_usecase_TypeSignalsRequest__self
R34_usecase_usecase_TypeSignalsService_run --> T32_usecase_usecase_TypeSignalsError__self
T46_usecase_usecase_PreCommitTypeSignalsInteractor__self -.impl.-> R43_usecase_usecase_PreCommitTypeSignalsService__self
T37_usecase_usecase_TypeSignalsInteractor__self -.impl.-> R34_usecase_usecase_TypeSignalsService__self
F80_infrastructure_infrastructure_infrastructure__track__render__sync_rendered_views --> T41_infrastructure_infrastructure_RenderError__self
class T25_domain_domain_NextCommand_Implement variant_node
class T25_domain_domain_NextCommand_Done variant_node
class T25_domain_domain_NextCommand_PlanNewFeature variant_node
class T25_domain_domain_NextCommand_Status variant_node
class T25_domain_domain_NextCommand__self value_object
class T24_domain_domain_TrackPhase_Planning variant_node
class T24_domain_domain_TrackPhase_InProgress variant_node
class T24_domain_domain_TrackPhase_ReadyToShip variant_node
class T24_domain_domain_TrackPhase_Blocked variant_node
class T24_domain_domain_TrackPhase_Cancelled variant_node
class T24_domain_domain_TrackPhase_Archived variant_node
class T24_domain_domain_TrackPhase__self value_object
class F47_domain_domain_domain__track_phase__next_command free_function
class F47_domain_domain_domain__track_phase__next_command function_node
class F48_domain_domain_domain__track_phase__resolve_phase free_function
class F48_domain_domain_domain__track_phase__resolve_phase function_node
class F60_domain_domain_domain__track_phase__resolve_phase_from_record free_function
class F60_domain_domain_domain__track_phase__resolve_phase_from_record function_node
class T31_usecase_usecase_PrBranchContext__self value_object
class T41_usecase_usecase_PreCommitTypeSignalsError_GitDiscoverFailed variant_node
class T41_usecase_usecase_PreCommitTypeSignalsError_RulesFileMissing variant_node
class T41_usecase_usecase_PreCommitTypeSignalsError_RulesParseError variant_node
class T41_usecase_usecase_PreCommitTypeSignalsError_SymlinkRejected variant_node
class T41_usecase_usecase_PreCommitTypeSignalsError_BranchNotFound variant_node
class T41_usecase_usecase_PreCommitTypeSignalsError_BranchMismatch variant_node
class T41_usecase_usecase_PreCommitTypeSignalsError_TypeSignalsRecomputeFailed variant_node
class T41_usecase_usecase_PreCommitTypeSignalsError__self error_type
class T46_usecase_usecase_PreCommitTypeSignalsInteractor_new method_node
class T46_usecase_usecase_PreCommitTypeSignalsInteractor__self interactor
class T42_usecase_usecase_PreCommitTypeSignalsOutput__self dto
class R43_usecase_usecase_PreCommitTypeSignalsService_run method_node
class R43_usecase_usecase_PreCommitTypeSignalsService__self app_service
class T39_usecase_usecase_TaskOperationInteractor_new method_node
class T39_usecase_usecase_TaskOperationInteractor__self interactor
class T36_usecase_usecase_TrackPhaseInteractor_new method_node
class T36_usecase_usecase_TrackPhaseInteractor__self interactor
class T36_usecase_usecase_TrackResolutionError_DetachedHead variant_node
class T36_usecase_usecase_TrackResolutionError_NotTrackBranch variant_node
class T36_usecase_usecase_TrackResolutionError_NoBranch variant_node
class T36_usecase_usecase_TrackResolutionError_InvalidTrackId variant_node
class T36_usecase_usecase_TrackResolutionError_UnsupportedTargetStatus variant_node
class T36_usecase_usecase_TrackResolutionError_TrackNotFound variant_node
class T36_usecase_usecase_TrackResolutionError_ReadError variant_node
class T36_usecase_usecase_TrackResolutionError__self error_type
class T32_usecase_usecase_TypeSignalsError_InvalidTrackId variant_node
class T32_usecase_usecase_TypeSignalsError_NonActiveTrack variant_node
class T32_usecase_usecase_TypeSignalsError_BranchTrackMismatch variant_node
class T32_usecase_usecase_TypeSignalsError_LayerBindingsLoad variant_node
class T32_usecase_usecase_TypeSignalsError_NoLayers variant_node
class T32_usecase_usecase_TypeSignalsError_EvaluationFailed variant_node
class T32_usecase_usecase_TypeSignalsError_InconsistentRequest variant_node
class T32_usecase_usecase_TypeSignalsError__self error_type
class T37_usecase_usecase_TypeSignalsInteractor_new method_node
class T37_usecase_usecase_TypeSignalsInteractor__self interactor
class T34_usecase_usecase_TypeSignalsRequest__self command
class R34_usecase_usecase_TypeSignalsService_run method_node
class R34_usecase_usecase_TypeSignalsService__self app_service
class T41_infrastructure_infrastructure_RenderError_Io variant_node
class T41_infrastructure_infrastructure_RenderError_InvalidMetadata variant_node
class T41_infrastructure_infrastructure_RenderError_OutOfSync variant_node
class T41_infrastructure_infrastructure_RenderError_UnsupportedSchemaVersion variant_node
class T41_infrastructure_infrastructure_RenderError_InvalidTrackMetadata variant_node
class T41_infrastructure_infrastructure_RenderError__self error_type
class F80_infrastructure_infrastructure_infrastructure__track__render__sync_rendered_views free_function
class F80_infrastructure_infrastructure_infrastructure__track__render__sync_rendered_views function_node
```
