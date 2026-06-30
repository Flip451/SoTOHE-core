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
  subgraph usecase_usecase_module_pre_review_gate["usecase::pre_review_gate"]
    direction TB
  subgraph T37_usecase_usecase_CoverageVerifyCommand["pre_review_gate::CoverageVerifyCommand"]
    direction TB
    T37_usecase_usecase_CoverageVerifyCommand__self[CoverageVerifyCommand]
  end
  subgraph T40_usecase_usecase_CoverageVerifyInteractor["pre_review_gate::CoverageVerifyInteractor"]
    direction TB
    T40_usecase_usecase_CoverageVerifyInteractor__self[CoverageVerifyInteractor]
    T40_usecase_usecase_CoverageVerifyInteractor_new([new])
  end
  subgraph T36_usecase_usecase_PreReviewGateCommand["pre_review_gate::PreReviewGateCommand"]
    direction TB
    T36_usecase_usecase_PreReviewGateCommand__self[PreReviewGateCommand]
  end
  subgraph T34_usecase_usecase_PreReviewGateError["pre_review_gate::PreReviewGateError"]
    direction TB
    T34_usecase_usecase_PreReviewGateError__self[PreReviewGateError]
    T34_usecase_usecase_PreReviewGateError_TaskContractNotFound[TaskContractNotFound]
    T34_usecase_usecase_PreReviewGateError_TaskContractReadFailed[TaskContractReadFailed]
    T34_usecase_usecase_PreReviewGateError_SignalReadFailed[SignalReadFailed]
    T34_usecase_usecase_PreReviewGateError_ImplPlanReadFailed[ImplPlanReadFailed]
  end
  subgraph T39_usecase_usecase_PreReviewGateInteractor["pre_review_gate::PreReviewGateInteractor"]
    direction TB
    T39_usecase_usecase_PreReviewGateInteractor__self[PreReviewGateInteractor]
    T39_usecase_usecase_PreReviewGateInteractor_new([new])
  end
  subgraph R37_usecase_usecase_CoverageVerifyService["pre_review_gate::CoverageVerifyService"]
    direction TB
    R37_usecase_usecase_CoverageVerifyService__self[CoverageVerifyService]
    R37_usecase_usecase_CoverageVerifyService_verify_coverage([verify_coverage])
  end
  subgraph R43_usecase_usecase_ImplCatalogSignalReaderPort["pre_review_gate::ImplCatalogSignalReaderPort"]
    direction TB
    R43_usecase_usecase_ImplCatalogSignalReaderPort__self[ImplCatalogSignalReaderPort]
    R43_usecase_usecase_ImplCatalogSignalReaderPort_read_signals([read_signals])
    R43_usecase_usecase_ImplCatalogSignalReaderPort_read_optional_signals([read_optional_signals])
  end
  subgraph R34_usecase_usecase_ImplPlanReaderPort["pre_review_gate::ImplPlanReaderPort"]
    direction TB
    R34_usecase_usecase_ImplPlanReaderPort__self[ImplPlanReaderPort]
    R34_usecase_usecase_ImplPlanReaderPort_read_task_statuses([read_task_statuses])
  end
  subgraph R36_usecase_usecase_PreReviewGateService["pre_review_gate::PreReviewGateService"]
    direction TB
    R36_usecase_usecase_PreReviewGateService__self[PreReviewGateService]
    R36_usecase_usecase_PreReviewGateService_check([check])
  end
  subgraph R38_usecase_usecase_TaskContractReaderPort["pre_review_gate::TaskContractReaderPort"]
    direction TB
    R38_usecase_usecase_TaskContractReaderPort__self[TaskContractReaderPort]
    R38_usecase_usecase_TaskContractReaderPort_read([read])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_task_contract["cli_driver::task_contract"]
    direction TB
  subgraph T40_cli_driver_cli_driver_TaskContractDriver["task_contract::TaskContractDriver"]
    direction TB
    T40_cli_driver_cli_driver_TaskContractDriver__self[TaskContractDriver]
    T40_cli_driver_cli_driver_TaskContractDriver_new([new])
    T40_cli_driver_cli_driver_TaskContractDriver_handle([handle])
  end
  subgraph T39_cli_driver_cli_driver_TaskContractInput["task_contract::TaskContractInput"]
    direction TB
    T39_cli_driver_cli_driver_TaskContractInput__self[TaskContractInput]
    T39_cli_driver_cli_driver_TaskContractInput_Check[Check]
    T39_cli_driver_cli_driver_TaskContractInput_Coverage[Coverage]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
