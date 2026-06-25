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
  subgraph domain_domain_module_review_v2["domain::review_v2"]
    direction TB
  subgraph T31_domain_domain_ReviewScopeConfig["review_v2::scope_config::ReviewScopeConfig"]
    direction TB
    T31_domain_domain_ReviewScopeConfig__self[ReviewScopeConfig]
    T31_domain_domain_ReviewScopeConfig_new([new])
    T31_domain_domain_ReviewScopeConfig_classify([classify])
    T31_domain_domain_ReviewScopeConfig_get_scope_names([get_scope_names])
    T31_domain_domain_ReviewScopeConfig_contains_scope([contains_scope])
    T31_domain_domain_ReviewScopeConfig_all_scope_names([all_scope_names])
    T31_domain_domain_ReviewScopeConfig_briefing_file_for_scope([briefing_file_for_scope])
    T31_domain_domain_ReviewScopeConfig_diff_ceiling_for_scope([diff_ceiling_for_scope])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config[[load_v2_scope_config]]
  end
end
T31_domain_domain_ReviewScopeConfig_new --> T31_domain_domain_ReviewScopeConfig__self
F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config --> T31_domain_domain_ReviewScopeConfig__self
class T31_domain_domain_ReviewScopeConfig_new method_node
class T31_domain_domain_ReviewScopeConfig_classify method_node
class T31_domain_domain_ReviewScopeConfig_get_scope_names method_node
class T31_domain_domain_ReviewScopeConfig_contains_scope method_node
class T31_domain_domain_ReviewScopeConfig_all_scope_names method_node
class T31_domain_domain_ReviewScopeConfig_briefing_file_for_scope method_node
class T31_domain_domain_ReviewScopeConfig_diff_ceiling_for_scope method_node
class T31_domain_domain_ReviewScopeConfig__self domain_service
class F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config free_function
class F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config function_node
```
