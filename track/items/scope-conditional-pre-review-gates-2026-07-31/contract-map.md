<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
---
config:
  layout: elk
---
flowchart LR
classDef aggregate_root fill:#ede9fe,stroke:#4c1d95,stroke-width:2px
classDef app_service fill:#ecfdf5,stroke:#059669,stroke-width:2px
classDef command fill:#fff7ed,stroke:#c2410c,stroke-width:1px
classDef composition_root fill:#e0e7ff,stroke:#3730a3,stroke-width:2px
classDef domain_event fill:#fce7f3,stroke:#9d174d,stroke-width:1px
classDef domain_service fill:#fee2e2,stroke:#991b1b,stroke-width:1px
classDef dto fill:#f8fafc,stroke:#64748b,stroke-width:1px
classDef entity fill:#dbeafe,stroke:#1e40af,stroke-width:2px
classDef error_type fill:#fef2f2,stroke:#b91c1c,stroke-width:1px,stroke-dasharray:4 2
classDef event_policy fill:#fef3c7,stroke:#92400e,stroke-width:1px
classDef factory fill:#e0f2fe,stroke:#0369a1,stroke-width:1px
classDef free_function fill:#f5f3ff,stroke:#7c3aed,stroke-width:1px
classDef function_node fill:#f5f3ff,stroke:#a78bfa,stroke-width:1px
classDef interactor fill:#f0fdfa,stroke:#0d9488,stroke-width:1px
classDef method_node fill:#f8fafc,stroke:#cbd5e1,stroke-width:1px
classDef primary_adapter fill:#ecfccb,stroke:#3f6212,stroke-width:1px
classDef query fill:#f0f9ff,stroke:#0369a1,stroke-width:1px
classDef repository fill:#f5f3ff,stroke:#6d28d9,stroke-width:1px
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
  subgraph usecase_usecase_module_pre_review_gate_dispatch["usecase::pre_review_gate_dispatch"]
    direction TB
  subgraph T44_usecase_usecase_PreReviewGateConfigLoadError["pre_review_gate_dispatch::PreReviewGateConfigLoadError"]
    direction TB
    T44_usecase_usecase_PreReviewGateConfigLoadError__self[PreReviewGateConfigLoadError]
    T44_usecase_usecase_PreReviewGateConfigLoadError_ReadFailed[ReadFailed]
    T44_usecase_usecase_PreReviewGateConfigLoadError_InvalidMatrix[InvalidMatrix]
  end
  subgraph T44_usecase_usecase_PreReviewGateDispatchCommand["pre_review_gate_dispatch::PreReviewGateDispatchCommand"]
    direction TB
    T44_usecase_usecase_PreReviewGateDispatchCommand__self[PreReviewGateDispatchCommand]
  end
  subgraph T42_usecase_usecase_PreReviewGateDispatchError["pre_review_gate_dispatch::PreReviewGateDispatchError"]
    direction TB
    T42_usecase_usecase_PreReviewGateDispatchError__self[PreReviewGateDispatchError]
    T42_usecase_usecase_PreReviewGateDispatchError_Config[Config]
    T42_usecase_usecase_PreReviewGateDispatchError_TaskContract[TaskContract]
    T42_usecase_usecase_PreReviewGateDispatchError_Lookup[Lookup]
  end
  subgraph T47_usecase_usecase_PreReviewGateDispatchInteractor["pre_review_gate_dispatch::PreReviewGateDispatchInteractor"]
    direction TB
    T47_usecase_usecase_PreReviewGateDispatchInteractor__self[PreReviewGateDispatchInteractor]
    T47_usecase_usecase_PreReviewGateDispatchInteractor_new([new])
  end
  subgraph T44_usecase_usecase_PreReviewGateDispatchOutcome["pre_review_gate_dispatch::PreReviewGateDispatchOutcome"]
    direction TB
    T44_usecase_usecase_PreReviewGateDispatchOutcome__self[PreReviewGateDispatchOutcome]
    T44_usecase_usecase_PreReviewGateDispatchOutcome_NotApplicable[NotApplicable]
    T44_usecase_usecase_PreReviewGateDispatchOutcome_TaskContract[TaskContract]
  end
  subgraph T33_usecase_usecase_PreReviewGateKind["pre_review_gate_dispatch::PreReviewGateKind"]
    direction TB
    T33_usecase_usecase_PreReviewGateKind__self[PreReviewGateKind]
    T33_usecase_usecase_PreReviewGateKind_TaskContractLiveness[TaskContractLiveness]
  end
  subgraph T40_usecase_usecase_PreReviewGateLookupError["pre_review_gate_dispatch::PreReviewGateLookupError"]
    direction TB
    T40_usecase_usecase_PreReviewGateLookupError__self[PreReviewGateLookupError]
    T40_usecase_usecase_PreReviewGateLookupError_UnknownScope[UnknownScope]
  end
  subgraph T35_usecase_usecase_PreReviewGateMatrix["pre_review_gate_dispatch::PreReviewGateMatrix"]
    direction TB
    T35_usecase_usecase_PreReviewGateMatrix__self[PreReviewGateMatrix]
    T35_usecase_usecase_PreReviewGateMatrix_try_new([try_new])
    T35_usecase_usecase_PreReviewGateMatrix_gates_for([gates_for])
  end
  subgraph T40_usecase_usecase_PreReviewGateMatrixError["pre_review_gate_dispatch::PreReviewGateMatrixError"]
    direction TB
    T40_usecase_usecase_PreReviewGateMatrixError__self[PreReviewGateMatrixError]
    T40_usecase_usecase_PreReviewGateMatrixError_MissingScope[MissingScope]
    T40_usecase_usecase_PreReviewGateMatrixError_UnknownScope[UnknownScope]
    T40_usecase_usecase_PreReviewGateMatrixError_DuplicateScope[DuplicateScope]
    T40_usecase_usecase_PreReviewGateMatrixError_DuplicateGate[DuplicateGate]
  end
  subgraph R45_usecase_usecase_PreReviewGateConfigLoaderPort["pre_review_gate_dispatch::PreReviewGateConfigLoaderPort"]
    direction TB
    R45_usecase_usecase_PreReviewGateConfigLoaderPort__self[PreReviewGateConfigLoaderPort]
    R45_usecase_usecase_PreReviewGateConfigLoaderPort_load([load])
  end
  subgraph R44_usecase_usecase_PreReviewGateDispatchService["pre_review_gate_dispatch::PreReviewGateDispatchService"]
    direction TB
    R44_usecase_usecase_PreReviewGateDispatchService__self[PreReviewGateDispatchService]
    R44_usecase_usecase_PreReviewGateDispatchService_dispatch([dispatch])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_pre_review_gate_config["infrastructure::pre_review_gate_config"]
    direction TB
  subgraph T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader["pre_review_gate_config::FsPreReviewGateConfigLoader"]
    direction TB
    T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader__self[FsPreReviewGateConfigLoader]
    T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader_new([new])
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
end
subgraph cli_composition["cli_composition"]
  direction TB
