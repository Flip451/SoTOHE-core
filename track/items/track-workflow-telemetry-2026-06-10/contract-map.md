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
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_agent_profiles["infrastructure::agent_profiles"]
    direction TB
  subgraph T39_infrastructure_infrastructure_RoundType["agent_profiles::RoundType"]
    direction TB
    T39_infrastructure_infrastructure_RoundType__self[RoundType]
    T39_infrastructure_infrastructure_RoundType_Final[Final]
    T39_infrastructure_infrastructure_RoundType_Fast[Fast]
  end
  end
  subgraph infrastructure_infrastructure_module_telemetry["infrastructure::telemetry"]
    direction TB
  subgraph T50_infrastructure_infrastructure_PhaseDurationSummary["telemetry::PhaseDurationSummary"]
    direction TB
    T50_infrastructure_infrastructure_PhaseDurationSummary__self[PhaseDurationSummary]
  end
  subgraph T45_infrastructure_infrastructure_TelemetryConfig["telemetry::TelemetryConfig"]
    direction TB
    T45_infrastructure_infrastructure_TelemetryConfig__self[TelemetryConfig]
    T45_infrastructure_infrastructure_TelemetryConfig_from_env([from_env])
    T45_infrastructure_infrastructure_TelemetryConfig_is_enabled([is_enabled])
  end
  subgraph T49_infrastructure_infrastructure_TelemetryErrorEntry["telemetry::TelemetryErrorEntry"]
    direction TB
    T49_infrastructure_infrastructure_TelemetryErrorEntry__self[TelemetryErrorEntry]
  end
  subgraph T44_infrastructure_infrastructure_TelemetryEvent["telemetry::TelemetryEvent"]
    direction TB
    T44_infrastructure_infrastructure_TelemetryEvent__self[TelemetryEvent]
    T44_infrastructure_infrastructure_TelemetryEvent_TrackSubcommand[TrackSubcommand]
    T44_infrastructure_infrastructure_TelemetryEvent_GateEval[GateEval]
    T44_infrastructure_infrastructure_TelemetryEvent_ReviewRound[ReviewRound]
    T44_infrastructure_infrastructure_TelemetryEvent_ExternalSubprocess[ExternalSubprocess]
    T44_infrastructure_infrastructure_TelemetryEvent_HookBlock[HookBlock]
    T44_infrastructure_infrastructure_TelemetryEvent_AdvisoryHookFired[AdvisoryHookFired]
    T44_infrastructure_infrastructure_TelemetryEvent_NonZeroExit[NonZeroExit]
  end
  subgraph T53_infrastructure_infrastructure_TelemetryHookBlockEntry["telemetry::TelemetryHookBlockEntry"]
    direction TB
    T53_infrastructure_infrastructure_TelemetryHookBlockEntry__self[TelemetryHookBlockEntry]
  end
  subgraph T45_infrastructure_infrastructure_TelemetryReport["telemetry::TelemetryReport"]
    direction TB
    T45_infrastructure_infrastructure_TelemetryReport__self[TelemetryReport]
    T45_infrastructure_infrastructure_TelemetryReport_new([new])
    T45_infrastructure_infrastructure_TelemetryReport_aggregate([aggregate])
  end
  subgraph T50_infrastructure_infrastructure_TelemetryReportError["telemetry::TelemetryReportError"]
    direction TB
    T50_infrastructure_infrastructure_TelemetryReportError__self[TelemetryReportError]
    T50_infrastructure_infrastructure_TelemetryReportError_Io[Io]
    T50_infrastructure_infrastructure_TelemetryReportError_TrackNotFound[TrackNotFound]
  end
  subgraph T51_infrastructure_infrastructure_TelemetryReportOutput["telemetry::TelemetryReportOutput"]
    direction TB
    T51_infrastructure_infrastructure_TelemetryReportOutput__self[TelemetryReportOutput]
  end
  subgraph T49_infrastructure_infrastructure_TelemetryWriteError["telemetry::TelemetryWriteError"]
    direction TB
    T49_infrastructure_infrastructure_TelemetryWriteError__self[TelemetryWriteError]
    T49_infrastructure_infrastructure_TelemetryWriteError_Serialize[Serialize]
    T49_infrastructure_infrastructure_TelemetryWriteError_Io[Io]
  end
  subgraph T45_infrastructure_infrastructure_TelemetryWriter["telemetry::TelemetryWriter"]
    direction TB
    T45_infrastructure_infrastructure_TelemetryWriter__self[TelemetryWriter]
    T45_infrastructure_infrastructure_TelemetryWriter_new([new])
    T45_infrastructure_infrastructure_TelemetryWriter_write([write])
  end
  end
