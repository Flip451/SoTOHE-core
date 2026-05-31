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
  subgraph usecase_usecase_module_review_v2["usecase::review_v2"]
    direction TB
  subgraph T36_usecase_usecase_ReviewFixRunnerError["review_v2::run_review_fix::ReviewFixRunnerError"]
    direction TB
    T36_usecase_usecase_ReviewFixRunnerError__self[ReviewFixRunnerError]
    T36_usecase_usecase_ReviewFixRunnerError_SmokeTestFailed[SmokeTestFailed]
    T36_usecase_usecase_ReviewFixRunnerError_SpawnFailed[SpawnFailed]
    T36_usecase_usecase_ReviewFixRunnerError_SentinelNotFound[SentinelNotFound]
    T36_usecase_usecase_ReviewFixRunnerError_Unexpected[Unexpected]
  end
  subgraph T35_usecase_usecase_RunReviewFixCommand["review_v2::run_review_fix::RunReviewFixCommand"]
    direction TB
    T35_usecase_usecase_RunReviewFixCommand__self[RunReviewFixCommand]
  end
  subgraph T33_usecase_usecase_RunReviewFixError["review_v2::run_review_fix::RunReviewFixError"]
    direction TB
    T33_usecase_usecase_RunReviewFixError__self[RunReviewFixError]
    T33_usecase_usecase_RunReviewFixError_InvalidScope[InvalidScope]
    T33_usecase_usecase_RunReviewFixError_InvalidTrackId[InvalidTrackId]
    T33_usecase_usecase_RunReviewFixError_InvalidRoundType[InvalidRoundType]
    T33_usecase_usecase_RunReviewFixError_EmptyScopeFiles[EmptyScopeFiles]
    T33_usecase_usecase_RunReviewFixError_SmokeTestFailed[SmokeTestFailed]
    T33_usecase_usecase_RunReviewFixError_FixRunnerFailed[FixRunnerFailed]
  end
  subgraph T38_usecase_usecase_RunReviewFixInteractor["review_v2::run_review_fix::RunReviewFixInteractor"]
    direction TB
    T38_usecase_usecase_RunReviewFixInteractor__self[RunReviewFixInteractor]
    T38_usecase_usecase_RunReviewFixInteractor_new([new])
  end
  subgraph T34_usecase_usecase_RunReviewFixOutput["review_v2::run_review_fix::RunReviewFixOutput"]
    direction TB
    T34_usecase_usecase_RunReviewFixOutput__self[RunReviewFixOutput]
  end
  subgraph R31_usecase_usecase_ReviewFixRunner["review_v2::run_review_fix::ReviewFixRunner"]
    direction TB
    R31_usecase_usecase_ReviewFixRunner__self[ReviewFixRunner]
    R31_usecase_usecase_ReviewFixRunner_run_fix([run_fix])
  end
  subgraph R35_usecase_usecase_RunReviewFixService["review_v2::run_review_fix::RunReviewFixService"]
    direction TB
    R35_usecase_usecase_RunReviewFixService__self[RunReviewFixService]
    R35_usecase_usecase_RunReviewFixService_run([run])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  subgraph T50_infrastructure_infrastructure_CodexReviewFixRunner["review_v2::review_fix_runner::CodexReviewFixRunner"]
    direction TB
    T50_infrastructure_infrastructure_CodexReviewFixRunner__self[CodexReviewFixRunner]
    T50_infrastructure_infrastructure_CodexReviewFixRunner_new([new])
  end
  end
end
T38_usecase_usecase_RunReviewFixInteractor_new --> T38_usecase_usecase_RunReviewFixInteractor__self
R31_usecase_usecase_ReviewFixRunner_run_fix --o T35_usecase_usecase_RunReviewFixCommand__self
R31_usecase_usecase_ReviewFixRunner_run_fix --> T36_usecase_usecase_ReviewFixRunnerError__self
R31_usecase_usecase_ReviewFixRunner_run_fix --> T34_usecase_usecase_RunReviewFixOutput__self
R35_usecase_usecase_RunReviewFixService_run --o T35_usecase_usecase_RunReviewFixCommand__self
R35_usecase_usecase_RunReviewFixService_run --> T33_usecase_usecase_RunReviewFixError__self
R35_usecase_usecase_RunReviewFixService_run --> T34_usecase_usecase_RunReviewFixOutput__self
T38_usecase_usecase_RunReviewFixInteractor__self -.impl.-> R35_usecase_usecase_RunReviewFixService__self
T50_infrastructure_infrastructure_CodexReviewFixRunner_new --> T50_infrastructure_infrastructure_CodexReviewFixRunner__self
T50_infrastructure_infrastructure_CodexReviewFixRunner__self -.impl.-> R31_usecase_usecase_ReviewFixRunner__self
class T36_usecase_usecase_ReviewFixRunnerError_SmokeTestFailed variant_node
class T36_usecase_usecase_ReviewFixRunnerError_SpawnFailed variant_node
class T36_usecase_usecase_ReviewFixRunnerError_SentinelNotFound variant_node
class T36_usecase_usecase_ReviewFixRunnerError_Unexpected variant_node
class T36_usecase_usecase_ReviewFixRunnerError__self error_type
class T35_usecase_usecase_RunReviewFixCommand__self command
class T33_usecase_usecase_RunReviewFixError_InvalidScope variant_node
class T33_usecase_usecase_RunReviewFixError_InvalidTrackId variant_node
class T33_usecase_usecase_RunReviewFixError_InvalidRoundType variant_node
class T33_usecase_usecase_RunReviewFixError_EmptyScopeFiles variant_node
class T33_usecase_usecase_RunReviewFixError_SmokeTestFailed variant_node
class T33_usecase_usecase_RunReviewFixError_FixRunnerFailed variant_node
class T33_usecase_usecase_RunReviewFixError__self error_type
class T38_usecase_usecase_RunReviewFixInteractor_new method_node
class T38_usecase_usecase_RunReviewFixInteractor__self interactor
class T34_usecase_usecase_RunReviewFixOutput__self value_object
class R31_usecase_usecase_ReviewFixRunner_run_fix method_node
class R31_usecase_usecase_ReviewFixRunner__self secondary_port
class R35_usecase_usecase_RunReviewFixService_run method_node
class R35_usecase_usecase_RunReviewFixService__self app_service
class T50_infrastructure_infrastructure_CodexReviewFixRunner_new method_node
class T50_infrastructure_infrastructure_CodexReviewFixRunner__self secondary_adapter
```
