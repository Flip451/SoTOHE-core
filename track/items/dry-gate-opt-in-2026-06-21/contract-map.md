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
  subgraph usecase_usecase_module_dry_check["usecase::dry_check"]
    direction TB
  subgraph T42_usecase_usecase_DryCheckApprovalInteractor["dry_check::approval_interactor::DryCheckApprovalInteractor"]
    direction TB
    T42_usecase_usecase_DryCheckApprovalInteractor__self[DryCheckApprovalInteractor]
    T42_usecase_usecase_DryCheckApprovalInteractor_new([new])
  end
  subgraph T30_usecase_usecase_DryCheckConfig["dry_check::config::DryCheckConfig"]
    direction TB
    T30_usecase_usecase_DryCheckConfig__self[DryCheckConfig]
    T30_usecase_usecase_DryCheckConfig_new([new])
  end
  end
  subgraph usecase_usecase_module_fixpoint_resolve["usecase::fixpoint_resolve"]
    direction TB
  subgraph T41_usecase_usecase_FixpointResolveInteractor["fixpoint_resolve::FixpointResolveInteractor"]
    direction TB
    T41_usecase_usecase_FixpointResolveInteractor__self[FixpointResolveInteractor]
    T41_usecase_usecase_FixpointResolveInteractor_new([new])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_dry_check["infrastructure::dry_check"]
    direction TB
  subgraph T44_infrastructure_infrastructure_DryCheckConfig["dry_check::config::DryCheckConfig"]
    direction TB
    T44_infrastructure_infrastructure_DryCheckConfig__self[DryCheckConfig]
    T44_infrastructure_infrastructure_DryCheckConfig_load([load])
    T44_infrastructure_infrastructure_DryCheckConfig_enabled([enabled])
    T44_infrastructure_infrastructure_DryCheckConfig_threshold([threshold])
    T44_infrastructure_infrastructure_DryCheckConfig_max_parallelism([max_parallelism])
    T44_infrastructure_infrastructure_DryCheckConfig_fast_reasoning_effort([fast_reasoning_effort])
    T44_infrastructure_infrastructure_DryCheckConfig_final_reasoning_effort([final_reasoning_effort])
    T44_infrastructure_infrastructure_DryCheckConfig_known_bad_injection_rate_percent([known_bad_injection_rate_percent])
    T44_infrastructure_infrastructure_DryCheckConfig_known_bad_detection_threshold_percent([known_bad_detection_threshold_percent])
    T44_infrastructure_infrastructure_DryCheckConfig_fingerprint_with_threshold([fingerprint_with_threshold])
    T44_infrastructure_infrastructure_DryCheckConfig_fingerprint([fingerprint])
  end
  subgraph T49_infrastructure_infrastructure_DryCheckConfigError["dry_check::config::DryCheckConfigError"]
    direction TB
    T49_infrastructure_infrastructure_DryCheckConfigError__self[DryCheckConfigError]
    T49_infrastructure_infrastructure_DryCheckConfigError_Io[Io]
    T49_infrastructure_infrastructure_DryCheckConfigError_Parse[Parse]
    T49_infrastructure_infrastructure_DryCheckConfigError_UnsupportedSchemaVersion[UnsupportedSchemaVersion]
    T49_infrastructure_infrastructure_DryCheckConfigError_InvalidThreshold[InvalidThreshold]
    T49_infrastructure_infrastructure_DryCheckConfigError_InvalidParallelism[InvalidParallelism]
    T49_infrastructure_infrastructure_DryCheckConfigError_InvalidReasoningEffort[InvalidReasoningEffort]
    T49_infrastructure_infrastructure_DryCheckConfigError_InvalidPercent[InvalidPercent]
  end
  end
end
T42_usecase_usecase_DryCheckApprovalInteractor_new --o T30_usecase_usecase_DryCheckConfig__self
T42_usecase_usecase_DryCheckApprovalInteractor_new --> T42_usecase_usecase_DryCheckApprovalInteractor__self
T30_usecase_usecase_DryCheckConfig_new --> T30_usecase_usecase_DryCheckConfig__self
T41_usecase_usecase_FixpointResolveInteractor_new --o T30_usecase_usecase_DryCheckConfig__self
T41_usecase_usecase_FixpointResolveInteractor_new --> T41_usecase_usecase_FixpointResolveInteractor__self
T44_infrastructure_infrastructure_DryCheckConfig_load --> T44_infrastructure_infrastructure_DryCheckConfig__self
T44_infrastructure_infrastructure_DryCheckConfig_load --> T49_infrastructure_infrastructure_DryCheckConfigError__self
class T42_usecase_usecase_DryCheckApprovalInteractor_new method_node
class T42_usecase_usecase_DryCheckApprovalInteractor__self interactor
class T30_usecase_usecase_DryCheckConfig_new method_node
class T30_usecase_usecase_DryCheckConfig__self value_object
class T41_usecase_usecase_FixpointResolveInteractor_new method_node
class T41_usecase_usecase_FixpointResolveInteractor__self interactor
class T44_infrastructure_infrastructure_DryCheckConfig_load method_node
class T44_infrastructure_infrastructure_DryCheckConfig_enabled method_node
class T44_infrastructure_infrastructure_DryCheckConfig_threshold method_node
class T44_infrastructure_infrastructure_DryCheckConfig_max_parallelism method_node
class T44_infrastructure_infrastructure_DryCheckConfig_fast_reasoning_effort method_node
class T44_infrastructure_infrastructure_DryCheckConfig_final_reasoning_effort method_node
class T44_infrastructure_infrastructure_DryCheckConfig_known_bad_injection_rate_percent method_node
class T44_infrastructure_infrastructure_DryCheckConfig_known_bad_detection_threshold_percent method_node
class T44_infrastructure_infrastructure_DryCheckConfig_fingerprint_with_threshold method_node
class T44_infrastructure_infrastructure_DryCheckConfig_fingerprint method_node
class T44_infrastructure_infrastructure_DryCheckConfig__self dto
class T49_infrastructure_infrastructure_DryCheckConfigError_Io variant_node
class T49_infrastructure_infrastructure_DryCheckConfigError_Parse variant_node
class T49_infrastructure_infrastructure_DryCheckConfigError_UnsupportedSchemaVersion variant_node
class T49_infrastructure_infrastructure_DryCheckConfigError_InvalidThreshold variant_node
class T49_infrastructure_infrastructure_DryCheckConfigError_InvalidParallelism variant_node
class T49_infrastructure_infrastructure_DryCheckConfigError_InvalidReasoningEffort variant_node
class T49_infrastructure_infrastructure_DryCheckConfigError_InvalidPercent variant_node
class T49_infrastructure_infrastructure_DryCheckConfigError__self error_type
```
