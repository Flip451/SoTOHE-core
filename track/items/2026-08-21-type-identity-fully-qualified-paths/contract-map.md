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
  subgraph T31_domain_domain_CatalogueDocument["tddd::catalogue_v2::document::CatalogueDocument"]
    direction TB
    T31_domain_domain_CatalogueDocument__self[CatalogueDocument]
    T31_domain_domain_CatalogueDocument_new([new])
    T31_domain_domain_CatalogueDocument_schema_version([schema_version])
    T31_domain_domain_CatalogueDocument_crate_name([crate_name])
    T31_domain_domain_CatalogueDocument_layer([layer])
    T31_domain_domain_CatalogueDocument_types([types])
    T31_domain_domain_CatalogueDocument_traits([traits])
    T31_domain_domain_CatalogueDocument_functions([functions])
    T31_domain_domain_CatalogueDocument_inherent_impls([inherent_impls])
    T31_domain_domain_CatalogueDocument_trait_impls([trait_impls])
    T31_domain_domain_CatalogueDocument_deletions([deletions])
    T31_domain_domain_CatalogueDocument_insert_type([insert_type])
    T31_domain_domain_CatalogueDocument_insert_trait([insert_trait])
    T31_domain_domain_CatalogueDocument_insert_function([insert_function])
    T31_domain_domain_CatalogueDocument_push_inherent_impl([push_inherent_impl])
    T31_domain_domain_CatalogueDocument_push_trait_impl([push_trait_impl])
    T31_domain_domain_CatalogueDocument_push_deletion([push_deletion])
    T31_domain_domain_CatalogueDocument_validate_filename([validate_filename])
  end
  subgraph T31_domain_domain_CatalogueEntryKey["tddd::semantic_verify::CatalogueEntryKey"]
    direction TB
    T31_domain_domain_CatalogueEntryKey__self[CatalogueEntryKey]
    T31_domain_domain_CatalogueEntryKey_try_new([try_new])
    T31_domain_domain_CatalogueEntryKey_as_str([as_str])
  end
  subgraph T36_domain_domain_CatalogueSchemaVersion["tddd::catalogue_v2::document::CatalogueSchemaVersion"]
    direction TB
    T36_domain_domain_CatalogueSchemaVersion__self[CatalogueSchemaVersion]
    T36_domain_domain_CatalogueSchemaVersion_new([new])
    T36_domain_domain_CatalogueSchemaVersion_value([value])
  end
  subgraph T28_domain_domain_DeletionRecord["tddd::catalogue_v2::deletions::DeletionRecord"]
    direction TB
    T28_domain_domain_DeletionRecord__self[DeletionRecord]
    T28_domain_domain_DeletionRecord_Type[Type]
    T28_domain_domain_DeletionRecord_Trait[Trait]
    T28_domain_domain_DeletionRecord_Function[Function]
    T28_domain_domain_DeletionRecord_spec_refs([spec_refs])
    T28_domain_domain_DeletionRecord_informal_grounds([informal_grounds])
  end
  subgraph T36_domain_domain_FullyQualifiedItemPath["tddd::catalogue_v2::identifiers::FullyQualifiedItemPath"]
    direction TB
    T36_domain_domain_FullyQualifiedItemPath__self[FullyQualifiedItemPath]
    T36_domain_domain_FullyQualifiedItemPath_new([new])
    T36_domain_domain_FullyQualifiedItemPath_crate_name([crate_name])
    T36_domain_domain_FullyQualifiedItemPath_module_path([module_path])
    T36_domain_domain_FullyQualifiedItemPath_name([name])
    T36_domain_domain_FullyQualifiedItemPath_from_catalogue_entry_key([from_catalogue_entry_key])
    T36_domain_domain_FullyQualifiedItemPath_from_fully_qualified_key([from_fully_qualified_key])
  end
  subgraph T32_domain_domain_InherentImplDeclV2["tddd::catalogue_v2::entries::InherentImplDeclV2"]
    direction TB
    T32_domain_domain_InherentImplDeclV2__self[InherentImplDeclV2]
    T32_domain_domain_InherentImplDeclV2_new([new])
    T32_domain_domain_InherentImplDeclV2_type_name([type_name])
    T32_domain_domain_InherentImplDeclV2_impl_generics([impl_generics])
    T32_domain_domain_InherentImplDeclV2_impl_where_predicates([impl_where_predicates])
    T32_domain_domain_InherentImplDeclV2_methods([methods])
  end
  subgraph T36_domain_domain_NewTypeGraphCodecError["tddd::new_typegraph_codec_error::NewTypeGraphCodecError"]
    direction TB
    T36_domain_domain_NewTypeGraphCodecError__self[NewTypeGraphCodecError]
    T36_domain_domain_NewTypeGraphCodecError_InvalidTypeRef[InvalidTypeRef]
    T36_domain_domain_NewTypeGraphCodecError_AmbiguousIdentifier[AmbiguousIdentifier]
    T36_domain_domain_NewTypeGraphCodecError_UnresolvedIdentifier[UnresolvedIdentifier]
  end
  subgraph T27_domain_domain_TraitRefScope["tddd::catalogue_v2::traits::TraitRefScope"]
    direction TB
    T27_domain_domain_TraitRefScope__self[TraitRefScope]
    T27_domain_domain_TraitRefScope_SelfCrate[SelfCrate]
    T27_domain_domain_TraitRefScope_Workspace[Workspace]
    T27_domain_domain_TraitRefScope_External[External]
  end
  subgraph R42_domain_domain_CatalogueToExtendedCratePort["tddd::catalogue_to_extended_crate_port::CatalogueToExtendedCratePort"]
    direction TB
    R42_domain_domain_CatalogueToExtendedCratePort__self[CatalogueToExtendedCratePort]
    R42_domain_domain_CatalogueToExtendedCratePort_encode([encode])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_tddd["infrastructure::tddd"]
    direction TB
  subgraph T51_infrastructure_infrastructure_CanonicalTypeIdentity["tddd::canonical_type_identity::CanonicalTypeIdentity"]
    direction TB
    T51_infrastructure_infrastructure_CanonicalTypeIdentity__self[CanonicalTypeIdentity]
    T51_infrastructure_infrastructure_CanonicalTypeIdentity_as_str([as_str])
  end
  F108_infrastructure_infrastructure_infrastructure__tddd__canonical_type_identity__canonicalize_catalogue_type_ref[[canonicalize_catalogue_type_ref]]
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
T31_domain_domain_CatalogueDocument_new --o T36_domain_domain_CatalogueSchemaVersion__self
T31_domain_domain_CatalogueDocument_new --> T31_domain_domain_CatalogueDocument__self
T31_domain_domain_CatalogueDocument_schema_version --> T36_domain_domain_CatalogueSchemaVersion__self
T31_domain_domain_CatalogueDocument_types --> T31_domain_domain_CatalogueEntryKey__self
T31_domain_domain_CatalogueDocument_traits --> T31_domain_domain_CatalogueEntryKey__self
T31_domain_domain_CatalogueDocument_inherent_impls --> T32_domain_domain_InherentImplDeclV2__self
T31_domain_domain_CatalogueDocument_deletions --> T28_domain_domain_DeletionRecord__self
T31_domain_domain_CatalogueDocument_insert_type --o T31_domain_domain_CatalogueEntryKey__self
T31_domain_domain_CatalogueDocument_insert_trait --o T31_domain_domain_CatalogueEntryKey__self
T31_domain_domain_CatalogueDocument_push_inherent_impl --o T32_domain_domain_InherentImplDeclV2__self
T31_domain_domain_CatalogueDocument_push_deletion --o T28_domain_domain_DeletionRecord__self
T31_domain_domain_CatalogueEntryKey_try_new --> T31_domain_domain_CatalogueEntryKey__self
T36_domain_domain_CatalogueSchemaVersion_new --> T36_domain_domain_CatalogueSchemaVersion__self
T28_domain_domain_DeletionRecord_Type --o|name| T31_domain_domain_CatalogueEntryKey__self
T28_domain_domain_DeletionRecord_Trait --o|name| T31_domain_domain_CatalogueEntryKey__self
T36_domain_domain_FullyQualifiedItemPath_new --> T36_domain_domain_FullyQualifiedItemPath__self
T36_domain_domain_FullyQualifiedItemPath_from_catalogue_entry_key --o T31_domain_domain_CatalogueEntryKey__self
T36_domain_domain_FullyQualifiedItemPath_from_catalogue_entry_key --> T36_domain_domain_FullyQualifiedItemPath__self
T36_domain_domain_FullyQualifiedItemPath_from_fully_qualified_key --o T31_domain_domain_CatalogueEntryKey__self
T36_domain_domain_FullyQualifiedItemPath_from_fully_qualified_key --> T36_domain_domain_FullyQualifiedItemPath__self
T32_domain_domain_InherentImplDeclV2_new --o T31_domain_domain_CatalogueEntryKey__self
T32_domain_domain_InherentImplDeclV2_new --> T32_domain_domain_InherentImplDeclV2__self
T32_domain_domain_InherentImplDeclV2_type_name --> T31_domain_domain_CatalogueEntryKey__self
T36_domain_domain_NewTypeGraphCodecError_AmbiguousIdentifier --o T36_domain_domain_FullyQualifiedItemPath__self
T27_domain_domain_TraitRefScope_SelfCrate --o T31_domain_domain_CatalogueEntryKey__self
T27_domain_domain_TraitRefScope_Workspace --o T31_domain_domain_CatalogueEntryKey__self
R42_domain_domain_CatalogueToExtendedCratePort_encode --o T31_domain_domain_CatalogueDocument__self
R42_domain_domain_CatalogueToExtendedCratePort_encode --> T36_domain_domain_NewTypeGraphCodecError__self
F108_infrastructure_infrastructure_infrastructure__tddd__canonical_type_identity__canonicalize_catalogue_type_ref --> T51_infrastructure_infrastructure_CanonicalTypeIdentity__self
F108_infrastructure_infrastructure_infrastructure__tddd__canonical_type_identity__canonicalize_catalogue_type_ref --> T36_domain_domain_NewTypeGraphCodecError__self
class T31_domain_domain_CatalogueDocument_new method_node
class T31_domain_domain_CatalogueDocument_schema_version method_node
class T31_domain_domain_CatalogueDocument_crate_name method_node
class T31_domain_domain_CatalogueDocument_layer method_node
class T31_domain_domain_CatalogueDocument_types method_node
class T31_domain_domain_CatalogueDocument_traits method_node
class T31_domain_domain_CatalogueDocument_functions method_node
class T31_domain_domain_CatalogueDocument_inherent_impls method_node
class T31_domain_domain_CatalogueDocument_trait_impls method_node
class T31_domain_domain_CatalogueDocument_deletions method_node
class T31_domain_domain_CatalogueDocument_insert_type method_node
class T31_domain_domain_CatalogueDocument_insert_trait method_node
class T31_domain_domain_CatalogueDocument_insert_function method_node
class T31_domain_domain_CatalogueDocument_push_inherent_impl method_node
class T31_domain_domain_CatalogueDocument_push_trait_impl method_node
class T31_domain_domain_CatalogueDocument_push_deletion method_node
class T31_domain_domain_CatalogueDocument_validate_filename method_node
class T31_domain_domain_CatalogueDocument__self aggregate_root
class T31_domain_domain_CatalogueEntryKey_try_new method_node
class T31_domain_domain_CatalogueEntryKey_as_str method_node
class T31_domain_domain_CatalogueEntryKey__self value_object
class T36_domain_domain_CatalogueSchemaVersion_new method_node
class T36_domain_domain_CatalogueSchemaVersion_value method_node
class T36_domain_domain_CatalogueSchemaVersion__self value_object
class T28_domain_domain_DeletionRecord_Type variant_node
class T28_domain_domain_DeletionRecord_Trait variant_node
class T28_domain_domain_DeletionRecord_Function variant_node
class T28_domain_domain_DeletionRecord_spec_refs method_node
class T28_domain_domain_DeletionRecord_informal_grounds method_node
class T28_domain_domain_DeletionRecord__self value_object
class T36_domain_domain_FullyQualifiedItemPath_new method_node
class T36_domain_domain_FullyQualifiedItemPath_crate_name method_node
class T36_domain_domain_FullyQualifiedItemPath_module_path method_node
class T36_domain_domain_FullyQualifiedItemPath_name method_node
class T36_domain_domain_FullyQualifiedItemPath_from_catalogue_entry_key method_node
class T36_domain_domain_FullyQualifiedItemPath_from_fully_qualified_key method_node
class T36_domain_domain_FullyQualifiedItemPath__self value_object
class T32_domain_domain_InherentImplDeclV2_new method_node
class T32_domain_domain_InherentImplDeclV2_type_name method_node
class T32_domain_domain_InherentImplDeclV2_impl_generics method_node
class T32_domain_domain_InherentImplDeclV2_impl_where_predicates method_node
class T32_domain_domain_InherentImplDeclV2_methods method_node
class T32_domain_domain_InherentImplDeclV2__self value_object
class T36_domain_domain_NewTypeGraphCodecError_InvalidTypeRef variant_node
class T36_domain_domain_NewTypeGraphCodecError_AmbiguousIdentifier variant_node
class T36_domain_domain_NewTypeGraphCodecError_UnresolvedIdentifier variant_node
class T36_domain_domain_NewTypeGraphCodecError__self error_type
class T27_domain_domain_TraitRefScope_SelfCrate variant_node
class T27_domain_domain_TraitRefScope_Workspace variant_node
class T27_domain_domain_TraitRefScope_External variant_node
class T27_domain_domain_TraitRefScope__self value_object
class R42_domain_domain_CatalogueToExtendedCratePort_encode method_node
class R42_domain_domain_CatalogueToExtendedCratePort__self secondary_port
class T51_infrastructure_infrastructure_CanonicalTypeIdentity_as_str method_node
class T51_infrastructure_infrastructure_CanonicalTypeIdentity__self value_object
class F108_infrastructure_infrastructure_infrastructure__tddd__canonical_type_identity__canonicalize_catalogue_type_ref free_function
class F108_infrastructure_infrastructure_infrastructure__tddd__canonical_type_identity__canonicalize_catalogue_type_ref function_node
```
