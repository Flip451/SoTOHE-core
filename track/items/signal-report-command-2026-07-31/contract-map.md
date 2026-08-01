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
  subgraph domain_domain_module_ids["domain::ids"]
    direction TB
  subgraph T28_domain_domain_NonEmptyString["ids::NonEmptyString"]
    direction TB
    T28_domain_domain_NonEmptyString__self[NonEmptyString]
    T28_domain_domain_NonEmptyString_try_new([try_new])
  end
  end
  subgraph domain_domain_module_review_v2["domain::review_v2"]
    direction TB
  subgraph T22_domain_domain_FilePath["review_v2::types::FilePath"]
    direction TB
    T22_domain_domain_FilePath__self[FilePath]
    T22_domain_domain_FilePath_new([new])
    T22_domain_domain_FilePath_as_str([as_str])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_signal_report["usecase::signal_report"]
    direction TB
  subgraph T33_usecase_usecase_SignalReportChain["signal_report::SignalReportChain"]
    direction TB
    T33_usecase_usecase_SignalReportChain__self[SignalReportChain]
    T33_usecase_usecase_SignalReportChain_AdrUser[AdrUser]
    T33_usecase_usecase_SignalReportChain_SpecAdr[SpecAdr]
    T33_usecase_usecase_SignalReportChain_CatalogSpec[CatalogSpec]
    T33_usecase_usecase_SignalReportChain_ImplCatalog[ImplCatalog]
  end
  subgraph T42_usecase_usecase_SignalReportChainSelection["signal_report::SignalReportChainSelection"]
    direction TB
    T42_usecase_usecase_SignalReportChainSelection__self[SignalReportChainSelection]
    T42_usecase_usecase_SignalReportChainSelection_All[All]
    T42_usecase_usecase_SignalReportChainSelection_One[One]
  end
  subgraph T35_usecase_usecase_SignalReportEntryId["signal_report::SignalReportEntryId"]
    direction TB
    T35_usecase_usecase_SignalReportEntryId__self[SignalReportEntryId]
    T35_usecase_usecase_SignalReportEntryId_new([new])
  end
  subgraph T33_usecase_usecase_SignalReportError["signal_report::SignalReportError"]
    direction TB
    T33_usecase_usecase_SignalReportError__self[SignalReportError]
    T33_usecase_usecase_SignalReportError_SourceUnavailable[SourceUnavailable]
    T33_usecase_usecase_SignalReportError_InvalidOccurrence[InvalidOccurrence]
  end
  subgraph T38_usecase_usecase_SignalReportInteractor["signal_report::SignalReportInteractor"]
    direction TB
    T38_usecase_usecase_SignalReportInteractor__self[SignalReportInteractor]
    T38_usecase_usecase_SignalReportInteractor_new([new])
  end
  subgraph T33_usecase_usecase_SignalReportLevel["signal_report::SignalReportLevel"]
    direction TB
    T33_usecase_usecase_SignalReportLevel__self[SignalReportLevel]
    T33_usecase_usecase_SignalReportLevel_Yellow[Yellow]
    T33_usecase_usecase_SignalReportLevel_Red[Red]
  end
  subgraph T42_usecase_usecase_SignalReportLevelSelection["signal_report::SignalReportLevelSelection"]
    direction TB
    T42_usecase_usecase_SignalReportLevelSelection__self[SignalReportLevelSelection]
    T42_usecase_usecase_SignalReportLevelSelection_YellowOnly[YellowOnly]
    T42_usecase_usecase_SignalReportLevelSelection_RedOnly[RedOnly]
    T42_usecase_usecase_SignalReportLevelSelection_YellowAndRed[YellowAndRed]
  end
  subgraph T36_usecase_usecase_SignalReportLocation["signal_report::SignalReportLocation"]
    direction TB
    T36_usecase_usecase_SignalReportLocation__self[SignalReportLocation]
    T36_usecase_usecase_SignalReportLocation_new([new])
  end
  subgraph T38_usecase_usecase_SignalReportOccurrence["signal_report::SignalReportOccurrence"]
    direction TB
    T38_usecase_usecase_SignalReportOccurrence__self[SignalReportOccurrence]
  end
  subgraph T34_usecase_usecase_SignalReportOutput["signal_report::SignalReportOutput"]
    direction TB
    T34_usecase_usecase_SignalReportOutput__self[SignalReportOutput]
  end
  subgraph T33_usecase_usecase_SignalReportQuery["signal_report::SignalReportQuery"]
    direction TB
    T33_usecase_usecase_SignalReportQuery__self[SignalReportQuery]
  end
  subgraph T34_usecase_usecase_SignalReportReason["signal_report::SignalReportReason"]
    direction TB
    T34_usecase_usecase_SignalReportReason__self[SignalReportReason]
    T34_usecase_usecase_SignalReportReason_new([new])
  end
  subgraph T37_usecase_usecase_SignalReportReference["signal_report::SignalReportReference"]
    direction TB
    T37_usecase_usecase_SignalReportReference__self[SignalReportReference]
    T37_usecase_usecase_SignalReportReference_new([new])
  end
  subgraph R35_usecase_usecase_SignalReportService["signal_report::SignalReportService"]
    direction TB
    R35_usecase_usecase_SignalReportService__self[SignalReportService]
    R35_usecase_usecase_SignalReportService_report([report])
  end
  subgraph R38_usecase_usecase_SignalReportSourcePort["signal_report::SignalReportSourcePort"]
    direction TB
    R38_usecase_usecase_SignalReportSourcePort__self[SignalReportSourcePort]
    R38_usecase_usecase_SignalReportSourcePort_load([load])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_signal_report["infrastructure::signal_report"]
    direction TB
  subgraph T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter["signal_report::SystemSignalReportSourceAdapter"]
    direction TB
    T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter__self[SystemSignalReportSourceAdapter]
    T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter_new([new])
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_signal["cli_driver::signal"]
    direction TB
  subgraph T34_cli_driver_cli_driver_SignalDriver["signal::SignalDriver"]
    direction TB
    T34_cli_driver_cli_driver_SignalDriver__self[SignalDriver]
    T34_cli_driver_cli_driver_SignalDriver_new([new])
    T34_cli_driver_cli_driver_SignalDriver_handle([handle])
  end
  end
  subgraph cli_driver_cli_driver_module_signal_report["cli_driver::signal_report"]
    direction TB
  subgraph T45_cli_driver_cli_driver_SignalReportChainFilter["signal_report::SignalReportChainFilter"]
    direction TB
    T45_cli_driver_cli_driver_SignalReportChainFilter__self[SignalReportChainFilter]
    T45_cli_driver_cli_driver_SignalReportChainFilter_All[All]
    T45_cli_driver_cli_driver_SignalReportChainFilter_AdrUser[AdrUser]
    T45_cli_driver_cli_driver_SignalReportChainFilter_SpecAdr[SpecAdr]
    T45_cli_driver_cli_driver_SignalReportChainFilter_CatalogSpec[CatalogSpec]
    T45_cli_driver_cli_driver_SignalReportChainFilter_ImplCatalog[ImplCatalog]
  end
  subgraph T40_cli_driver_cli_driver_SignalReportDriver["signal_report::SignalReportDriver"]
    direction TB
    T40_cli_driver_cli_driver_SignalReportDriver__self[SignalReportDriver]
    T40_cli_driver_cli_driver_SignalReportDriver_new([new])
    T40_cli_driver_cli_driver_SignalReportDriver_handle([handle])
  end
  subgraph T39_cli_driver_cli_driver_SignalReportInput["signal_report::SignalReportInput"]
    direction TB
    T39_cli_driver_cli_driver_SignalReportInput__self[SignalReportInput]
  end
  subgraph T45_cli_driver_cli_driver_SignalReportLevelFilter["signal_report::SignalReportLevelFilter"]
    direction TB
    T45_cli_driver_cli_driver_SignalReportLevelFilter__self[SignalReportLevelFilter]
    T45_cli_driver_cli_driver_SignalReportLevelFilter_YellowOnly[YellowOnly]
    T45_cli_driver_cli_driver_SignalReportLevelFilter_RedOnly[RedOnly]
    T45_cli_driver_cli_driver_SignalReportLevelFilter_YellowAndRed[YellowAndRed]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_signal["cli_composition::signal"]
    direction TB
  subgraph T53_cli_composition_cli_composition_SignalCompositionRoot["signal::SignalCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_SignalCompositionRoot__self[SignalCompositionRoot]
    T53_cli_composition_cli_composition_SignalCompositionRoot_new([new])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver([signal_driver])
    T53_cli_composition_cli_composition_SignalCompositionRoot_signal_report_driver([signal_report_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T21_cli_cli_SignalCommand["commands::signal::SignalCommand"]
    direction TB
    T21_cli_cli_SignalCommand__self[SignalCommand]
    T21_cli_cli_SignalCommand_CalcAdrUser[CalcAdrUser]
    T21_cli_cli_SignalCommand_CheckAdrUser[CheckAdrUser]
    T21_cli_cli_SignalCommand_CalcSpecAdr[CalcSpecAdr]
    T21_cli_cli_SignalCommand_CheckSpecAdr[CheckSpecAdr]
    T21_cli_cli_SignalCommand_CalcCatalogSpec[CalcCatalogSpec]
    T21_cli_cli_SignalCommand_CheckCatalogSpec[CheckCatalogSpec]
    T21_cli_cli_SignalCommand_CalcImplCatalog[CalcImplCatalog]
    T21_cli_cli_SignalCommand_CheckImplCatalog[CheckImplCatalog]
    T21_cli_cli_SignalCommand_Check[Check]
    T21_cli_cli_SignalCommand_Report[Report]
  end
  subgraph T24_cli_cli_SignalReportArgs["commands::signal::SignalReportArgs"]
    direction TB
    T24_cli_cli_SignalReportArgs__self[SignalReportArgs]
  end
  subgraph T28_cli_cli_SignalReportChainArg["commands::signal::SignalReportChainArg"]
    direction TB
    T28_cli_cli_SignalReportChainArg__self[SignalReportChainArg]
    T28_cli_cli_SignalReportChainArg_All[All]
    T28_cli_cli_SignalReportChainArg_AdrUser[AdrUser]
    T28_cli_cli_SignalReportChainArg_SpecAdr[SpecAdr]
    T28_cli_cli_SignalReportChainArg_CatalogSpec[CatalogSpec]
    T28_cli_cli_SignalReportChainArg_ImplCatalog[ImplCatalog]
  end
  subgraph T27_cli_cli_SignalReportOnlyArg["commands::signal::SignalReportOnlyArg"]
    direction TB
    T27_cli_cli_SignalReportOnlyArg__self[SignalReportOnlyArg]
    T27_cli_cli_SignalReportOnlyArg_YellowOnly[YellowOnly]
    T27_cli_cli_SignalReportOnlyArg_RedOnly[RedOnly]
    T27_cli_cli_SignalReportOnlyArg_YellowAndRed[YellowAndRed]
  end
  F38_cli_cli_cli__commands__signal__execute[[execute]]
  end
end
T28_domain_domain_NonEmptyString_try_new --> T28_domain_domain_NonEmptyString__self
T22_domain_domain_FilePath_new --> T22_domain_domain_FilePath__self
T42_usecase_usecase_SignalReportChainSelection_One --o T33_usecase_usecase_SignalReportChain__self
T35_usecase_usecase_SignalReportEntryId_new --o T28_domain_domain_NonEmptyString__self
T35_usecase_usecase_SignalReportEntryId_new --> T35_usecase_usecase_SignalReportEntryId__self
T33_usecase_usecase_SignalReportError_SourceUnavailable --o T33_usecase_usecase_SignalReportChain__self
T33_usecase_usecase_SignalReportError_InvalidOccurrence --o T33_usecase_usecase_SignalReportChain__self
T38_usecase_usecase_SignalReportInteractor_new --o R38_usecase_usecase_SignalReportSourcePort__self
T38_usecase_usecase_SignalReportInteractor_new --> T38_usecase_usecase_SignalReportInteractor__self
T36_usecase_usecase_SignalReportLocation_new --o T22_domain_domain_FilePath__self
T36_usecase_usecase_SignalReportLocation_new --> T36_usecase_usecase_SignalReportLocation__self
T38_usecase_usecase_SignalReportOccurrence__self --o|chain| T33_usecase_usecase_SignalReportChain__self
T38_usecase_usecase_SignalReportOccurrence__self --o|level| T33_usecase_usecase_SignalReportLevel__self
T38_usecase_usecase_SignalReportOccurrence__self --o|entry_id| T35_usecase_usecase_SignalReportEntryId__self
T38_usecase_usecase_SignalReportOccurrence__self --o|reference| T37_usecase_usecase_SignalReportReference__self
T38_usecase_usecase_SignalReportOccurrence__self --o|reason| T34_usecase_usecase_SignalReportReason__self
T38_usecase_usecase_SignalReportOccurrence__self --o|location| T36_usecase_usecase_SignalReportLocation__self
T34_usecase_usecase_SignalReportOutput__self --o|occurrences| T38_usecase_usecase_SignalReportOccurrence__self
T33_usecase_usecase_SignalReportQuery__self --o|chain| T42_usecase_usecase_SignalReportChainSelection__self
T33_usecase_usecase_SignalReportQuery__self --o|levels| T42_usecase_usecase_SignalReportLevelSelection__self
T34_usecase_usecase_SignalReportReason_new --o T28_domain_domain_NonEmptyString__self
T34_usecase_usecase_SignalReportReason_new --> T34_usecase_usecase_SignalReportReason__self
T37_usecase_usecase_SignalReportReference_new --o T28_domain_domain_NonEmptyString__self
T37_usecase_usecase_SignalReportReference_new --> T37_usecase_usecase_SignalReportReference__self
R35_usecase_usecase_SignalReportService_report --o T33_usecase_usecase_SignalReportQuery__self
R35_usecase_usecase_SignalReportService_report --> T33_usecase_usecase_SignalReportError__self
R35_usecase_usecase_SignalReportService_report --> T34_usecase_usecase_SignalReportOutput__self
R38_usecase_usecase_SignalReportSourcePort_load --o T33_usecase_usecase_SignalReportChain__self
R38_usecase_usecase_SignalReportSourcePort_load --> T33_usecase_usecase_SignalReportError__self
R38_usecase_usecase_SignalReportSourcePort_load --> T38_usecase_usecase_SignalReportOccurrence__self
T38_usecase_usecase_SignalReportInteractor__self -.impl.-> R35_usecase_usecase_SignalReportService__self
T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter_new --> T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter__self
T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter__self -.impl.-> R38_usecase_usecase_SignalReportSourcePort__self
T34_cli_driver_cli_driver_SignalDriver_new --> T34_cli_driver_cli_driver_SignalDriver__self
T40_cli_driver_cli_driver_SignalReportDriver_new --o R35_usecase_usecase_SignalReportService__self
T40_cli_driver_cli_driver_SignalReportDriver_new --> T40_cli_driver_cli_driver_SignalReportDriver__self
T40_cli_driver_cli_driver_SignalReportDriver_handle --o T39_cli_driver_cli_driver_SignalReportInput__self
T39_cli_driver_cli_driver_SignalReportInput__self --o|chain| T45_cli_driver_cli_driver_SignalReportChainFilter__self
T39_cli_driver_cli_driver_SignalReportInput__self --o|levels| T45_cli_driver_cli_driver_SignalReportLevelFilter__self
T53_cli_composition_cli_composition_SignalCompositionRoot_new --> T53_cli_composition_cli_composition_SignalCompositionRoot__self
T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver --> T34_cli_driver_cli_driver_SignalDriver__self
T53_cli_composition_cli_composition_SignalCompositionRoot_signal_report_driver --> T40_cli_driver_cli_driver_SignalReportDriver__self
T21_cli_cli_SignalCommand_Report --o T24_cli_cli_SignalReportArgs__self
T24_cli_cli_SignalReportArgs__self --o|chain| T28_cli_cli_SignalReportChainArg__self
T24_cli_cli_SignalReportArgs__self --o|levels| T27_cli_cli_SignalReportOnlyArg__self
F38_cli_cli_cli__commands__signal__execute --o T21_cli_cli_SignalCommand__self
class T28_domain_domain_NonEmptyString_try_new method_node
class T28_domain_domain_NonEmptyString__self value_object
class T22_domain_domain_FilePath_new method_node
class T22_domain_domain_FilePath_as_str method_node
class T22_domain_domain_FilePath__self value_object
class T33_usecase_usecase_SignalReportChain_AdrUser variant_node
class T33_usecase_usecase_SignalReportChain_SpecAdr variant_node
class T33_usecase_usecase_SignalReportChain_CatalogSpec variant_node
class T33_usecase_usecase_SignalReportChain_ImplCatalog variant_node
class T33_usecase_usecase_SignalReportChain__self value_object
class T42_usecase_usecase_SignalReportChainSelection_All variant_node
class T42_usecase_usecase_SignalReportChainSelection_One variant_node
class T42_usecase_usecase_SignalReportChainSelection__self value_object
class T35_usecase_usecase_SignalReportEntryId_new method_node
class T35_usecase_usecase_SignalReportEntryId__self value_object
class T33_usecase_usecase_SignalReportError_SourceUnavailable variant_node
class T33_usecase_usecase_SignalReportError_InvalidOccurrence variant_node
class T33_usecase_usecase_SignalReportError__self error_type
class T38_usecase_usecase_SignalReportInteractor_new method_node
class T38_usecase_usecase_SignalReportInteractor__self interactor
class T33_usecase_usecase_SignalReportLevel_Yellow variant_node
class T33_usecase_usecase_SignalReportLevel_Red variant_node
class T33_usecase_usecase_SignalReportLevel__self value_object
class T42_usecase_usecase_SignalReportLevelSelection_YellowOnly variant_node
class T42_usecase_usecase_SignalReportLevelSelection_RedOnly variant_node
class T42_usecase_usecase_SignalReportLevelSelection_YellowAndRed variant_node
class T42_usecase_usecase_SignalReportLevelSelection__self value_object
class T36_usecase_usecase_SignalReportLocation_new method_node
class T36_usecase_usecase_SignalReportLocation__self value_object
class T38_usecase_usecase_SignalReportOccurrence__self dto
class T34_usecase_usecase_SignalReportOutput__self dto
class T33_usecase_usecase_SignalReportQuery__self query
class T34_usecase_usecase_SignalReportReason_new method_node
class T34_usecase_usecase_SignalReportReason__self value_object
class T37_usecase_usecase_SignalReportReference_new method_node
class T37_usecase_usecase_SignalReportReference__self value_object
class R35_usecase_usecase_SignalReportService_report method_node
class R35_usecase_usecase_SignalReportService__self app_service
class R38_usecase_usecase_SignalReportSourcePort_load method_node
class R38_usecase_usecase_SignalReportSourcePort__self secondary_port
class T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter_new method_node
class T61_infrastructure_infrastructure_SystemSignalReportSourceAdapter__self secondary_adapter
class T34_cli_driver_cli_driver_SignalDriver_new method_node
class T34_cli_driver_cli_driver_SignalDriver_handle method_node
class T34_cli_driver_cli_driver_SignalDriver__self primary_adapter
class T45_cli_driver_cli_driver_SignalReportChainFilter_All variant_node
class T45_cli_driver_cli_driver_SignalReportChainFilter_AdrUser variant_node
class T45_cli_driver_cli_driver_SignalReportChainFilter_SpecAdr variant_node
class T45_cli_driver_cli_driver_SignalReportChainFilter_CatalogSpec variant_node
class T45_cli_driver_cli_driver_SignalReportChainFilter_ImplCatalog variant_node
class T45_cli_driver_cli_driver_SignalReportChainFilter__self dto
class T40_cli_driver_cli_driver_SignalReportDriver_new method_node
class T40_cli_driver_cli_driver_SignalReportDriver_handle method_node
class T40_cli_driver_cli_driver_SignalReportDriver__self primary_adapter
class T39_cli_driver_cli_driver_SignalReportInput__self dto
class T45_cli_driver_cli_driver_SignalReportLevelFilter_YellowOnly variant_node
class T45_cli_driver_cli_driver_SignalReportLevelFilter_RedOnly variant_node
class T45_cli_driver_cli_driver_SignalReportLevelFilter_YellowAndRed variant_node
class T45_cli_driver_cli_driver_SignalReportLevelFilter__self dto
class T53_cli_composition_cli_composition_SignalCompositionRoot_new method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_driver method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot_signal_report_driver method_node
class T53_cli_composition_cli_composition_SignalCompositionRoot__self composition_root
class T21_cli_cli_SignalCommand_CalcAdrUser variant_node
class T21_cli_cli_SignalCommand_CheckAdrUser variant_node
class T21_cli_cli_SignalCommand_CalcSpecAdr variant_node
class T21_cli_cli_SignalCommand_CheckSpecAdr variant_node
class T21_cli_cli_SignalCommand_CalcCatalogSpec variant_node
class T21_cli_cli_SignalCommand_CheckCatalogSpec variant_node
class T21_cli_cli_SignalCommand_CalcImplCatalog variant_node
class T21_cli_cli_SignalCommand_CheckImplCatalog variant_node
class T21_cli_cli_SignalCommand_Check variant_node
class T21_cli_cli_SignalCommand_Report variant_node
class T21_cli_cli_SignalCommand__self dto
class T24_cli_cli_SignalReportArgs__self dto
class T28_cli_cli_SignalReportChainArg_All variant_node
class T28_cli_cli_SignalReportChainArg_AdrUser variant_node
class T28_cli_cli_SignalReportChainArg_SpecAdr variant_node
class T28_cli_cli_SignalReportChainArg_CatalogSpec variant_node
class T28_cli_cli_SignalReportChainArg_ImplCatalog variant_node
class T28_cli_cli_SignalReportChainArg__self dto
class T27_cli_cli_SignalReportOnlyArg_YellowOnly variant_node
class T27_cli_cli_SignalReportOnlyArg_RedOnly variant_node
class T27_cli_cli_SignalReportOnlyArg_YellowAndRed variant_node
class T27_cli_cli_SignalReportOnlyArg__self dto
class F38_cli_cli_cli__commands__signal__execute free_function
class F38_cli_cli_cli__commands__signal__execute function_node
```
