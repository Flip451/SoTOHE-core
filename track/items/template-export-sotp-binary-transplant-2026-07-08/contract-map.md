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
  subgraph usecase_usecase_module_template_export["usecase::template_export"]
    direction TB
  subgraph T41_usecase_usecase_SelfBinaryTransplantError["template_export::SelfBinaryTransplantError"]
    direction TB
    T41_usecase_usecase_SelfBinaryTransplantError__self[SelfBinaryTransplantError]
    T41_usecase_usecase_SelfBinaryTransplantError_SourcePathUnavailable[SourcePathUnavailable]
    T41_usecase_usecase_SelfBinaryTransplantError_DestinationWriteFailure[DestinationWriteFailure]
    T41_usecase_usecase_SelfBinaryTransplantError_PermissionSetFailure[PermissionSetFailure]
  end
  subgraph T37_usecase_usecase_TemplateExportCommand["template_export::TemplateExportCommand"]
    direction TB
    T37_usecase_usecase_TemplateExportCommand__self[TemplateExportCommand]
  end
  subgraph T35_usecase_usecase_TemplateExportError["template_export::TemplateExportError"]
    direction TB
    T35_usecase_usecase_TemplateExportError__self[TemplateExportError]
    T35_usecase_usecase_TemplateExportError_ManifestRead[ManifestRead]
    T35_usecase_usecase_TemplateExportError_Export[Export]
    T35_usecase_usecase_TemplateExportError_BinaryTransplant[BinaryTransplant]
  end
  subgraph T40_usecase_usecase_TemplateExportInteractor["template_export::TemplateExportInteractor"]
    direction TB
    T40_usecase_usecase_TemplateExportInteractor__self[TemplateExportInteractor]
    T40_usecase_usecase_TemplateExportInteractor_new([new])
  end
  subgraph T39_usecase_usecase_TemplateExportPortError["template_export::TemplateExportPortError"]
    direction TB
    T39_usecase_usecase_TemplateExportPortError__self[TemplateExportPortError]
    T39_usecase_usecase_TemplateExportPortError_OutputDirExists[OutputDirExists]
    T39_usecase_usecase_TemplateExportPortError_OverlayMissing[OverlayMissing]
    T39_usecase_usecase_TemplateExportPortError_SourceMissing[SourceMissing]
    T39_usecase_usecase_TemplateExportPortError_UnclassifiedPath[UnclassifiedPath]
    T39_usecase_usecase_TemplateExportPortError_Io[Io]
  end
  subgraph T36_usecase_usecase_TemplateExportReport["template_export::TemplateExportReport"]
    direction TB
    T36_usecase_usecase_TemplateExportReport__self[TemplateExportReport]
  end
  subgraph R40_usecase_usecase_SelfBinaryTransplantPort["template_export::SelfBinaryTransplantPort"]
    direction TB
    R40_usecase_usecase_SelfBinaryTransplantPort__self[SelfBinaryTransplantPort]
    R40_usecase_usecase_SelfBinaryTransplantPort_transplant([transplant])
  end
  subgraph R34_usecase_usecase_TemplateExportPort["template_export::TemplateExportPort"]
    direction TB
    R34_usecase_usecase_TemplateExportPort__self[TemplateExportPort]
    R34_usecase_usecase_TemplateExportPort_export([export])
  end
  subgraph R37_usecase_usecase_TemplateExportService["template_export::TemplateExportService"]
    direction TB
    R37_usecase_usecase_TemplateExportService__self[TemplateExportService]
    R37_usecase_usecase_TemplateExportService_export([export])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_template_export["infrastructure::template_export"]
    direction TB
  subgraph T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter["template_export::FsSelfBinaryTransplantAdapter"]
    direction TB
    T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter__self[FsSelfBinaryTransplantAdapter]
    T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter_new([new])
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
T35_usecase_usecase_TemplateExportError_Export --o|source| T39_usecase_usecase_TemplateExportPortError__self
T35_usecase_usecase_TemplateExportError_BinaryTransplant --o|source| T41_usecase_usecase_SelfBinaryTransplantError__self
T40_usecase_usecase_TemplateExportInteractor_new --> T40_usecase_usecase_TemplateExportInteractor__self
R40_usecase_usecase_SelfBinaryTransplantPort_transplant --> T41_usecase_usecase_SelfBinaryTransplantError__self
R34_usecase_usecase_TemplateExportPort_export --o T37_usecase_usecase_TemplateExportCommand__self
R34_usecase_usecase_TemplateExportPort_export --> T39_usecase_usecase_TemplateExportPortError__self
R34_usecase_usecase_TemplateExportPort_export --> T36_usecase_usecase_TemplateExportReport__self
R37_usecase_usecase_TemplateExportService_export --o T37_usecase_usecase_TemplateExportCommand__self
R37_usecase_usecase_TemplateExportService_export --> T35_usecase_usecase_TemplateExportError__self
R37_usecase_usecase_TemplateExportService_export --> T36_usecase_usecase_TemplateExportReport__self
T40_usecase_usecase_TemplateExportInteractor__self -.impl.-> R37_usecase_usecase_TemplateExportService__self
T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter_new --> T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter__self
T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter__self -.impl.-> R40_usecase_usecase_SelfBinaryTransplantPort__self
class T41_usecase_usecase_SelfBinaryTransplantError_SourcePathUnavailable variant_node
class T41_usecase_usecase_SelfBinaryTransplantError_DestinationWriteFailure variant_node
class T41_usecase_usecase_SelfBinaryTransplantError_PermissionSetFailure variant_node
class T41_usecase_usecase_SelfBinaryTransplantError__self error_type
class T37_usecase_usecase_TemplateExportCommand__self command
class T35_usecase_usecase_TemplateExportError_ManifestRead variant_node
class T35_usecase_usecase_TemplateExportError_Export variant_node
class T35_usecase_usecase_TemplateExportError_BinaryTransplant variant_node
class T35_usecase_usecase_TemplateExportError__self error_type
class T40_usecase_usecase_TemplateExportInteractor_new method_node
class T40_usecase_usecase_TemplateExportInteractor__self interactor
class T39_usecase_usecase_TemplateExportPortError_OutputDirExists variant_node
class T39_usecase_usecase_TemplateExportPortError_OverlayMissing variant_node
class T39_usecase_usecase_TemplateExportPortError_SourceMissing variant_node
class T39_usecase_usecase_TemplateExportPortError_UnclassifiedPath variant_node
class T39_usecase_usecase_TemplateExportPortError_Io variant_node
class T39_usecase_usecase_TemplateExportPortError__self error_type
class T36_usecase_usecase_TemplateExportReport__self dto
class R40_usecase_usecase_SelfBinaryTransplantPort_transplant method_node
class R40_usecase_usecase_SelfBinaryTransplantPort__self secondary_port
class R34_usecase_usecase_TemplateExportPort_export method_node
class R34_usecase_usecase_TemplateExportPort__self secondary_port
class R37_usecase_usecase_TemplateExportService_export method_node
class R37_usecase_usecase_TemplateExportService__self app_service
class T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter_new method_node
class T59_infrastructure_infrastructure_FsSelfBinaryTransplantAdapter__self secondary_adapter
```
