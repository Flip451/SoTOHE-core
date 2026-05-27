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
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_task_ops["usecase::task_ops"]
    direction TB
  subgraph T39_usecase_usecase_TaskOperationInteractor["task_ops::TaskOperationInteractor"]
    direction TB
    T39_usecase_usecase_TaskOperationInteractor__self[TaskOperationInteractor]
    T39_usecase_usecase_TaskOperationInteractor_new([new])
  end
  subgraph R36_usecase_usecase_TaskOperationService["task_ops::TaskOperationService"]
    direction TB
    R36_usecase_usecase_TaskOperationService__self[TaskOperationService]
    R36_usecase_usecase_TaskOperationService_transition_task([transition_task])
    R36_usecase_usecase_TaskOperationService_add_task([add_task])
    R36_usecase_usecase_TaskOperationService_set_override([set_override])
    R36_usecase_usecase_TaskOperationService_clear_override([clear_override])
  end
  end
  subgraph usecase_usecase_module_track_resolution["usecase::track_resolution"]
    direction TB
  subgraph T39_usecase_usecase_ActiveTrackResolveError["track_resolution::ActiveTrackResolveError"]
    direction TB
    T39_usecase_usecase_ActiveTrackResolveError__self[ActiveTrackResolveError]
    T39_usecase_usecase_ActiveTrackResolveError_BranchRead[BranchRead]
    T39_usecase_usecase_ActiveTrackResolveError_Resolution[Resolution]
  end
  subgraph T44_usecase_usecase_ActiveTrackResolveInteractor["track_resolution::ActiveTrackResolveInteractor"]
    direction TB
    T44_usecase_usecase_ActiveTrackResolveInteractor__self[ActiveTrackResolveInteractor]
    T44_usecase_usecase_ActiveTrackResolveInteractor_new([new])
  end
  subgraph T31_usecase_usecase_BranchReadError["track_resolution::BranchReadError"]
    direction TB
    T31_usecase_usecase_BranchReadError__self[BranchReadError]
    T31_usecase_usecase_BranchReadError_ReadFailed[ReadFailed]
  end
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
  subgraph R41_usecase_usecase_ActiveTrackResolveService["track_resolution::ActiveTrackResolveService"]
    direction TB
    R41_usecase_usecase_ActiveTrackResolveService__self[ActiveTrackResolveService]
    R41_usecase_usecase_ActiveTrackResolveService_resolve_active_track([resolve_active_track])
  end
  subgraph R32_usecase_usecase_BranchReaderPort["track_resolution::BranchReaderPort"]
    direction TB
    R32_usecase_usecase_BranchReaderPort__self[BranchReaderPort]
    R32_usecase_usecase_BranchReaderPort_current_branch([current_branch])
  end
  F71_usecase_usecase_usecase__track_resolution__resolve_track_id_from_branch[[resolve_track_id_from_branch]]
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_git_cli["infrastructure::git_cli"]
    direction TB
  subgraph T43_infrastructure_infrastructure_SystemGitRepo["git_cli::SystemGitRepo"]
    direction TB
    T43_infrastructure_infrastructure_SystemGitRepo__self[SystemGitRepo]
  end
  end
end
T39_usecase_usecase_TaskOperationInteractor_new --> T39_usecase_usecase_TaskOperationInteractor__self
T39_usecase_usecase_ActiveTrackResolveError_BranchRead --o T31_usecase_usecase_BranchReadError__self
T39_usecase_usecase_ActiveTrackResolveError_Resolution --o T36_usecase_usecase_TrackResolutionError__self
T44_usecase_usecase_ActiveTrackResolveInteractor_new --> T44_usecase_usecase_ActiveTrackResolveInteractor__self
R41_usecase_usecase_ActiveTrackResolveService_resolve_active_track --> T39_usecase_usecase_ActiveTrackResolveError__self
R32_usecase_usecase_BranchReaderPort_current_branch --> T31_usecase_usecase_BranchReadError__self
F71_usecase_usecase_usecase__track_resolution__resolve_track_id_from_branch --> T36_usecase_usecase_TrackResolutionError__self
T44_usecase_usecase_ActiveTrackResolveInteractor__self -.impl.-> R41_usecase_usecase_ActiveTrackResolveService__self
T39_usecase_usecase_TaskOperationInteractor__self -.impl.-> R36_usecase_usecase_TaskOperationService__self
T43_infrastructure_infrastructure_SystemGitRepo__self -.impl.-> R32_usecase_usecase_BranchReaderPort__self
class T39_usecase_usecase_TaskOperationInteractor_new method_node
class T39_usecase_usecase_TaskOperationInteractor__self interactor
class R36_usecase_usecase_TaskOperationService_transition_task method_node
class R36_usecase_usecase_TaskOperationService_add_task method_node
class R36_usecase_usecase_TaskOperationService_set_override method_node
class R36_usecase_usecase_TaskOperationService_clear_override method_node
class R36_usecase_usecase_TaskOperationService__self app_service
class T39_usecase_usecase_ActiveTrackResolveError_BranchRead variant_node
class T39_usecase_usecase_ActiveTrackResolveError_Resolution variant_node
class T39_usecase_usecase_ActiveTrackResolveError__self error_type
class T44_usecase_usecase_ActiveTrackResolveInteractor_new method_node
class T44_usecase_usecase_ActiveTrackResolveInteractor__self interactor
class T31_usecase_usecase_BranchReadError_ReadFailed variant_node
class T31_usecase_usecase_BranchReadError__self error_type
class T36_usecase_usecase_TrackResolutionError_DetachedHead variant_node
class T36_usecase_usecase_TrackResolutionError_NotTrackBranch variant_node
class T36_usecase_usecase_TrackResolutionError_NoBranch variant_node
class T36_usecase_usecase_TrackResolutionError_InvalidTrackId variant_node
class T36_usecase_usecase_TrackResolutionError_UnsupportedTargetStatus variant_node
class T36_usecase_usecase_TrackResolutionError_TrackNotFound variant_node
class T36_usecase_usecase_TrackResolutionError_ReadError variant_node
class T36_usecase_usecase_TrackResolutionError__self error_type
class R41_usecase_usecase_ActiveTrackResolveService_resolve_active_track method_node
class R41_usecase_usecase_ActiveTrackResolveService__self app_service
class R32_usecase_usecase_BranchReaderPort_current_branch method_node
class R32_usecase_usecase_BranchReaderPort__self secondary_port
class F71_usecase_usecase_usecase__track_resolution__resolve_track_id_from_branch free_function
class F71_usecase_usecase_usecase__track_resolution__resolve_track_id_from_branch function_node
class T43_infrastructure_infrastructure_SystemGitRepo__self secondary_adapter
```
