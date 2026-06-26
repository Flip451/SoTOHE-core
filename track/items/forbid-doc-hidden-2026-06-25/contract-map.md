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
  subgraph usecase_usecase_module_verify["usecase::verify"]
    direction TB
  subgraph T32_usecase_usecase_VerifyInteractor["verify::VerifyInteractor"]
    direction TB
    T32_usecase_usecase_VerifyInteractor__self[VerifyInteractor]
  end
  subgraph R26_usecase_usecase_VerifyPort["verify::VerifyPort"]
    direction TB
    R26_usecase_usecase_VerifyPort__self[VerifyPort]
    R26_usecase_usecase_VerifyPort_verify_tech_stack([verify_tech_stack])
    R26_usecase_usecase_VerifyPort_verify_latest_track([verify_latest_track])
    R26_usecase_usecase_VerifyPort_verify_arch_docs([verify_arch_docs])
    R26_usecase_usecase_VerifyPort_verify_layers([verify_layers])
    R26_usecase_usecase_VerifyPort_verify_hooks_path([verify_hooks_path])
    R26_usecase_usecase_VerifyPort_verify_spec_attribution([verify_spec_attribution])
    R26_usecase_usecase_VerifyPort_verify_spec_frontmatter([verify_spec_frontmatter])
    R26_usecase_usecase_VerifyPort_verify_canonical_modules([verify_canonical_modules])
    R26_usecase_usecase_VerifyPort_verify_module_size([verify_module_size])
    R26_usecase_usecase_VerifyPort_verify_domain_purity([verify_domain_purity])
    R26_usecase_usecase_VerifyPort_verify_domain_strings([verify_domain_strings])
    R26_usecase_usecase_VerifyPort_verify_usecase_purity([verify_usecase_purity])
    R26_usecase_usecase_VerifyPort_verify_doc_links([verify_doc_links])
    R26_usecase_usecase_VerifyPort_verify_view_freshness([verify_view_freshness])
    R26_usecase_usecase_VerifyPort_verify_spec_signals([verify_spec_signals])
    R26_usecase_usecase_VerifyPort_verify_plan_artifact_refs([verify_plan_artifact_refs])
    R26_usecase_usecase_VerifyPort_verify_catalogue_spec_refs([verify_catalogue_spec_refs])
    R26_usecase_usecase_VerifyPort_verify_doc_hidden([verify_doc_hidden])
  end
  subgraph R29_usecase_usecase_VerifyService["verify::VerifyService"]
    direction TB
    R29_usecase_usecase_VerifyService__self[VerifyService]
    R29_usecase_usecase_VerifyService_verify_tech_stack([verify_tech_stack])
    R29_usecase_usecase_VerifyService_verify_latest_track([verify_latest_track])
    R29_usecase_usecase_VerifyService_verify_arch_docs([verify_arch_docs])
    R29_usecase_usecase_VerifyService_verify_layers([verify_layers])
    R29_usecase_usecase_VerifyService_verify_hooks_path([verify_hooks_path])
    R29_usecase_usecase_VerifyService_verify_spec_attribution([verify_spec_attribution])
    R29_usecase_usecase_VerifyService_verify_spec_frontmatter([verify_spec_frontmatter])
    R29_usecase_usecase_VerifyService_verify_canonical_modules([verify_canonical_modules])
    R29_usecase_usecase_VerifyService_verify_module_size([verify_module_size])
    R29_usecase_usecase_VerifyService_verify_domain_purity([verify_domain_purity])
    R29_usecase_usecase_VerifyService_verify_domain_strings([verify_domain_strings])
    R29_usecase_usecase_VerifyService_verify_usecase_purity([verify_usecase_purity])
    R29_usecase_usecase_VerifyService_verify_doc_links([verify_doc_links])
    R29_usecase_usecase_VerifyService_verify_view_freshness([verify_view_freshness])
    R29_usecase_usecase_VerifyService_verify_spec_signals([verify_spec_signals])
    R29_usecase_usecase_VerifyService_verify_plan_artifact_refs([verify_plan_artifact_refs])
    R29_usecase_usecase_VerifyService_verify_catalogue_spec_refs([verify_catalogue_spec_refs])
    R29_usecase_usecase_VerifyService_verify_doc_hidden([verify_doc_hidden])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_verify["infrastructure::verify"]
    direction TB
  F72_infrastructure_infrastructure_infrastructure__verify__doc_hidden__verify[[verify]]
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_verify["cli_driver::verify"]
    direction TB
  subgraph T33_cli_driver_cli_driver_VerifyInput["verify::VerifyInput"]
    direction TB
    T33_cli_driver_cli_driver_VerifyInput__self[VerifyInput]
    T33_cli_driver_cli_driver_VerifyInput_TechStack[TechStack]
    T33_cli_driver_cli_driver_VerifyInput_LatestTrack[LatestTrack]
    T33_cli_driver_cli_driver_VerifyInput_ArchDocs[ArchDocs]
    T33_cli_driver_cli_driver_VerifyInput_Layers[Layers]
    T33_cli_driver_cli_driver_VerifyInput_HooksPath[HooksPath]
    T33_cli_driver_cli_driver_VerifyInput_SpecAttribution[SpecAttribution]
    T33_cli_driver_cli_driver_VerifyInput_SpecFrontmatter[SpecFrontmatter]
    T33_cli_driver_cli_driver_VerifyInput_CanonicalModules[CanonicalModules]
    T33_cli_driver_cli_driver_VerifyInput_ModuleSize[ModuleSize]
    T33_cli_driver_cli_driver_VerifyInput_DomainPurity[DomainPurity]
    T33_cli_driver_cli_driver_VerifyInput_DomainStrings[DomainStrings]
    T33_cli_driver_cli_driver_VerifyInput_UsecasePurity[UsecasePurity]
    T33_cli_driver_cli_driver_VerifyInput_DocLinks[DocLinks]
    T33_cli_driver_cli_driver_VerifyInput_ViewFreshness[ViewFreshness]
    T33_cli_driver_cli_driver_VerifyInput_SpecSignals[SpecSignals]
    T33_cli_driver_cli_driver_VerifyInput_PlanArtifactRefs[PlanArtifactRefs]
    T33_cli_driver_cli_driver_VerifyInput_CatalogueSpecRefs[CatalogueSpecRefs]
    T33_cli_driver_cli_driver_VerifyInput_DocHidden[DocHidden]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T21_cli_cli_VerifyCommand["commands::verify::VerifyCommand"]
    direction TB
    T21_cli_cli_VerifyCommand__self[VerifyCommand]
    T21_cli_cli_VerifyCommand_TechStack[TechStack]
    T21_cli_cli_VerifyCommand_LatestTrack[LatestTrack]
    T21_cli_cli_VerifyCommand_ArchDocs[ArchDocs]
    T21_cli_cli_VerifyCommand_Layers[Layers]
    T21_cli_cli_VerifyCommand_HooksPath[HooksPath]
    T21_cli_cli_VerifyCommand_SpecAttribution[SpecAttribution]
    T21_cli_cli_VerifyCommand_SpecFrontmatter[SpecFrontmatter]
    T21_cli_cli_VerifyCommand_CanonicalModules[CanonicalModules]
    T21_cli_cli_VerifyCommand_ModuleSize[ModuleSize]
    T21_cli_cli_VerifyCommand_DomainPurity[DomainPurity]
    T21_cli_cli_VerifyCommand_DomainStrings[DomainStrings]
    T21_cli_cli_VerifyCommand_UsecasePurity[UsecasePurity]
    T21_cli_cli_VerifyCommand_DocLinks[DocLinks]
    T21_cli_cli_VerifyCommand_ViewFreshness[ViewFreshness]
    T21_cli_cli_VerifyCommand_SpecSignals[SpecSignals]
    T21_cli_cli_VerifyCommand_PlanArtifactRefs[PlanArtifactRefs]
    T21_cli_cli_VerifyCommand_CatalogueSpecRefs[CatalogueSpecRefs]
    T21_cli_cli_VerifyCommand_DocHidden[DocHidden]
  end
  end
