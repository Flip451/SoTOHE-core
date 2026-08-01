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
  subgraph T35_domain_domain_ObligationDeriveError["tddd::test_obligation::errors::ObligationDeriveError"]
    direction TB
    T35_domain_domain_ObligationDeriveError__self[ObligationDeriveError]
    T35_domain_domain_ObligationDeriveError_RulesLoad[RulesLoad]
    T35_domain_domain_ObligationDeriveError_TrackNotActive[TrackNotActive]
    T35_domain_domain_ObligationDeriveError_TrackFrozen[TrackFrozen]
    T35_domain_domain_ObligationDeriveError_TrackStatusRead[TrackStatusRead]
    T35_domain_domain_ObligationDeriveError_CatalogueLoad[CatalogueLoad]
    T35_domain_domain_ObligationDeriveError_SpecLoad[SpecLoad]
    T35_domain_domain_ObligationDeriveError_InvalidCatalogueState[InvalidCatalogueState]
    T35_domain_domain_ObligationDeriveError_ArtifactWrite[ArtifactWrite]
  end
  subgraph T40_domain_domain_TrackStatusReadFailureKind["tddd::test_obligation::errors::TrackStatusReadFailureKind"]
    direction TB
    T40_domain_domain_TrackStatusReadFailureKind__self[TrackStatusReadFailureKind]
    T40_domain_domain_TrackStatusReadFailureKind_Unavailable[Unavailable]
  end
  end
  subgraph domain_domain_module_track["domain::track"]
    direction TB
  subgraph T31_domain_domain_FrozenTrackStatus["track::FrozenTrackStatus"]
    direction TB
    T31_domain_domain_FrozenTrackStatus__self[FrozenTrackStatus]
    T31_domain_domain_FrozenTrackStatus_Done[Done]
    T31_domain_domain_FrozenTrackStatus_Archived[Archived]
  end
  subgraph T25_domain_domain_TrackStatus["track::TrackStatus"]
    direction TB
    T25_domain_domain_TrackStatus__self[TrackStatus]
    T25_domain_domain_TrackStatus_Planned[Planned]
    T25_domain_domain_TrackStatus_InProgress[InProgress]
    T25_domain_domain_TrackStatus_Done[Done]
    T25_domain_domain_TrackStatus_Blocked[Blocked]
    T25_domain_domain_TrackStatus_Cancelled[Cancelled]
    T25_domain_domain_TrackStatus_Archived[Archived]
    T25_domain_domain_TrackStatus_is_active([is_active])
    T25_domain_domain_TrackStatus_frozen_status([frozen_status])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_test_obligation["usecase::test_obligation"]
    direction TB
  subgraph T47_usecase_usecase_DeriveTestObligationsInteractor["test_obligation::derive::DeriveTestObligationsInteractor"]
    direction TB
    T47_usecase_usecase_DeriveTestObligationsInteractor__self[DeriveTestObligationsInteractor]
    T47_usecase_usecase_DeriveTestObligationsInteractor_new([new])
  end
  end
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
T35_domain_domain_ObligationDeriveError_TrackFrozen --o|status| T31_domain_domain_FrozenTrackStatus__self
T35_domain_domain_ObligationDeriveError_TrackStatusRead --o T40_domain_domain_TrackStatusReadFailureKind__self
T25_domain_domain_TrackStatus_frozen_status --> T31_domain_domain_FrozenTrackStatus__self
T47_usecase_usecase_DeriveTestObligationsInteractor_new --> T47_usecase_usecase_DeriveTestObligationsInteractor__self
class T35_domain_domain_ObligationDeriveError_RulesLoad variant_node
class T35_domain_domain_ObligationDeriveError_TrackNotActive variant_node
class T35_domain_domain_ObligationDeriveError_TrackFrozen variant_node
class T35_domain_domain_ObligationDeriveError_TrackStatusRead variant_node
class T35_domain_domain_ObligationDeriveError_CatalogueLoad variant_node
class T35_domain_domain_ObligationDeriveError_SpecLoad variant_node
class T35_domain_domain_ObligationDeriveError_InvalidCatalogueState variant_node
class T35_domain_domain_ObligationDeriveError_ArtifactWrite variant_node
class T35_domain_domain_ObligationDeriveError__self error_type
class T40_domain_domain_TrackStatusReadFailureKind_Unavailable variant_node
class T40_domain_domain_TrackStatusReadFailureKind__self value_object
class T31_domain_domain_FrozenTrackStatus_Done variant_node
class T31_domain_domain_FrozenTrackStatus_Archived variant_node
class T31_domain_domain_FrozenTrackStatus__self value_object
class T25_domain_domain_TrackStatus_Planned variant_node
class T25_domain_domain_TrackStatus_InProgress variant_node
class T25_domain_domain_TrackStatus_Done variant_node
class T25_domain_domain_TrackStatus_Blocked variant_node
class T25_domain_domain_TrackStatus_Cancelled variant_node
class T25_domain_domain_TrackStatus_Archived variant_node
class T25_domain_domain_TrackStatus_is_active method_node
class T25_domain_domain_TrackStatus_frozen_status method_node
class T25_domain_domain_TrackStatus__self value_object
class T47_usecase_usecase_DeriveTestObligationsInteractor_new method_node
class T47_usecase_usecase_DeriveTestObligationsInteractor__self interactor
```
