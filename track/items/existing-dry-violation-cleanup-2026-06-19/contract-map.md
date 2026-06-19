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
  subgraph domain_domain_module_ids["domain::ids"]
    direction TB
  subgraph T28_domain_domain_NonEmptyString["ids::NonEmptyString"]
    direction TB
    T28_domain_domain_NonEmptyString__self[NonEmptyString]
  end
  subgraph T21_domain_domain_TrackId["ids::TrackId"]
    direction TB
    T21_domain_domain_TrackId__self[TrackId]
  end
  end
end
subgraph usecase["usecase"]
  direction TB
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_dry_check["infrastructure::dry_check"]
    direction TB
  subgraph T45_infrastructure_infrastructure_CodexDryChecker["dry_check::codex_dry_checker::CodexDryChecker"]
    direction TB
    T45_infrastructure_infrastructure_CodexDryChecker__self[CodexDryChecker]
  end
  F75_infrastructure_infrastructure_infrastructure__dry_check__corpus__sha256_hex[[sha256_hex]]
  end
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  subgraph T43_infrastructure_infrastructure_CodexReviewer["review_v2::codex_reviewer::CodexReviewer"]
    direction TB
    T43_infrastructure_infrastructure_CodexReviewer__self[CodexReviewer]
  end
  end
end
class T28_domain_domain_NonEmptyString__self value_object
class T21_domain_domain_TrackId__self value_object
class T45_infrastructure_infrastructure_CodexDryChecker__self secondary_adapter
class F75_infrastructure_infrastructure_infrastructure__dry_check__corpus__sha256_hex free_function
class F75_infrastructure_infrastructure_infrastructure__dry_check__corpus__sha256_hex function_node
class T43_infrastructure_infrastructure_CodexReviewer__self secondary_adapter
```