end
subgraph cli["cli"]
  direction TB
end
T40_usecase_usecase_CoverageVerifyInteractor_new --> T40_usecase_usecase_CoverageVerifyInteractor__self
T39_usecase_usecase_PreReviewGateInteractor_new --> T39_usecase_usecase_PreReviewGateInteractor__self
R37_usecase_usecase_CoverageVerifyService_verify_coverage --o T37_usecase_usecase_CoverageVerifyCommand__self
R37_usecase_usecase_CoverageVerifyService_verify_coverage --> T34_usecase_usecase_PreReviewGateError__self
R43_usecase_usecase_ImplCatalogSignalReaderPort_read_signals --> T34_usecase_usecase_PreReviewGateError__self
R43_usecase_usecase_ImplCatalogSignalReaderPort_read_optional_signals --> T34_usecase_usecase_PreReviewGateError__self
R34_usecase_usecase_ImplPlanReaderPort_read_task_statuses --> T34_usecase_usecase_PreReviewGateError__self
R36_usecase_usecase_PreReviewGateService_check --o T36_usecase_usecase_PreReviewGateCommand__self
R36_usecase_usecase_PreReviewGateService_check --> T34_usecase_usecase_PreReviewGateError__self
R38_usecase_usecase_TaskContractReaderPort_read --> T34_usecase_usecase_PreReviewGateError__self
T39_usecase_usecase_PreReviewGateInteractor__self -.impl.-> R36_usecase_usecase_PreReviewGateService__self
T40_usecase_usecase_CoverageVerifyInteractor__self -.impl.-> R37_usecase_usecase_CoverageVerifyService__self
T40_cli_driver_cli_driver_TaskContractDriver_new --> T40_cli_driver_cli_driver_TaskContractDriver__self
T40_cli_driver_cli_driver_TaskContractDriver_handle --o T39_cli_driver_cli_driver_TaskContractInput__self
class T37_usecase_usecase_CoverageVerifyCommand__self command
class T40_usecase_usecase_CoverageVerifyInteractor_new method_node
class T40_usecase_usecase_CoverageVerifyInteractor__self interactor
class T36_usecase_usecase_PreReviewGateCommand__self command
class T34_usecase_usecase_PreReviewGateError_TaskContractNotFound variant_node
class T34_usecase_usecase_PreReviewGateError_TaskContractReadFailed variant_node
class T34_usecase_usecase_PreReviewGateError_SignalReadFailed variant_node
class T34_usecase_usecase_PreReviewGateError_ImplPlanReadFailed variant_node
class T34_usecase_usecase_PreReviewGateError__self error_type
class T39_usecase_usecase_PreReviewGateInteractor_new method_node
class T39_usecase_usecase_PreReviewGateInteractor__self interactor
class R37_usecase_usecase_CoverageVerifyService_verify_coverage method_node
class R37_usecase_usecase_CoverageVerifyService__self app_service
class R43_usecase_usecase_ImplCatalogSignalReaderPort_read_signals method_node
class R43_usecase_usecase_ImplCatalogSignalReaderPort_read_optional_signals method_node
class R43_usecase_usecase_ImplCatalogSignalReaderPort__self secondary_port
class R34_usecase_usecase_ImplPlanReaderPort_read_task_statuses method_node
class R34_usecase_usecase_ImplPlanReaderPort__self secondary_port
class R36_usecase_usecase_PreReviewGateService_check method_node
class R36_usecase_usecase_PreReviewGateService__self app_service
class R38_usecase_usecase_TaskContractReaderPort_read method_node
class R38_usecase_usecase_TaskContractReaderPort__self secondary_port
class T40_cli_driver_cli_driver_TaskContractDriver_new method_node
class T40_cli_driver_cli_driver_TaskContractDriver_handle method_node
class T39_cli_driver_cli_driver_TaskContractInput_Check variant_node
class T39_cli_driver_cli_driver_TaskContractInput_Coverage variant_node
class T39_cli_driver_cli_driver_TaskContractInput__self dto
```
