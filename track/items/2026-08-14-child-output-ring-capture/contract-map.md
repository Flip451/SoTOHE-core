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
  subgraph usecase_usecase_module_program_runner["usecase::program_runner"]
    direction TB
  subgraph T37_usecase_usecase_CapturedProgramOutput["program_runner::CapturedProgramOutput"]
    direction TB
    T37_usecase_usecase_CapturedProgramOutput__self[CapturedProgramOutput]
  end
  subgraph T36_usecase_usecase_CapturedStreamOutput["program_runner::CapturedStreamOutput"]
    direction TB
    T36_usecase_usecase_CapturedStreamOutput__self[CapturedStreamOutput]
    T36_usecase_usecase_CapturedStreamOutput_Complete[Complete]
    T36_usecase_usecase_CapturedStreamOutput_TruncatedTail[TruncatedTail]
  end
  subgraph T44_usecase_usecase_FailedProgramExecutionRecord["program_runner::FailedProgramExecutionRecord"]
    direction TB
    T44_usecase_usecase_FailedProgramExecutionRecord__self[FailedProgramExecutionRecord]
  end
  subgraph T38_usecase_usecase_ProgramExecutionRecord["program_runner::ProgramExecutionRecord"]
    direction TB
    T38_usecase_usecase_ProgramExecutionRecord__self[ProgramExecutionRecord]
    T38_usecase_usecase_ProgramExecutionRecord_classify([classify])
  end
  subgraph T33_usecase_usecase_ProgramRunOutcome["program_runner::ProgramRunOutcome"]
    direction TB
    T33_usecase_usecase_ProgramRunOutcome__self[ProgramRunOutcome]
    T33_usecase_usecase_ProgramRunOutcome_Exited[Exited]
    T33_usecase_usecase_ProgramRunOutcome_TimedOut[TimedOut]
  end
  subgraph T48_usecase_usecase_SuccessfulProgramExecutionRecord["program_runner::SuccessfulProgramExecutionRecord"]
    direction TB
    T48_usecase_usecase_SuccessfulProgramExecutionRecord__self[SuccessfulProgramExecutionRecord]
  end
  subgraph R33_usecase_usecase_ProgramRunnerPort["program_runner::ProgramRunnerPort"]
    direction TB
    R33_usecase_usecase_ProgramRunnerPort__self[ProgramRunnerPort]
    R33_usecase_usecase_ProgramRunnerPort_run([run])
  end
  end
  subgraph usecase_usecase_module_review_v2["usecase::review_v2"]
    direction TB
  subgraph R31_usecase_usecase_ReviewFixRunner["review_v2::run_review_fix::ReviewFixRunner"]
    direction TB
    R31_usecase_usecase_ReviewFixRunner__self[ReviewFixRunner]
    R31_usecase_usecase_ReviewFixRunner_run_fix([run_fix])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_program_runner["infrastructure::program_runner"]
    direction TB
  subgraph T50_infrastructure_infrastructure_ProcessProgramRunner["program_runner::ProcessProgramRunner"]
    direction TB
    T50_infrastructure_infrastructure_ProcessProgramRunner__self[ProcessProgramRunner]
    T50_infrastructure_infrastructure_ProcessProgramRunner_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  subgraph T50_infrastructure_infrastructure_CodexReviewFixRunner["review_v2::review_fix_runner::CodexReviewFixRunner"]
    direction TB
    T50_infrastructure_infrastructure_CodexReviewFixRunner__self[CodexReviewFixRunner]
    T50_infrastructure_infrastructure_CodexReviewFixRunner_new([new])
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_phase_command["cli_driver::phase_command"]
    direction TB
  subgraph T40_cli_driver_cli_driver_PhaseCommandDriver["phase_command::PhaseCommandDriver"]
    direction TB
    T40_cli_driver_cli_driver_PhaseCommandDriver__self[PhaseCommandDriver]
    T40_cli_driver_cli_driver_PhaseCommandDriver_new([new])
    T40_cli_driver_cli_driver_PhaseCommandDriver_handle([handle])
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
end
subgraph cli["cli"]
  direction TB
end
T37_usecase_usecase_CapturedProgramOutput__self --o|stdout| T36_usecase_usecase_CapturedStreamOutput__self
T37_usecase_usecase_CapturedProgramOutput__self --o|stderr| T36_usecase_usecase_CapturedStreamOutput__self
T38_usecase_usecase_ProgramExecutionRecord__self --o|outcome| T33_usecase_usecase_ProgramRunOutcome__self
T33_usecase_usecase_ProgramRunOutcome_Exited --o|output| T37_usecase_usecase_CapturedProgramOutput__self
T33_usecase_usecase_ProgramRunOutcome_TimedOut --o|output| T37_usecase_usecase_CapturedProgramOutput__self
R33_usecase_usecase_ProgramRunnerPort_run --> T33_usecase_usecase_ProgramRunOutcome__self
T50_infrastructure_infrastructure_ProcessProgramRunner_new --> T50_infrastructure_infrastructure_ProcessProgramRunner__self
T50_infrastructure_infrastructure_CodexReviewFixRunner_new --> T50_infrastructure_infrastructure_CodexReviewFixRunner__self
T50_infrastructure_infrastructure_ProcessProgramRunner__self -.impl.-> R33_usecase_usecase_ProgramRunnerPort__self
T50_infrastructure_infrastructure_CodexReviewFixRunner__self -.impl.-> R31_usecase_usecase_ReviewFixRunner__self
T40_cli_driver_cli_driver_PhaseCommandDriver_new --> T40_cli_driver_cli_driver_PhaseCommandDriver__self
class T37_usecase_usecase_CapturedProgramOutput__self dto
class T36_usecase_usecase_CapturedStreamOutput_Complete variant_node
class T36_usecase_usecase_CapturedStreamOutput_TruncatedTail variant_node
class T36_usecase_usecase_CapturedStreamOutput__self dto
class T44_usecase_usecase_FailedProgramExecutionRecord__self dto
class T38_usecase_usecase_ProgramExecutionRecord_classify method_node
class T38_usecase_usecase_ProgramExecutionRecord__self dto
class T33_usecase_usecase_ProgramRunOutcome_Exited variant_node
class T33_usecase_usecase_ProgramRunOutcome_TimedOut variant_node
class T33_usecase_usecase_ProgramRunOutcome__self dto
class T48_usecase_usecase_SuccessfulProgramExecutionRecord__self dto
class R33_usecase_usecase_ProgramRunnerPort_run method_node
class R33_usecase_usecase_ProgramRunnerPort__self secondary_port
class R31_usecase_usecase_ReviewFixRunner_run_fix method_node
class R31_usecase_usecase_ReviewFixRunner__self secondary_port
class T50_infrastructure_infrastructure_ProcessProgramRunner_new method_node
class T50_infrastructure_infrastructure_ProcessProgramRunner__self secondary_adapter
class T50_infrastructure_infrastructure_CodexReviewFixRunner_new method_node
class T50_infrastructure_infrastructure_CodexReviewFixRunner__self secondary_adapter
class T40_cli_driver_cli_driver_PhaseCommandDriver_new method_node
class T40_cli_driver_cli_driver_PhaseCommandDriver_handle method_node
class T40_cli_driver_cli_driver_PhaseCommandDriver__self primary_adapter
```
