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
  subgraph usecase_usecase_module_semantic_dup["usecase::semantic_dup"]
    direction TB
  subgraph T31_usecase_usecase_BuildIndexError["semantic_dup::BuildIndexError"]
    direction TB
    T31_usecase_usecase_BuildIndexError__self[BuildIndexError]
    T31_usecase_usecase_BuildIndexError_Embedding[Embedding]
    T31_usecase_usecase_BuildIndexError_Index[Index]
    T31_usecase_usecase_BuildIndexError_Io[Io]
  end
  subgraph T29_usecase_usecase_DupCheckError["semantic_dup::DupCheckError"]
    direction TB
    T29_usecase_usecase_DupCheckError__self[DupCheckError]
    T29_usecase_usecase_DupCheckError_Embedding[Embedding]
    T29_usecase_usecase_DupCheckError_Index[Index]
  end
  subgraph T32_usecase_usecase_FindSimilarError["semantic_dup::FindSimilarError"]
    direction TB
    T32_usecase_usecase_FindSimilarError__self[FindSimilarError]
    T32_usecase_usecase_FindSimilarError_Embedding[Embedding]
    T32_usecase_usecase_FindSimilarError_Index[Index]
  end
  subgraph T35_usecase_usecase_MeasureQualityError["semantic_dup::MeasureQualityError"]
    direction TB
    T35_usecase_usecase_MeasureQualityError__self[MeasureQualityError]
    T35_usecase_usecase_MeasureQualityError_Embedding[Embedding]
    T35_usecase_usecase_MeasureQualityError_Index[Index]
    T35_usecase_usecase_MeasureQualityError_Io[Io]
  end
  subgraph T34_usecase_usecase_SemanticIndexError["semantic_dup::SemanticIndexError"]
    direction TB
    T34_usecase_usecase_SemanticIndexError__self[SemanticIndexError]
    T34_usecase_usecase_SemanticIndexError_OpenFailed[OpenFailed]
    T34_usecase_usecase_SemanticIndexError_InsertFailed[InsertFailed]
    T34_usecase_usecase_SemanticIndexError_DeleteFailed[DeleteFailed]
    T34_usecase_usecase_SemanticIndexError_SearchFailed[SearchFailed]
  end
  subgraph R29_usecase_usecase_EmbeddingPort["semantic_dup::EmbeddingPort"]
    direction TB
    R29_usecase_usecase_EmbeddingPort__self[EmbeddingPort]
    R29_usecase_usecase_EmbeddingPort_embed([embed])
    R29_usecase_usecase_EmbeddingPort_embed_batch([embed_batch])
  end
  subgraph R33_usecase_usecase_SemanticIndexPort["semantic_dup::SemanticIndexPort"]
    direction TB
    R33_usecase_usecase_SemanticIndexPort__self[SemanticIndexPort]
    R33_usecase_usecase_SemanticIndexPort_insert([insert])
    R33_usecase_usecase_SemanticIndexPort_insert_batch([insert_batch])
    R33_usecase_usecase_SemanticIndexPort_delete_by_source_path([delete_by_source_path])
    R33_usecase_usecase_SemanticIndexPort_search([search])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_semantic_dup["infrastructure::semantic_dup"]
    direction TB
  subgraph T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter["semantic_dup::index::LanceDbSemanticIndexAdapter"]
    direction TB
    T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self[LanceDbSemanticIndexAdapter]
    T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new([new])
  end
  end
end
T31_usecase_usecase_BuildIndexError_Index --o T34_usecase_usecase_SemanticIndexError__self
T29_usecase_usecase_DupCheckError_Index --o T34_usecase_usecase_SemanticIndexError__self
T32_usecase_usecase_FindSimilarError_Index --o T34_usecase_usecase_SemanticIndexError__self
T35_usecase_usecase_MeasureQualityError_Index --o T34_usecase_usecase_SemanticIndexError__self
R33_usecase_usecase_SemanticIndexPort_insert --> T34_usecase_usecase_SemanticIndexError__self
R33_usecase_usecase_SemanticIndexPort_insert_batch --> T34_usecase_usecase_SemanticIndexError__self
R33_usecase_usecase_SemanticIndexPort_delete_by_source_path --> T34_usecase_usecase_SemanticIndexError__self
R33_usecase_usecase_SemanticIndexPort_search --> T34_usecase_usecase_SemanticIndexError__self
T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new --> T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self
T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new --> T34_usecase_usecase_SemanticIndexError__self
T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self -.impl.-> R33_usecase_usecase_SemanticIndexPort__self
class T31_usecase_usecase_BuildIndexError_Embedding variant_node
class T31_usecase_usecase_BuildIndexError_Index variant_node
class T31_usecase_usecase_BuildIndexError_Io variant_node
class T31_usecase_usecase_BuildIndexError__self error_type
class T29_usecase_usecase_DupCheckError_Embedding variant_node
class T29_usecase_usecase_DupCheckError_Index variant_node
class T29_usecase_usecase_DupCheckError__self error_type
class T32_usecase_usecase_FindSimilarError_Embedding variant_node
class T32_usecase_usecase_FindSimilarError_Index variant_node
class T32_usecase_usecase_FindSimilarError__self error_type
class T35_usecase_usecase_MeasureQualityError_Embedding variant_node
class T35_usecase_usecase_MeasureQualityError_Index variant_node
class T35_usecase_usecase_MeasureQualityError_Io variant_node
class T35_usecase_usecase_MeasureQualityError__self error_type
class T34_usecase_usecase_SemanticIndexError_OpenFailed variant_node
class T34_usecase_usecase_SemanticIndexError_InsertFailed variant_node
class T34_usecase_usecase_SemanticIndexError_DeleteFailed variant_node
class T34_usecase_usecase_SemanticIndexError_SearchFailed variant_node
class T34_usecase_usecase_SemanticIndexError__self error_type
class R29_usecase_usecase_EmbeddingPort_embed method_node
class R29_usecase_usecase_EmbeddingPort_embed_batch method_node
class R29_usecase_usecase_EmbeddingPort__self secondary_port
class R33_usecase_usecase_SemanticIndexPort_insert method_node
class R33_usecase_usecase_SemanticIndexPort_insert_batch method_node
class R33_usecase_usecase_SemanticIndexPort_delete_by_source_path method_node
class R33_usecase_usecase_SemanticIndexPort_search method_node
class R33_usecase_usecase_SemanticIndexPort__self secondary_port
class T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new method_node
class T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self secondary_adapter
```