end
T32_usecase_usecase_VerifyInteractor__self -.impl.-> R29_usecase_usecase_VerifyService__self
class T32_usecase_usecase_VerifyInteractor__self interactor
class R26_usecase_usecase_VerifyPort_verify_tech_stack method_node
class R26_usecase_usecase_VerifyPort_verify_latest_track method_node
class R26_usecase_usecase_VerifyPort_verify_arch_docs method_node
class R26_usecase_usecase_VerifyPort_verify_layers method_node
class R26_usecase_usecase_VerifyPort_verify_hooks_path method_node
class R26_usecase_usecase_VerifyPort_verify_spec_attribution method_node
class R26_usecase_usecase_VerifyPort_verify_spec_frontmatter method_node
class R26_usecase_usecase_VerifyPort_verify_canonical_modules method_node
class R26_usecase_usecase_VerifyPort_verify_module_size method_node
class R26_usecase_usecase_VerifyPort_verify_domain_purity method_node
class R26_usecase_usecase_VerifyPort_verify_domain_strings method_node
class R26_usecase_usecase_VerifyPort_verify_usecase_purity method_node
class R26_usecase_usecase_VerifyPort_verify_doc_links method_node
class R26_usecase_usecase_VerifyPort_verify_view_freshness method_node
class R26_usecase_usecase_VerifyPort_verify_spec_signals method_node
class R26_usecase_usecase_VerifyPort_verify_plan_artifact_refs method_node
class R26_usecase_usecase_VerifyPort_verify_catalogue_spec_refs method_node
class R26_usecase_usecase_VerifyPort_verify_doc_hidden method_node
class R26_usecase_usecase_VerifyPort__self secondary_port
class R29_usecase_usecase_VerifyService_verify_tech_stack method_node
class R29_usecase_usecase_VerifyService_verify_latest_track method_node
class R29_usecase_usecase_VerifyService_verify_arch_docs method_node
class R29_usecase_usecase_VerifyService_verify_layers method_node
class R29_usecase_usecase_VerifyService_verify_hooks_path method_node
class R29_usecase_usecase_VerifyService_verify_spec_attribution method_node
class R29_usecase_usecase_VerifyService_verify_spec_frontmatter method_node
class R29_usecase_usecase_VerifyService_verify_canonical_modules method_node
class R29_usecase_usecase_VerifyService_verify_module_size method_node
class R29_usecase_usecase_VerifyService_verify_domain_purity method_node
class R29_usecase_usecase_VerifyService_verify_domain_strings method_node
class R29_usecase_usecase_VerifyService_verify_usecase_purity method_node
class R29_usecase_usecase_VerifyService_verify_doc_links method_node
class R29_usecase_usecase_VerifyService_verify_view_freshness method_node
class R29_usecase_usecase_VerifyService_verify_spec_signals method_node
class R29_usecase_usecase_VerifyService_verify_plan_artifact_refs method_node
class R29_usecase_usecase_VerifyService_verify_catalogue_spec_refs method_node
class R29_usecase_usecase_VerifyService_verify_doc_hidden method_node
class R29_usecase_usecase_VerifyService__self app_service
class F72_infrastructure_infrastructure_infrastructure__verify__doc_hidden__verify free_function
class F72_infrastructure_infrastructure_infrastructure__verify__doc_hidden__verify function_node
class T33_cli_driver_cli_driver_VerifyInput_TechStack variant_node
class T33_cli_driver_cli_driver_VerifyInput_LatestTrack variant_node
class T33_cli_driver_cli_driver_VerifyInput_ArchDocs variant_node
class T33_cli_driver_cli_driver_VerifyInput_Layers variant_node
class T33_cli_driver_cli_driver_VerifyInput_HooksPath variant_node
class T33_cli_driver_cli_driver_VerifyInput_SpecAttribution variant_node
class T33_cli_driver_cli_driver_VerifyInput_SpecFrontmatter variant_node
class T33_cli_driver_cli_driver_VerifyInput_CanonicalModules variant_node
class T33_cli_driver_cli_driver_VerifyInput_ModuleSize variant_node
class T33_cli_driver_cli_driver_VerifyInput_DomainPurity variant_node
class T33_cli_driver_cli_driver_VerifyInput_DomainStrings variant_node
class T33_cli_driver_cli_driver_VerifyInput_UsecasePurity variant_node
class T33_cli_driver_cli_driver_VerifyInput_DocLinks variant_node
class T33_cli_driver_cli_driver_VerifyInput_ViewFreshness variant_node
class T33_cli_driver_cli_driver_VerifyInput_SpecSignals variant_node
class T33_cli_driver_cli_driver_VerifyInput_PlanArtifactRefs variant_node
class T33_cli_driver_cli_driver_VerifyInput_CatalogueSpecRefs variant_node
class T33_cli_driver_cli_driver_VerifyInput_DocHidden variant_node
class T33_cli_driver_cli_driver_VerifyInput__self dto
class T21_cli_cli_VerifyCommand_TechStack variant_node
class T21_cli_cli_VerifyCommand_LatestTrack variant_node
class T21_cli_cli_VerifyCommand_ArchDocs variant_node
class T21_cli_cli_VerifyCommand_Layers variant_node
class T21_cli_cli_VerifyCommand_HooksPath variant_node
class T21_cli_cli_VerifyCommand_SpecAttribution variant_node
class T21_cli_cli_VerifyCommand_SpecFrontmatter variant_node
class T21_cli_cli_VerifyCommand_CanonicalModules variant_node
class T21_cli_cli_VerifyCommand_ModuleSize variant_node
class T21_cli_cli_VerifyCommand_DomainPurity variant_node
class T21_cli_cli_VerifyCommand_DomainStrings variant_node
class T21_cli_cli_VerifyCommand_UsecasePurity variant_node
class T21_cli_cli_VerifyCommand_DocLinks variant_node
class T21_cli_cli_VerifyCommand_ViewFreshness variant_node
class T21_cli_cli_VerifyCommand_SpecSignals variant_node
class T21_cli_cli_VerifyCommand_PlanArtifactRefs variant_node
class T21_cli_cli_VerifyCommand_CatalogueSpecRefs variant_node
class T21_cli_cli_VerifyCommand_DocHidden variant_node
class T21_cli_cli_VerifyCommand__self dto
```
