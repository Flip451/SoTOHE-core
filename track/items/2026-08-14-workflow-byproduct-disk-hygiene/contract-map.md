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
  subgraph usecase_usecase_module_batch_plan["usecase::batch_plan"]
    direction TB
  subgraph R36_usecase_usecase_ScopeDiffMeasurePort["batch_plan::ports::ScopeDiffMeasurePort"]
    direction TB
    R36_usecase_usecase_ScopeDiffMeasurePort__self[ScopeDiffMeasurePort]
    R36_usecase_usecase_ScopeDiffMeasurePort_measure_scope_diff([measure_scope_diff])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_scope_diff_measure["infrastructure::scope_diff_measure"]
    direction TB
  subgraph T50_infrastructure_infrastructure_GitScopeDiffMeasurer["scope_diff_measure::GitScopeDiffMeasurer"]
    direction TB
    T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self[GitScopeDiffMeasurer]
    T50_infrastructure_infrastructure_GitScopeDiffMeasurer_new([new])
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
T50_infrastructure_infrastructure_GitScopeDiffMeasurer_new --> T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self
T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self -.impl.-> R36_usecase_usecase_ScopeDiffMeasurePort__self
class R36_usecase_usecase_ScopeDiffMeasurePort_measure_scope_diff method_node
class R36_usecase_usecase_ScopeDiffMeasurePort__self secondary_port
class T50_infrastructure_infrastructure_GitScopeDiffMeasurer_new method_node
class T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self secondary_adapter
```
