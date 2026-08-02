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
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T34_domain_domain_CatalogueLinterError["tddd::catalogue_linter::CatalogueLinterError"]
    direction TB
    T34_domain_domain_CatalogueLinterError__self[CatalogueLinterError]
    T34_domain_domain_CatalogueLinterError_DuplicateTypeAliasGenericParameter[DuplicateTypeAliasGenericParameter]
    T34_domain_domain_CatalogueLinterError_InvalidRuleConfig[InvalidRuleConfig]
    T34_domain_domain_CatalogueLinterError_UnknownLayer[UnknownLayer]
    T34_domain_domain_CatalogueLinterError_ScanFailed[ScanFailed]
  end
  subgraph T24_domain_domain_TypeKindV2["tddd::catalogue_v2::composite::TypeKindV2"]
    direction TB
    T24_domain_domain_TypeKindV2__self[TypeKindV2]
    T24_domain_domain_TypeKindV2_Struct[Struct]
    T24_domain_domain_TypeKindV2_Enum[Enum]
    T24_domain_domain_TypeKindV2_TypeAlias[TypeAlias]
  end
  F69_domain_domain_domain__tddd__catalogue_linter__evaluate_catalogue_lint[[evaluate_catalogue_lint]]
  end
end
subgraph usecase["usecase"]
  direction TB
end
subgraph infrastructure["infrastructure"]
  direction TB
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
F69_domain_domain_domain__tddd__catalogue_linter__evaluate_catalogue_lint --> T34_domain_domain_CatalogueLinterError__self
class T34_domain_domain_CatalogueLinterError_DuplicateTypeAliasGenericParameter variant_node
class T34_domain_domain_CatalogueLinterError_InvalidRuleConfig variant_node
class T34_domain_domain_CatalogueLinterError_UnknownLayer variant_node
class T34_domain_domain_CatalogueLinterError_ScanFailed variant_node
class T34_domain_domain_CatalogueLinterError__self error_type
class T24_domain_domain_TypeKindV2_Struct variant_node
class T24_domain_domain_TypeKindV2_Enum variant_node
class T24_domain_domain_TypeKindV2_TypeAlias variant_node
class T24_domain_domain_TypeKindV2__self value_object
class F69_domain_domain_domain__tddd__catalogue_linter__evaluate_catalogue_lint free_function
class F69_domain_domain_domain__tddd__catalogue_linter__evaluate_catalogue_lint function_node
```