end
T45_infrastructure_infrastructure_TelemetryConfig_from_env --> T45_infrastructure_infrastructure_TelemetryConfig__self
T45_infrastructure_infrastructure_TelemetryReport_new --> T45_infrastructure_infrastructure_TelemetryReport__self
T45_infrastructure_infrastructure_TelemetryReport_aggregate --> T50_infrastructure_infrastructure_TelemetryReportError__self
T45_infrastructure_infrastructure_TelemetryReport_aggregate --> T51_infrastructure_infrastructure_TelemetryReportOutput__self
T51_infrastructure_infrastructure_TelemetryReportOutput__self --o|phase_durations| T50_infrastructure_infrastructure_PhaseDurationSummary__self
T51_infrastructure_infrastructure_TelemetryReportOutput__self --o|errors| T49_infrastructure_infrastructure_TelemetryErrorEntry__self
T51_infrastructure_infrastructure_TelemetryReportOutput__self --o|hook_blocks| T53_infrastructure_infrastructure_TelemetryHookBlockEntry__self
T45_infrastructure_infrastructure_TelemetryWriter_new --o T45_infrastructure_infrastructure_TelemetryConfig__self
T45_infrastructure_infrastructure_TelemetryWriter_new --> T45_infrastructure_infrastructure_TelemetryWriter__self
T45_infrastructure_infrastructure_TelemetryWriter_write --o T44_infrastructure_infrastructure_TelemetryEvent__self
T45_infrastructure_infrastructure_TelemetryWriter_write --> T49_infrastructure_infrastructure_TelemetryWriteError__self
class T39_infrastructure_infrastructure_RoundType_Final variant_node
class T39_infrastructure_infrastructure_RoundType_Fast variant_node
class T39_infrastructure_infrastructure_RoundType__self dto
class T50_infrastructure_infrastructure_PhaseDurationSummary__self dto
class T45_infrastructure_infrastructure_TelemetryConfig_from_env method_node
class T45_infrastructure_infrastructure_TelemetryConfig_is_enabled method_node
class T45_infrastructure_infrastructure_TelemetryConfig__self dto
class T49_infrastructure_infrastructure_TelemetryErrorEntry__self dto
class T44_infrastructure_infrastructure_TelemetryEvent_TrackSubcommand variant_node
class T44_infrastructure_infrastructure_TelemetryEvent_GateEval variant_node
class T44_infrastructure_infrastructure_TelemetryEvent_ReviewRound variant_node
class T44_infrastructure_infrastructure_TelemetryEvent_ExternalSubprocess variant_node
class T44_infrastructure_infrastructure_TelemetryEvent_HookBlock variant_node
class T44_infrastructure_infrastructure_TelemetryEvent_AdvisoryHookFired variant_node
class T44_infrastructure_infrastructure_TelemetryEvent_NonZeroExit variant_node
class T44_infrastructure_infrastructure_TelemetryEvent__self dto
class T53_infrastructure_infrastructure_TelemetryHookBlockEntry__self dto
class T45_infrastructure_infrastructure_TelemetryReport_new method_node
class T45_infrastructure_infrastructure_TelemetryReport_aggregate method_node
class T45_infrastructure_infrastructure_TelemetryReport__self secondary_adapter
class T50_infrastructure_infrastructure_TelemetryReportError_Io variant_node
class T50_infrastructure_infrastructure_TelemetryReportError_TrackNotFound variant_node
class T50_infrastructure_infrastructure_TelemetryReportError__self error_type
class T51_infrastructure_infrastructure_TelemetryReportOutput__self dto
class T49_infrastructure_infrastructure_TelemetryWriteError_Serialize variant_node
class T49_infrastructure_infrastructure_TelemetryWriteError_Io variant_node
class T49_infrastructure_infrastructure_TelemetryWriteError__self error_type
class T45_infrastructure_infrastructure_TelemetryWriter_new method_node
class T45_infrastructure_infrastructure_TelemetryWriter_write method_node
class T45_infrastructure_infrastructure_TelemetryWriter__self secondary_adapter
```
