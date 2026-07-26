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
  subgraph usecase_usecase_module_pr["usecase::pr"]
    direction TB
  subgraph T25_usecase_usecase_PrCommand["pr::PrCommand"]
    direction TB
    T25_usecase_usecase_PrCommand__self[PrCommand]
    T25_usecase_usecase_PrCommand_Push[Push]
    T25_usecase_usecase_PrCommand_Ensure[Ensure]
    T25_usecase_usecase_PrCommand_Status[Status]
    T25_usecase_usecase_PrCommand_WaitAndMerge[WaitAndMerge]
    T25_usecase_usecase_PrCommand_TriggerReview[TriggerReview]
    T25_usecase_usecase_PrCommand_PollReview[PollReview]
    T25_usecase_usecase_PrCommand_ReviewCycle[ReviewCycle]
  end
  subgraph T35_usecase_usecase_PrCommandInteractor["pr::PrCommandInteractor"]
    direction TB
    T35_usecase_usecase_PrCommandInteractor__self[PrCommandInteractor]
    T35_usecase_usecase_PrCommandInteractor_new([new])
  end
  subgraph T31_usecase_usecase_PrCommandOutput["pr::PrCommandOutput"]
    direction TB
    T31_usecase_usecase_PrCommandOutput__self[PrCommandOutput]
    T31_usecase_usecase_PrCommandOutput_success([success])
    T31_usecase_usecase_PrCommandOutput_failure([failure])
    T31_usecase_usecase_PrCommandOutput_with_exit_code([with_exit_code])
  end
  subgraph T28_usecase_usecase_PrIdentifier["pr::PrIdentifier"]
    direction TB
    T28_usecase_usecase_PrIdentifier__self[PrIdentifier]
    T28_usecase_usecase_PrIdentifier_try_new([try_new])
    T28_usecase_usecase_PrIdentifier_as_str([as_str])
    T28_usecase_usecase_PrIdentifier_is_valid([is_valid])
  end
  subgraph T37_usecase_usecase_PrPollIntervalSeconds["pr::PrPollIntervalSeconds"]
    direction TB
    T37_usecase_usecase_PrPollIntervalSeconds__self[PrPollIntervalSeconds]
    T37_usecase_usecase_PrPollIntervalSeconds_try_new([try_new])
    T37_usecase_usecase_PrPollIntervalSeconds_as_secs([as_secs])
    T37_usecase_usecase_PrPollIntervalSeconds_is_valid([is_valid])
  end
  subgraph T36_usecase_usecase_PrPollTimeoutSeconds["pr::PrPollTimeoutSeconds"]
    direction TB
    T36_usecase_usecase_PrPollTimeoutSeconds__self[PrPollTimeoutSeconds]
    T36_usecase_usecase_PrPollTimeoutSeconds_try_new([try_new])
    T36_usecase_usecase_PrPollTimeoutSeconds_as_secs([as_secs])
    T36_usecase_usecase_PrPollTimeoutSeconds_is_valid([is_valid])
  end
  subgraph T33_usecase_usecase_PrReviewCycleMode["pr::PrReviewCycleMode"]
    direction TB
    T33_usecase_usecase_PrReviewCycleMode__self[PrReviewCycleMode]
    T33_usecase_usecase_PrReviewCycleMode_Start[Start]
    T33_usecase_usecase_PrReviewCycleMode_Resume[Resume]
  end
  subgraph R29_usecase_usecase_PrCommandPort["pr::PrCommandPort"]
    direction TB
    R29_usecase_usecase_PrCommandPort__self[PrCommandPort]
    R29_usecase_usecase_PrCommandPort_execute([execute])
  end
  end
  subgraph usecase_usecase_module_signal_service["usecase::signal_service"]
    direction TB
  subgraph T29_usecase_usecase_SignalCommand["signal_service::SignalCommand"]
    direction TB
    T29_usecase_usecase_SignalCommand__self[SignalCommand]
    T29_usecase_usecase_SignalCommand_CalcAdrUser[CalcAdrUser]
    T29_usecase_usecase_SignalCommand_CheckAdrUser[CheckAdrUser]
    T29_usecase_usecase_SignalCommand_CalcSpecAdr[CalcSpecAdr]
    T29_usecase_usecase_SignalCommand_CheckSpecAdr[CheckSpecAdr]
    T29_usecase_usecase_SignalCommand_CalcCatalogSpec[CalcCatalogSpec]
    T29_usecase_usecase_SignalCommand_CheckCatalogSpec[CheckCatalogSpec]
    T29_usecase_usecase_SignalCommand_CalcImplCatalog[CalcImplCatalog]
    T29_usecase_usecase_SignalCommand_CheckImplCatalog[CheckImplCatalog]
    T29_usecase_usecase_SignalCommand_CheckGate[CheckGate]
  end
  subgraph T39_usecase_usecase_SignalCommandInteractor["signal_service::SignalCommandInteractor"]
    direction TB
    T39_usecase_usecase_SignalCommandInteractor__self[SignalCommandInteractor]
    T39_usecase_usecase_SignalCommandInteractor_new([new])
  end
  subgraph T35_usecase_usecase_SignalCommandOutput["signal_service::SignalCommandOutput"]
    direction TB
    T35_usecase_usecase_SignalCommandOutput__self[SignalCommandOutput]
    T35_usecase_usecase_SignalCommandOutput_success([success])
    T35_usecase_usecase_SignalCommandOutput_failure([failure])
  end
  subgraph R33_usecase_usecase_SignalCommandPort["signal_service::SignalCommandPort"]
    direction TB
    R33_usecase_usecase_SignalCommandPort__self[SignalCommandPort]
    R33_usecase_usecase_SignalCommandPort_execute([execute])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_pr["infrastructure::pr"]
    direction TB
  subgraph T52_infrastructure_infrastructure_SystemPrCommandAdapter["pr::SystemPrCommandAdapter"]
    direction TB
    T52_infrastructure_infrastructure_SystemPrCommandAdapter__self[SystemPrCommandAdapter]
    T52_infrastructure_infrastructure_SystemPrCommandAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_signal["infrastructure::signal"]
    direction TB
  subgraph T56_infrastructure_infrastructure_SystemSignalCommandAdapter["signal::SystemSignalCommandAdapter"]
    direction TB
    T56_infrastructure_infrastructure_SystemSignalCommandAdapter__self[SystemSignalCommandAdapter]
    T56_infrastructure_infrastructure_SystemSignalCommandAdapter_new([new])
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_pr["cli_driver::pr"]
    direction TB
  subgraph T30_cli_driver_cli_driver_PrDriver["pr::PrDriver"]
    direction TB
    T30_cli_driver_cli_driver_PrDriver__self[PrDriver]
    T30_cli_driver_cli_driver_PrDriver_new([new])
    T30_cli_driver_cli_driver_PrDriver_handle([handle])
  end
  end
  subgraph cli_driver_cli_driver_module_signal["cli_driver::signal"]
    direction TB
  subgraph T34_cli_driver_cli_driver_SignalDriver["signal::SignalDriver"]
    direction TB
    T34_cli_driver_cli_driver_SignalDriver__self[SignalDriver]
    T34_cli_driver_cli_driver_SignalDriver_new([new])
    T34_cli_driver_cli_driver_SignalDriver_handle([handle])
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_pr["cli_composition::pr"]
    direction TB
  subgraph T49_cli_composition_cli_composition_PrCompositionRoot["pr::PrCompositionRoot"]
    direction TB
    T49_cli_composition_cli_composition_PrCompositionRoot__self[PrCompositionRoot]
    T49_cli_composition_cli_composition_PrCompositionRoot_pr_driver([pr_driver])
    T49_cli_composition_cli_composition_PrCompositionRoot_new([new])
  end
  end
  subgraph cli_composition_cli_composition_module_signal["cli_composition::signal"]
    direction TB
  subgraph T53_cli_composition_cli_composition_SignalCompositionRoot["signal::SignalCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_SignalCompositionRoot__self[SignalCompositionRoot]
    T53_cli_composition_cli_composition_SignalCompositionRoot_new([new])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver([signal_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
end
T25_usecase_usecase_PrCommand_Status --o T28_usecase_usecase_PrIdentifier__self
T25_usecase_usecase_PrCommand_WaitAndMerge --o|pr| T28_usecase_usecase_PrIdentifier__self
T25_usecase_usecase_PrCommand_WaitAndMerge --o|interval| T37_usecase_usecase_PrPollIntervalSeconds__self
T25_usecase_usecase_PrCommand_WaitAndMerge --o|timeout| T36_usecase_usecase_PrPollTimeoutSeconds__self
T25_usecase_usecase_PrCommand_TriggerReview --o T28_usecase_usecase_PrIdentifier__self
T25_usecase_usecase_PrCommand_PollReview --o|pr| T28_usecase_usecase_PrIdentifier__self
T25_usecase_usecase_PrCommand_PollReview --o|interval| T37_usecase_usecase_PrPollIntervalSeconds__self
T25_usecase_usecase_PrCommand_PollReview --o|timeout| T36_usecase_usecase_PrPollTimeoutSeconds__self
T25_usecase_usecase_PrCommand_ReviewCycle --o|mode| T33_usecase_usecase_PrReviewCycleMode__self
T35_usecase_usecase_PrCommandInteractor_new --o R29_usecase_usecase_PrCommandPort__self
T35_usecase_usecase_PrCommandInteractor_new --> T35_usecase_usecase_PrCommandInteractor__self
T31_usecase_usecase_PrCommandOutput_success --> T31_usecase_usecase_PrCommandOutput__self
T31_usecase_usecase_PrCommandOutput_failure --> T31_usecase_usecase_PrCommandOutput__self
T31_usecase_usecase_PrCommandOutput_with_exit_code --> T31_usecase_usecase_PrCommandOutput__self
T28_usecase_usecase_PrIdentifier_try_new --> T28_usecase_usecase_PrIdentifier__self
T37_usecase_usecase_PrPollIntervalSeconds_try_new --> T37_usecase_usecase_PrPollIntervalSeconds__self
T36_usecase_usecase_PrPollTimeoutSeconds_try_new --> T36_usecase_usecase_PrPollTimeoutSeconds__self
R29_usecase_usecase_PrCommandPort_execute --o T25_usecase_usecase_PrCommand__self
R29_usecase_usecase_PrCommandPort_execute --> T31_usecase_usecase_PrCommandOutput__self
T39_usecase_usecase_SignalCommandInteractor_new --o R33_usecase_usecase_SignalCommandPort__self
T39_usecase_usecase_SignalCommandInteractor_new --> T39_usecase_usecase_SignalCommandInteractor__self
T35_usecase_usecase_SignalCommandOutput_success --> T35_usecase_usecase_SignalCommandOutput__self
T35_usecase_usecase_SignalCommandOutput_failure --> T35_usecase_usecase_SignalCommandOutput__self
R33_usecase_usecase_SignalCommandPort_execute --o T29_usecase_usecase_SignalCommand__self
R33_usecase_usecase_SignalCommandPort_execute --> T35_usecase_usecase_SignalCommandOutput__self
T52_infrastructure_infrastructure_SystemPrCommandAdapter_new --> T52_infrastructure_infrastructure_SystemPrCommandAdapter__self
T56_infrastructure_infrastructure_SystemSignalCommandAdapter_new --> T56_infrastructure_infrastructure_SystemSignalCommandAdapter__self
T52_infrastructure_infrastructure_SystemPrCommandAdapter__self -.impl.-> R29_usecase_usecase_PrCommandPort__self
T56_infrastructure_infrastructure_SystemSignalCommandAdapter__self -.impl.-> R33_usecase_usecase_SignalCommandPort__self
T30_cli_driver_cli_driver_PrDriver_new --> T30_cli_driver_cli_driver_PrDriver__self
T34_cli_driver_cli_driver_SignalDriver_new --> T34_cli_driver_cli_driver_SignalDriver__self
T49_cli_composition_cli_composition_PrCompositionRoot_pr_driver --> T30_cli_driver_cli_driver_PrDriver__self
T49_cli_composition_cli_composition_PrCompositionRoot_new --> T49_cli_composition_cli_composition_PrCompositionRoot__self
T53_cli_composition_cli_composition_SignalCompositionRoot_new --> T53_cli_composition_cli_composition_SignalCompositionRoot__self
T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver --> T34_cli_driver_cli_driver_SignalDriver__self
class T25_usecase_usecase_PrCommand_Push variant_node
class T25_usecase_usecase_PrCommand_Ensure variant_node
class T25_usecase_usecase_PrCommand_Status variant_node
class T25_usecase_usecase_PrCommand_WaitAndMerge variant_node
class T25_usecase_usecase_PrCommand_TriggerReview variant_node
class T25_usecase_usecase_PrCommand_PollReview variant_node
class T25_usecase_usecase_PrCommand_ReviewCycle variant_node
class T25_usecase_usecase_PrCommand__self command
class T35_usecase_usecase_PrCommandInteractor_new method_node
class T35_usecase_usecase_PrCommandInteractor__self interactor
class T31_usecase_usecase_PrCommandOutput_success method_node
class T31_usecase_usecase_PrCommandOutput_failure method_node
class T31_usecase_usecase_PrCommandOutput_with_exit_code method_node
class T31_usecase_usecase_PrCommandOutput__self dto
class T28_usecase_usecase_PrIdentifier_try_new method_node
class T28_usecase_usecase_PrIdentifier_as_str method_node
class T28_usecase_usecase_PrIdentifier_is_valid method_node
class T28_usecase_usecase_PrIdentifier__self value_object
class T37_usecase_usecase_PrPollIntervalSeconds_try_new method_node
class T37_usecase_usecase_PrPollIntervalSeconds_as_secs method_node
class T37_usecase_usecase_PrPollIntervalSeconds_is_valid method_node
class T37_usecase_usecase_PrPollIntervalSeconds__self value_object
class T36_usecase_usecase_PrPollTimeoutSeconds_try_new method_node
class T36_usecase_usecase_PrPollTimeoutSeconds_as_secs method_node
class T36_usecase_usecase_PrPollTimeoutSeconds_is_valid method_node
class T36_usecase_usecase_PrPollTimeoutSeconds__self value_object
class T33_usecase_usecase_PrReviewCycleMode_Start variant_node
class T33_usecase_usecase_PrReviewCycleMode_Resume variant_node
class T33_usecase_usecase_PrReviewCycleMode__self value_object
class R29_usecase_usecase_PrCommandPort_execute method_node
class R29_usecase_usecase_PrCommandPort__self secondary_port
class T29_usecase_usecase_SignalCommand_CalcAdrUser variant_node
class T29_usecase_usecase_SignalCommand_CheckAdrUser variant_node
class T29_usecase_usecase_SignalCommand_CalcSpecAdr variant_node
class T29_usecase_usecase_SignalCommand_CheckSpecAdr variant_node
class T29_usecase_usecase_SignalCommand_CalcCatalogSpec variant_node
class T29_usecase_usecase_SignalCommand_CheckCatalogSpec variant_node
class T29_usecase_usecase_SignalCommand_CalcImplCatalog variant_node
class T29_usecase_usecase_SignalCommand_CheckImplCatalog variant_node
class T29_usecase_usecase_SignalCommand_CheckGate variant_node
class T29_usecase_usecase_SignalCommand__self command
class T39_usecase_usecase_SignalCommandInteractor_new method_node
class T39_usecase_usecase_SignalCommandInteractor__self interactor
class T35_usecase_usecase_SignalCommandOutput_success method_node
class T35_usecase_usecase_SignalCommandOutput_failure method_node
class T35_usecase_usecase_SignalCommandOutput__self dto
class R33_usecase_usecase_SignalCommandPort_execute method_node
class R33_usecase_usecase_SignalCommandPort__self secondary_port
class T52_infrastructure_infrastructure_SystemPrCommandAdapter_new method_node
class T52_infrastructure_infrastructure_SystemPrCommandAdapter__self secondary_adapter
class T56_infrastructure_infrastructure_SystemSignalCommandAdapter_new method_node
class T56_infrastructure_infrastructure_SystemSignalCommandAdapter__self secondary_adapter
class T30_cli_driver_cli_driver_PrDriver_new method_node
class T30_cli_driver_cli_driver_PrDriver_handle method_node
class T30_cli_driver_cli_driver_PrDriver__self primary_adapter
class T34_cli_driver_cli_driver_SignalDriver_new method_node
class T34_cli_driver_cli_driver_SignalDriver_handle method_node
class T34_cli_driver_cli_driver_SignalDriver__self primary_adapter
class T49_cli_composition_cli_composition_PrCompositionRoot_pr_driver method_node
class T49_cli_composition_cli_composition_PrCompositionRoot_new method_node
class T49_cli_composition_cli_composition_PrCompositionRoot__self composition_root
class T53_cli_composition_cli_composition_SignalCompositionRoot_new method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot__self composition_root
```
