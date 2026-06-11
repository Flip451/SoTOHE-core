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
  subgraph domain_domain_module_plan_ref["domain::plan_ref"]
    direction TB
  subgraph T21_domain_domain_SpecRef["plan_ref::spec_ref::SpecRef"]
    direction TB
    T21_domain_domain_SpecRef__self[SpecRef]
    T21_domain_domain_SpecRef_new([new])
  end
  end
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T32_domain_domain_SpecRefFindingKind["tddd::catalogue_spec_signal::SpecRefFindingKind"]
    direction TB
    T32_domain_domain_SpecRefFindingKind__self[SpecRefFindingKind]
    T32_domain_domain_SpecRefFindingKind_DanglingAnchor[DanglingAnchor]
    T32_domain_domain_SpecRefFindingKind_StaleSignals[StaleSignals]
  end
  end
end
subgraph usecase["usecase"]
  direction TB
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_verify["infrastructure::verify"]
    direction TB
  subgraph T51_infrastructure_infrastructure_PlanArtifactRefsError["verify::plan_artifact_refs::PlanArtifactRefsError"]
    direction TB
    T51_infrastructure_infrastructure_PlanArtifactRefsError__self[PlanArtifactRefsError]
    T51_infrastructure_infrastructure_PlanArtifactRefsError_Json[Json]
    T51_infrastructure_infrastructure_PlanArtifactRefsError_Io[Io]
    T51_infrastructure_infrastructure_PlanArtifactRefsError_UnresolvedSpecRef[UnresolvedSpecRef]
    T51_infrastructure_infrastructure_PlanArtifactRefsError_InvalidAnchor[InvalidAnchor]
    T51_infrastructure_infrastructure_PlanArtifactRefsError_CoverageViolation[CoverageViolation]
  end
  end
end
T21_domain_domain_SpecRef_new --> T21_domain_domain_SpecRef__self
class T21_domain_domain_SpecRef_new method_node
class T21_domain_domain_SpecRef__self value_object
class T32_domain_domain_SpecRefFindingKind_DanglingAnchor variant_node
class T32_domain_domain_SpecRefFindingKind_StaleSignals variant_node
class T32_domain_domain_SpecRefFindingKind__self value_object
class T51_infrastructure_infrastructure_PlanArtifactRefsError_Json variant_node
class T51_infrastructure_infrastructure_PlanArtifactRefsError_Io variant_node
class T51_infrastructure_infrastructure_PlanArtifactRefsError_UnresolvedSpecRef variant_node
class T51_infrastructure_infrastructure_PlanArtifactRefsError_InvalidAnchor variant_node
class T51_infrastructure_infrastructure_PlanArtifactRefsError_CoverageViolation variant_node
class T51_infrastructure_infrastructure_PlanArtifactRefsError__self error_type
```