end
subgraph cli["cli"]
  direction TB
end
T44_usecase_usecase_PreReviewGateConfigLoadError_InvalidMatrix --o T40_usecase_usecase_PreReviewGateMatrixError__self
T42_usecase_usecase_PreReviewGateDispatchError_Config --o T44_usecase_usecase_PreReviewGateConfigLoadError__self
T42_usecase_usecase_PreReviewGateDispatchError_Lookup --o T40_usecase_usecase_PreReviewGateLookupError__self
T47_usecase_usecase_PreReviewGateDispatchInteractor_new --o R45_usecase_usecase_PreReviewGateConfigLoaderPort__self
T47_usecase_usecase_PreReviewGateDispatchInteractor_new --> T47_usecase_usecase_PreReviewGateDispatchInteractor__self
T35_usecase_usecase_PreReviewGateMatrix_try_new --o T33_usecase_usecase_PreReviewGateKind__self
T35_usecase_usecase_PreReviewGateMatrix_try_new --> T40_usecase_usecase_PreReviewGateMatrixError__self
T35_usecase_usecase_PreReviewGateMatrix_try_new --> T35_usecase_usecase_PreReviewGateMatrix__self
T35_usecase_usecase_PreReviewGateMatrix_gates_for --> T33_usecase_usecase_PreReviewGateKind__self
T35_usecase_usecase_PreReviewGateMatrix_gates_for --> T40_usecase_usecase_PreReviewGateLookupError__self
T40_usecase_usecase_PreReviewGateMatrixError_DuplicateGate --o|gate| T33_usecase_usecase_PreReviewGateKind__self
R45_usecase_usecase_PreReviewGateConfigLoaderPort_load --> T44_usecase_usecase_PreReviewGateConfigLoadError__self
R45_usecase_usecase_PreReviewGateConfigLoaderPort_load --> T35_usecase_usecase_PreReviewGateMatrix__self
R44_usecase_usecase_PreReviewGateDispatchService_dispatch --o T44_usecase_usecase_PreReviewGateDispatchCommand__self
R44_usecase_usecase_PreReviewGateDispatchService_dispatch --> T42_usecase_usecase_PreReviewGateDispatchError__self
R44_usecase_usecase_PreReviewGateDispatchService_dispatch --> T44_usecase_usecase_PreReviewGateDispatchOutcome__self
T47_usecase_usecase_PreReviewGateDispatchInteractor__self -.impl.-> R44_usecase_usecase_PreReviewGateDispatchService__self
T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader_new --> T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader__self
T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader__self -.impl.-> R45_usecase_usecase_PreReviewGateConfigLoaderPort__self
class T44_usecase_usecase_PreReviewGateConfigLoadError_ReadFailed variant_node
class T44_usecase_usecase_PreReviewGateConfigLoadError_InvalidMatrix variant_node
class T44_usecase_usecase_PreReviewGateConfigLoadError__self error_type
class T44_usecase_usecase_PreReviewGateDispatchCommand__self command
class T42_usecase_usecase_PreReviewGateDispatchError_Config variant_node
class T42_usecase_usecase_PreReviewGateDispatchError_TaskContract variant_node
class T42_usecase_usecase_PreReviewGateDispatchError_Lookup variant_node
class T42_usecase_usecase_PreReviewGateDispatchError__self error_type
class T47_usecase_usecase_PreReviewGateDispatchInteractor_new method_node
class T47_usecase_usecase_PreReviewGateDispatchInteractor__self interactor
class T44_usecase_usecase_PreReviewGateDispatchOutcome_NotApplicable variant_node
class T44_usecase_usecase_PreReviewGateDispatchOutcome_TaskContract variant_node
class T44_usecase_usecase_PreReviewGateDispatchOutcome__self dto
class T33_usecase_usecase_PreReviewGateKind_TaskContractLiveness variant_node
class T33_usecase_usecase_PreReviewGateKind__self value_object
class T40_usecase_usecase_PreReviewGateLookupError_UnknownScope variant_node
class T40_usecase_usecase_PreReviewGateLookupError__self error_type
class T35_usecase_usecase_PreReviewGateMatrix_try_new method_node
class T35_usecase_usecase_PreReviewGateMatrix_gates_for method_node
class T35_usecase_usecase_PreReviewGateMatrix__self value_object
class T40_usecase_usecase_PreReviewGateMatrixError_MissingScope variant_node
class T40_usecase_usecase_PreReviewGateMatrixError_UnknownScope variant_node
class T40_usecase_usecase_PreReviewGateMatrixError_DuplicateScope variant_node
class T40_usecase_usecase_PreReviewGateMatrixError_DuplicateGate variant_node
class T40_usecase_usecase_PreReviewGateMatrixError__self error_type
class R45_usecase_usecase_PreReviewGateConfigLoaderPort_load method_node
class R45_usecase_usecase_PreReviewGateConfigLoaderPort__self secondary_port
class R44_usecase_usecase_PreReviewGateDispatchService_dispatch method_node
class R44_usecase_usecase_PreReviewGateDispatchService__self app_service
class T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader_new method_node
class T57_infrastructure_infrastructure_FsPreReviewGateConfigLoader__self secondary_adapter
```
