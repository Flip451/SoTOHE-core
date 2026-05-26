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
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T24_domain_domain_StructKind["tddd::catalogue_v2::composite::StructKind"]
    direction TB
    T24_domain_domain_StructKind__self[StructKind]
    T24_domain_domain_StructKind_new([new])
  end
  subgraph T25_domain_domain_StructShape["tddd::catalogue_v2::composite::StructShape"]
    direction TB
    T25_domain_domain_StructShape__self[StructShape]
    T25_domain_domain_StructShape_Unit[Unit]
    T25_domain_domain_StructShape_Tuple[Tuple]
    T25_domain_domain_StructShape_Plain[Plain]
  end
  subgraph T24_domain_domain_TypeKindV2["tddd::catalogue_v2::composite::TypeKindV2"]
    direction TB
    T24_domain_domain_TypeKindV2__self[TypeKindV2]
    T24_domain_domain_TypeKindV2_Struct[Struct]
    T24_domain_domain_TypeKindV2_Enum[Enum]
    T24_domain_domain_TypeKindV2_TypeAlias[TypeAlias]
  end
  subgraph T29_domain_domain_TypestateMarker["tddd::catalogue_v2::composite::TypestateMarker"]
    direction TB
    T29_domain_domain_TypestateMarker__self[TypestateMarker]
  end
  subgraph T34_domain_domain_TypestateTransitions["tddd::catalogue_v2::composite::TypestateTransitions"]
    direction TB
    T34_domain_domain_TypestateTransitions__self[TypestateTransitions]
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
  subgraph T52_infrastructure_infrastructure_CatalogueDocumentCodec["tddd::catalogue_document_codec::CatalogueDocumentCodec"]
    direction TB
    T52_infrastructure_infrastructure_CatalogueDocumentCodec__self[CatalogueDocumentCodec]
    T52_infrastructure_infrastructure_CatalogueDocumentCodec_new([new])
    T52_infrastructure_infrastructure_CatalogueDocumentCodec_decode([decode])
    T52_infrastructure_infrastructure_CatalogueDocumentCodec_load([load])
    T52_infrastructure_infrastructure_CatalogueDocumentCodec_encode([encode])
  end
  subgraph T57_infrastructure_infrastructure_CatalogueDocumentCodecError["tddd::catalogue_document_codec::CatalogueDocumentCodecError"]
    direction TB
    T57_infrastructure_infrastructure_CatalogueDocumentCodecError__self[CatalogueDocumentCodecError]
    T57_infrastructure_infrastructure_CatalogueDocumentCodecError_Json[Json]
    T57_infrastructure_infrastructure_CatalogueDocumentCodecError_Io[Io]
    T57_infrastructure_infrastructure_CatalogueDocumentCodecError_UnsupportedSchemaVersion[UnsupportedSchemaVersion]
    T57_infrastructure_infrastructure_CatalogueDocumentCodecError_InvalidEntry[InvalidEntry]
    T57_infrastructure_infrastructure_CatalogueDocumentCodecError_CrateNameMismatch[CrateNameMismatch]
    T57_infrastructure_infrastructure_CatalogueDocumentCodecError_CrossCrateFunctionPath[CrossCrateFunctionPath]
  end
  end
end
T24_domain_domain_StructKind_new --o T25_domain_domain_StructShape__self
T24_domain_domain_StructKind_new --o T29_domain_domain_TypestateMarker__self
T24_domain_domain_StructKind_new --> T24_domain_domain_StructKind__self
T24_domain_domain_StructKind__self --o|shape| T25_domain_domain_StructShape__self
T24_domain_domain_StructKind__self --o|typestate| T29_domain_domain_TypestateMarker__self
T24_domain_domain_TypeKindV2_Struct --o T24_domain_domain_StructKind__self
T52_infrastructure_infrastructure_CatalogueDocumentCodec_new --> T52_infrastructure_infrastructure_CatalogueDocumentCodec__self
T52_infrastructure_infrastructure_CatalogueDocumentCodec_decode --> T57_infrastructure_infrastructure_CatalogueDocumentCodecError__self
T52_infrastructure_infrastructure_CatalogueDocumentCodec_load --> T57_infrastructure_infrastructure_CatalogueDocumentCodecError__self
T52_infrastructure_infrastructure_CatalogueDocumentCodec_encode --> T57_infrastructure_infrastructure_CatalogueDocumentCodecError__self
class T24_domain_domain_StructKind_new method_node
class T24_domain_domain_StructKind__self value_object
class T25_domain_domain_StructShape_Unit variant_node
class T25_domain_domain_StructShape_Tuple variant_node
class T25_domain_domain_StructShape_Plain variant_node
class T25_domain_domain_StructShape__self value_object
class T24_domain_domain_TypeKindV2_Struct variant_node
class T24_domain_domain_TypeKindV2_Enum variant_node
class T24_domain_domain_TypeKindV2_TypeAlias variant_node
class T24_domain_domain_TypeKindV2__self value_object
class T29_domain_domain_TypestateMarker__self value_object
class T34_domain_domain_TypestateTransitions__self value_object
class T52_infrastructure_infrastructure_CatalogueDocumentCodec_new method_node
class T52_infrastructure_infrastructure_CatalogueDocumentCodec_decode method_node
class T52_infrastructure_infrastructure_CatalogueDocumentCodec_load method_node
class T52_infrastructure_infrastructure_CatalogueDocumentCodec_encode method_node
class T52_infrastructure_infrastructure_CatalogueDocumentCodec__self secondary_adapter
class T57_infrastructure_infrastructure_CatalogueDocumentCodecError_Json variant_node
class T57_infrastructure_infrastructure_CatalogueDocumentCodecError_Io variant_node
class T57_infrastructure_infrastructure_CatalogueDocumentCodecError_UnsupportedSchemaVersion variant_node
class T57_infrastructure_infrastructure_CatalogueDocumentCodecError_InvalidEntry variant_node
class T57_infrastructure_infrastructure_CatalogueDocumentCodecError_CrateNameMismatch variant_node
class T57_infrastructure_infrastructure_CatalogueDocumentCodecError_CrossCrateFunctionPath variant_node
class T57_infrastructure_infrastructure_CatalogueDocumentCodecError__self error_type
```
