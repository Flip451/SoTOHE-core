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
  subgraph usecase_usecase_module_ref_verify["usecase::ref_verify"]
    direction TB
  subgraph T30_usecase_usecase_RefVerifyScope["ref_verify::RefVerifyScope"]
    direction TB
    T30_usecase_usecase_RefVerifyScope__self[RefVerifyScope]
    T30_usecase_usecase_RefVerifyScope_Chain1[Chain1]
    T30_usecase_usecase_RefVerifyScope_Chain2[Chain2]
    T30_usecase_usecase_RefVerifyScope_All[All]
  end
  subgraph R31_usecase_usecase_RefVerifierPort["ref_verify::RefVerifierPort"]
    direction TB
    R31_usecase_usecase_RefVerifierPort__self[RefVerifierPort]
    R31_usecase_usecase_RefVerifierPort_verify_pair([verify_pair])
  end
  subgraph R43_usecase_usecase_RefVerifyApplicationService["ref_verify::RefVerifyApplicationService"]
    direction TB
    R43_usecase_usecase_RefVerifyApplicationService__self[RefVerifyApplicationService]
    R43_usecase_usecase_RefVerifyApplicationService_execute([execute])
  end
  subgraph R34_usecase_usecase_RefVerifyCachePort["ref_verify::RefVerifyCachePort"]
    direction TB
    R34_usecase_usecase_RefVerifyCachePort__self[RefVerifyCachePort]
    R34_usecase_usecase_RefVerifyCachePort_load_entries([load_entries])
    R34_usecase_usecase_RefVerifyCachePort_save_entries([save_entries])
  end
  subgraph R39_usecase_usecase_RefVerifyPairSourcePort["ref_verify::RefVerifyPairSourcePort"]
    direction TB
    R39_usecase_usecase_RefVerifyPairSourcePort__self[RefVerifyPairSourcePort]
    R39_usecase_usecase_RefVerifyPairSourcePort_load_pairs([load_pairs])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_ref_verify["infrastructure::ref_verify"]
    direction TB
  subgraph T52_infrastructure_infrastructure_RefVerifyScopeResolver["ref_verify::scope_resolver::RefVerifyScopeResolver"]
    direction TB
    T52_infrastructure_infrastructure_RefVerifyScopeResolver__self[RefVerifyScopeResolver]
    T52_infrastructure_infrastructure_RefVerifyScopeResolver_new([new])
    T52_infrastructure_infrastructure_RefVerifyScopeResolver_resolve([resolve])
  end
  subgraph T57_infrastructure_infrastructure_RefVerifyScopeResolverError["ref_verify::scope_resolver::RefVerifyScopeResolverError"]
    direction TB
    T57_infrastructure_infrastructure_RefVerifyScopeResolverError__self[RefVerifyScopeResolverError]
    T57_infrastructure_infrastructure_RefVerifyScopeResolverError_Io[Io]
    T57_infrastructure_infrastructure_RefVerifyScopeResolverError_PartialCatalogues[PartialCatalogues]
  end
  end
end
T52_infrastructure_infrastructure_RefVerifyScopeResolver_new --> T52_infrastructure_infrastructure_RefVerifyScopeResolver__self
T52_infrastructure_infrastructure_RefVerifyScopeResolver_resolve --> T57_infrastructure_infrastructure_RefVerifyScopeResolverError__self
T52_infrastructure_infrastructure_RefVerifyScopeResolver_resolve --> T30_usecase_usecase_RefVerifyScope__self
class T30_usecase_usecase_RefVerifyScope_Chain1 variant_node
class T30_usecase_usecase_RefVerifyScope_Chain2 variant_node
class T30_usecase_usecase_RefVerifyScope_All variant_node
class T30_usecase_usecase_RefVerifyScope__self value_object
class R31_usecase_usecase_RefVerifierPort_verify_pair method_node
class R31_usecase_usecase_RefVerifierPort__self secondary_port
class R43_usecase_usecase_RefVerifyApplicationService_execute method_node
class R43_usecase_usecase_RefVerifyApplicationService__self app_service
class R34_usecase_usecase_RefVerifyCachePort_load_entries method_node
class R34_usecase_usecase_RefVerifyCachePort_save_entries method_node
class R34_usecase_usecase_RefVerifyCachePort__self secondary_port
class R39_usecase_usecase_RefVerifyPairSourcePort_load_pairs method_node
class R39_usecase_usecase_RefVerifyPairSourcePort__self secondary_port
class T52_infrastructure_infrastructure_RefVerifyScopeResolver_new method_node
class T52_infrastructure_infrastructure_RefVerifyScopeResolver_resolve method_node
class T52_infrastructure_infrastructure_RefVerifyScopeResolver__self secondary_adapter
class T57_infrastructure_infrastructure_RefVerifyScopeResolverError_Io variant_node
class T57_infrastructure_infrastructure_RefVerifyScopeResolverError_PartialCatalogues variant_node
class T57_infrastructure_infrastructure_RefVerifyScopeResolverError__self error_type
```
