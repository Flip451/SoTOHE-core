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
  subgraph domain_domain_module_semantic_dup["domain::semantic_dup"]
    direction TB
  subgraph T26_domain_domain_CodeFragment["semantic_dup::CodeFragment"]
    direction TB
    T26_domain_domain_CodeFragment__self[CodeFragment]
    T26_domain_domain_CodeFragment_new([new])
  end
  subgraph T30_domain_domain_SemanticDupError["semantic_dup::SemanticDupError"]
    direction TB
    T30_domain_domain_SemanticDupError__self[SemanticDupError]
    T30_domain_domain_SemanticDupError_InvalidScore[InvalidScore]
    T30_domain_domain_SemanticDupError_InvalidTopK[InvalidTopK]
    T30_domain_domain_SemanticDupError_InvalidThreshold[InvalidThreshold]
    T30_domain_domain_SemanticDupError_EmptyContent[EmptyContent]
  end
  subgraph T29_domain_domain_SimilarFragment["semantic_dup::SimilarFragment"]
    direction TB
    T29_domain_domain_SimilarFragment__self[SimilarFragment]
  end
  subgraph T29_domain_domain_SimilarityScore["semantic_dup::SimilarityScore"]
    direction TB
    T29_domain_domain_SimilarityScore__self[SimilarityScore]
    T29_domain_domain_SimilarityScore_new([new])
    T29_domain_domain_SimilarityScore_value([value])
  end
  subgraph T33_domain_domain_SimilarityThreshold["semantic_dup::SimilarityThreshold"]
    direction TB
    T33_domain_domain_SimilarityThreshold__self[SimilarityThreshold]
    T33_domain_domain_SimilarityThreshold_new([new])
    T33_domain_domain_SimilarityThreshold_value([value])
  end
  subgraph T18_domain_domain_TopK["semantic_dup::TopK"]
    direction TB
    T18_domain_domain_TopK__self[TopK]
    T18_domain_domain_TopK_new([new])
    T18_domain_domain_TopK_value([value])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_semantic_dup["usecase::semantic_dup"]
    direction TB
  subgraph T33_usecase_usecase_BuildIndexCommand["semantic_dup::BuildIndexCommand"]
    direction TB
    T33_usecase_usecase_BuildIndexCommand__self[BuildIndexCommand]
  end
  subgraph T31_usecase_usecase_BuildIndexError["semantic_dup::BuildIndexError"]
    direction TB
    T31_usecase_usecase_BuildIndexError__self[BuildIndexError]
    T31_usecase_usecase_BuildIndexError_Embedding[Embedding]
    T31_usecase_usecase_BuildIndexError_Index[Index]
    T31_usecase_usecase_BuildIndexError_Io[Io]
  end
  subgraph T36_usecase_usecase_BuildIndexInteractor["semantic_dup::BuildIndexInteractor"]
    direction TB
    T36_usecase_usecase_BuildIndexInteractor__self[BuildIndexInteractor]
    T36_usecase_usecase_BuildIndexInteractor_new([new])
  end
  subgraph T32_usecase_usecase_BuildIndexOutput["semantic_dup::BuildIndexOutput"]
    direction TB
    T32_usecase_usecase_BuildIndexOutput__self[BuildIndexOutput]
  end
  subgraph T31_usecase_usecase_DupCheckCommand["semantic_dup::DupCheckCommand"]
    direction TB
    T31_usecase_usecase_DupCheckCommand__self[DupCheckCommand]
  end
  subgraph T29_usecase_usecase_DupCheckError["semantic_dup::DupCheckError"]
    direction TB
    T29_usecase_usecase_DupCheckError__self[DupCheckError]
    T29_usecase_usecase_DupCheckError_Embedding[Embedding]
    T29_usecase_usecase_DupCheckError_Index[Index]
  end
  subgraph T34_usecase_usecase_DupCheckInteractor["semantic_dup::DupCheckInteractor"]
    direction TB
    T34_usecase_usecase_DupCheckInteractor__self[DupCheckInteractor]
    T34_usecase_usecase_DupCheckInteractor_new([new])
  end
  subgraph T30_usecase_usecase_DupCheckOutput["semantic_dup::DupCheckOutput"]
    direction TB
    T30_usecase_usecase_DupCheckOutput__self[DupCheckOutput]
  end
  subgraph T31_usecase_usecase_DupCheckWarning["semantic_dup::DupCheckWarning"]
    direction TB
    T31_usecase_usecase_DupCheckWarning__self[DupCheckWarning]
  end
  subgraph T30_usecase_usecase_EmbeddingError["semantic_dup::EmbeddingError"]
    direction TB
    T30_usecase_usecase_EmbeddingError__self[EmbeddingError]
    T30_usecase_usecase_EmbeddingError_ModelLoadFailed[ModelLoadFailed]
    T30_usecase_usecase_EmbeddingError_InferenceFailed[InferenceFailed]
  end
  subgraph T34_usecase_usecase_FindSimilarCommand["semantic_dup::FindSimilarCommand"]
    direction TB
    T34_usecase_usecase_FindSimilarCommand__self[FindSimilarCommand]
  end
  subgraph T32_usecase_usecase_FindSimilarError["semantic_dup::FindSimilarError"]
    direction TB
    T32_usecase_usecase_FindSimilarError__self[FindSimilarError]
    T32_usecase_usecase_FindSimilarError_Embedding[Embedding]
    T32_usecase_usecase_FindSimilarError_Index[Index]
  end
  subgraph T37_usecase_usecase_FindSimilarInteractor["semantic_dup::FindSimilarInteractor"]
    direction TB
    T37_usecase_usecase_FindSimilarInteractor__self[FindSimilarInteractor]
    T37_usecase_usecase_FindSimilarInteractor_new([new])
  end
  subgraph T33_usecase_usecase_FindSimilarOutput["semantic_dup::FindSimilarOutput"]
    direction TB
    T33_usecase_usecase_FindSimilarOutput__self[FindSimilarOutput]
  end
  subgraph T37_usecase_usecase_MeasureQualityCommand["semantic_dup::MeasureQualityCommand"]
    direction TB
    T37_usecase_usecase_MeasureQualityCommand__self[MeasureQualityCommand]
  end
  subgraph T35_usecase_usecase_MeasureQualityError["semantic_dup::MeasureQualityError"]
    direction TB
    T35_usecase_usecase_MeasureQualityError__self[MeasureQualityError]
    T35_usecase_usecase_MeasureQualityError_Embedding[Embedding]
    T35_usecase_usecase_MeasureQualityError_Index[Index]
    T35_usecase_usecase_MeasureQualityError_Io[Io]
  end
  subgraph T40_usecase_usecase_MeasureQualityInteractor["semantic_dup::MeasureQualityInteractor"]
    direction TB
    T40_usecase_usecase_MeasureQualityInteractor__self[MeasureQualityInteractor]
    T40_usecase_usecase_MeasureQualityInteractor_new([new])
  end
  subgraph T30_usecase_usecase_QualityMetrics["semantic_dup::QualityMetrics"]
    direction TB
    T30_usecase_usecase_QualityMetrics__self[QualityMetrics]
  end
  subgraph T34_usecase_usecase_SemanticIndexError["semantic_dup::SemanticIndexError"]
    direction TB
    T34_usecase_usecase_SemanticIndexError__self[SemanticIndexError]
    T34_usecase_usecase_SemanticIndexError_OpenFailed[OpenFailed]
    T34_usecase_usecase_SemanticIndexError_InsertFailed[InsertFailed]
    T34_usecase_usecase_SemanticIndexError_SearchFailed[SearchFailed]
  end
  subgraph R33_usecase_usecase_BuildIndexService["semantic_dup::BuildIndexService"]
    direction TB
    R33_usecase_usecase_BuildIndexService__self[BuildIndexService]
    R33_usecase_usecase_BuildIndexService_build_index([build_index])
  end
  subgraph R31_usecase_usecase_DupCheckService["semantic_dup::DupCheckService"]
    direction TB
    R31_usecase_usecase_DupCheckService__self[DupCheckService]
    R31_usecase_usecase_DupCheckService_dup_check([dup_check])
  end
  subgraph R29_usecase_usecase_EmbeddingPort["semantic_dup::EmbeddingPort"]
    direction TB
    R29_usecase_usecase_EmbeddingPort__self[EmbeddingPort]
    R29_usecase_usecase_EmbeddingPort_embed([embed])
  end
  subgraph R34_usecase_usecase_FindSimilarService["semantic_dup::FindSimilarService"]
    direction TB
    R34_usecase_usecase_FindSimilarService__self[FindSimilarService]
    R34_usecase_usecase_FindSimilarService_find_similar([find_similar])
  end
  subgraph R37_usecase_usecase_MeasureQualityService["semantic_dup::MeasureQualityService"]
    direction TB
    R37_usecase_usecase_MeasureQualityService__self[MeasureQualityService]
    R37_usecase_usecase_MeasureQualityService_measure_quality([measure_quality])
  end
  subgraph R33_usecase_usecase_SemanticIndexPort["semantic_dup::SemanticIndexPort"]
    direction TB
    R33_usecase_usecase_SemanticIndexPort__self[SemanticIndexPort]
    R33_usecase_usecase_SemanticIndexPort_insert([insert])
    R33_usecase_usecase_SemanticIndexPort_search([search])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_semantic_dup["infrastructure::semantic_dup"]
    direction TB
  subgraph T42_infrastructure_infrastructure_ExtractError["semantic_dup::extractor::ExtractError"]
    direction TB
    T42_infrastructure_infrastructure_ExtractError__self[ExtractError]
    T42_infrastructure_infrastructure_ExtractError_Io[Io]
  end
  subgraph T46_infrastructure_infrastructure_FastEmbedAdapter["semantic_dup::embedding::FastEmbedAdapter"]
    direction TB
    T46_infrastructure_infrastructure_FastEmbedAdapter__self[FastEmbedAdapter]
    T46_infrastructure_infrastructure_FastEmbedAdapter_new([new])
  end
  subgraph T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter["semantic_dup::index::LanceDbSemanticIndexAdapter"]
    direction TB
    T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self[LanceDbSemanticIndexAdapter]
    T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new([new])
  end
  F93_infrastructure_infrastructure_infrastructure__semantic_dup__extractor__extract_code_fragments[[extract_code_fragments]]
  end
end
T26_domain_domain_CodeFragment_new --> T26_domain_domain_CodeFragment__self
T26_domain_domain_CodeFragment_new --> T30_domain_domain_SemanticDupError__self
T29_domain_domain_SimilarFragment__self --o|fragment| T26_domain_domain_CodeFragment__self
T29_domain_domain_SimilarFragment__self --o|score| T29_domain_domain_SimilarityScore__self
T29_domain_domain_SimilarityScore_new --> T30_domain_domain_SemanticDupError__self
T29_domain_domain_SimilarityScore_new --> T29_domain_domain_SimilarityScore__self
T33_domain_domain_SimilarityThreshold_new --> T30_domain_domain_SemanticDupError__self
T33_domain_domain_SimilarityThreshold_new --> T33_domain_domain_SimilarityThreshold__self
T18_domain_domain_TopK_new --> T30_domain_domain_SemanticDupError__self
T18_domain_domain_TopK_new --> T18_domain_domain_TopK__self
T33_usecase_usecase_BuildIndexCommand__self --o|fragments| T26_domain_domain_CodeFragment__self
T31_usecase_usecase_BuildIndexError_Embedding --o T30_usecase_usecase_EmbeddingError__self
T31_usecase_usecase_BuildIndexError_Index --o T34_usecase_usecase_SemanticIndexError__self
T36_usecase_usecase_BuildIndexInteractor_new --> T36_usecase_usecase_BuildIndexInteractor__self
T31_usecase_usecase_DupCheckCommand__self --o|fragments| T26_domain_domain_CodeFragment__self
T31_usecase_usecase_DupCheckCommand__self --o|threshold| T33_domain_domain_SimilarityThreshold__self
T29_usecase_usecase_DupCheckError_Embedding --o T30_usecase_usecase_EmbeddingError__self
T29_usecase_usecase_DupCheckError_Index --o T34_usecase_usecase_SemanticIndexError__self
T34_usecase_usecase_DupCheckInteractor_new --> T34_usecase_usecase_DupCheckInteractor__self
T30_usecase_usecase_DupCheckOutput__self --o|warnings| T31_usecase_usecase_DupCheckWarning__self
T31_usecase_usecase_DupCheckWarning__self --o|input_fragment| T26_domain_domain_CodeFragment__self
T31_usecase_usecase_DupCheckWarning__self --o|similar_fragments| T29_domain_domain_SimilarFragment__self
T34_usecase_usecase_FindSimilarCommand__self --o|fragment| T26_domain_domain_CodeFragment__self
T34_usecase_usecase_FindSimilarCommand__self --o|top_k| T18_domain_domain_TopK__self
T32_usecase_usecase_FindSimilarError_Embedding --o T30_usecase_usecase_EmbeddingError__self
T32_usecase_usecase_FindSimilarError_Index --o T34_usecase_usecase_SemanticIndexError__self
T37_usecase_usecase_FindSimilarInteractor_new --> T37_usecase_usecase_FindSimilarInteractor__self
T33_usecase_usecase_FindSimilarOutput__self --o|results| T29_domain_domain_SimilarFragment__self
T37_usecase_usecase_MeasureQualityCommand__self --o|fragments| T26_domain_domain_CodeFragment__self
T35_usecase_usecase_MeasureQualityError_Embedding --o T30_usecase_usecase_EmbeddingError__self
T35_usecase_usecase_MeasureQualityError_Index --o T34_usecase_usecase_SemanticIndexError__self
T40_usecase_usecase_MeasureQualityInteractor_new --> T40_usecase_usecase_MeasureQualityInteractor__self
R33_usecase_usecase_BuildIndexService_build_index --o T33_usecase_usecase_BuildIndexCommand__self
R33_usecase_usecase_BuildIndexService_build_index --> T31_usecase_usecase_BuildIndexError__self
R33_usecase_usecase_BuildIndexService_build_index --> T32_usecase_usecase_BuildIndexOutput__self
R31_usecase_usecase_DupCheckService_dup_check --o T31_usecase_usecase_DupCheckCommand__self
R31_usecase_usecase_DupCheckService_dup_check --> T29_usecase_usecase_DupCheckError__self
R31_usecase_usecase_DupCheckService_dup_check --> T30_usecase_usecase_DupCheckOutput__self
R29_usecase_usecase_EmbeddingPort_embed --o T26_domain_domain_CodeFragment__self
R29_usecase_usecase_EmbeddingPort_embed --> T30_usecase_usecase_EmbeddingError__self
R34_usecase_usecase_FindSimilarService_find_similar --o T34_usecase_usecase_FindSimilarCommand__self
R34_usecase_usecase_FindSimilarService_find_similar --> T32_usecase_usecase_FindSimilarError__self
R34_usecase_usecase_FindSimilarService_find_similar --> T33_usecase_usecase_FindSimilarOutput__self
R37_usecase_usecase_MeasureQualityService_measure_quality --o T37_usecase_usecase_MeasureQualityCommand__self
R37_usecase_usecase_MeasureQualityService_measure_quality --> T35_usecase_usecase_MeasureQualityError__self
R37_usecase_usecase_MeasureQualityService_measure_quality --> T30_usecase_usecase_QualityMetrics__self
R33_usecase_usecase_SemanticIndexPort_insert --o T26_domain_domain_CodeFragment__self
R33_usecase_usecase_SemanticIndexPort_insert --> T34_usecase_usecase_SemanticIndexError__self
R33_usecase_usecase_SemanticIndexPort_search --o T18_domain_domain_TopK__self
R33_usecase_usecase_SemanticIndexPort_search --> T34_usecase_usecase_SemanticIndexError__self
R33_usecase_usecase_SemanticIndexPort_search --> T29_domain_domain_SimilarFragment__self
T37_usecase_usecase_FindSimilarInteractor__self -.impl.-> R34_usecase_usecase_FindSimilarService__self
T34_usecase_usecase_DupCheckInteractor__self -.impl.-> R31_usecase_usecase_DupCheckService__self
T36_usecase_usecase_BuildIndexInteractor__self -.impl.-> R33_usecase_usecase_BuildIndexService__self
T40_usecase_usecase_MeasureQualityInteractor__self -.impl.-> R37_usecase_usecase_MeasureQualityService__self
T46_infrastructure_infrastructure_FastEmbedAdapter_new --> T46_infrastructure_infrastructure_FastEmbedAdapter__self
T46_infrastructure_infrastructure_FastEmbedAdapter_new --> T30_usecase_usecase_EmbeddingError__self
T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new --> T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self
T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new --> T34_usecase_usecase_SemanticIndexError__self
F93_infrastructure_infrastructure_infrastructure__semantic_dup__extractor__extract_code_fragments --> T42_infrastructure_infrastructure_ExtractError__self
F93_infrastructure_infrastructure_infrastructure__semantic_dup__extractor__extract_code_fragments --> T26_domain_domain_CodeFragment__self
T46_infrastructure_infrastructure_FastEmbedAdapter__self -.impl.-> R29_usecase_usecase_EmbeddingPort__self
T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self -.impl.-> R33_usecase_usecase_SemanticIndexPort__self
class T26_domain_domain_CodeFragment_new method_node
class T26_domain_domain_CodeFragment__self value_object
class T30_domain_domain_SemanticDupError_InvalidScore variant_node
class T30_domain_domain_SemanticDupError_InvalidTopK variant_node
class T30_domain_domain_SemanticDupError_InvalidThreshold variant_node
class T30_domain_domain_SemanticDupError_EmptyContent variant_node
class T30_domain_domain_SemanticDupError__self error_type
class T29_domain_domain_SimilarFragment__self value_object
class T29_domain_domain_SimilarityScore_new method_node
class T29_domain_domain_SimilarityScore_value method_node
class T29_domain_domain_SimilarityScore__self value_object
class T33_domain_domain_SimilarityThreshold_new method_node
class T33_domain_domain_SimilarityThreshold_value method_node
class T33_domain_domain_SimilarityThreshold__self value_object
class T18_domain_domain_TopK_new method_node
class T18_domain_domain_TopK_value method_node
class T18_domain_domain_TopK__self value_object
class T33_usecase_usecase_BuildIndexCommand__self command
class T31_usecase_usecase_BuildIndexError_Embedding variant_node
class T31_usecase_usecase_BuildIndexError_Index variant_node
class T31_usecase_usecase_BuildIndexError_Io variant_node
class T31_usecase_usecase_BuildIndexError__self error_type
class T36_usecase_usecase_BuildIndexInteractor_new method_node
class T36_usecase_usecase_BuildIndexInteractor__self interactor
class T32_usecase_usecase_BuildIndexOutput__self value_object
class T31_usecase_usecase_DupCheckCommand__self query
class T29_usecase_usecase_DupCheckError_Embedding variant_node
class T29_usecase_usecase_DupCheckError_Index variant_node
class T29_usecase_usecase_DupCheckError__self error_type
class T34_usecase_usecase_DupCheckInteractor_new method_node
class T34_usecase_usecase_DupCheckInteractor__self interactor
class T30_usecase_usecase_DupCheckOutput__self value_object
class T31_usecase_usecase_DupCheckWarning__self value_object
class T30_usecase_usecase_EmbeddingError_ModelLoadFailed variant_node
class T30_usecase_usecase_EmbeddingError_InferenceFailed variant_node
class T30_usecase_usecase_EmbeddingError__self error_type
class T34_usecase_usecase_FindSimilarCommand__self query
class T32_usecase_usecase_FindSimilarError_Embedding variant_node
class T32_usecase_usecase_FindSimilarError_Index variant_node
class T32_usecase_usecase_FindSimilarError__self error_type
class T37_usecase_usecase_FindSimilarInteractor_new method_node
class T37_usecase_usecase_FindSimilarInteractor__self interactor
class T33_usecase_usecase_FindSimilarOutput__self value_object
class T37_usecase_usecase_MeasureQualityCommand__self command
class T35_usecase_usecase_MeasureQualityError_Embedding variant_node
class T35_usecase_usecase_MeasureQualityError_Index variant_node
class T35_usecase_usecase_MeasureQualityError_Io variant_node
class T35_usecase_usecase_MeasureQualityError__self error_type
class T40_usecase_usecase_MeasureQualityInteractor_new method_node
class T40_usecase_usecase_MeasureQualityInteractor__self interactor
class T30_usecase_usecase_QualityMetrics__self value_object
class T34_usecase_usecase_SemanticIndexError_OpenFailed variant_node
class T34_usecase_usecase_SemanticIndexError_InsertFailed variant_node
class T34_usecase_usecase_SemanticIndexError_SearchFailed variant_node
class T34_usecase_usecase_SemanticIndexError__self error_type
class R33_usecase_usecase_BuildIndexService_build_index method_node
class R33_usecase_usecase_BuildIndexService__self app_service
class R31_usecase_usecase_DupCheckService_dup_check method_node
class R31_usecase_usecase_DupCheckService__self app_service
class R29_usecase_usecase_EmbeddingPort_embed method_node
class R29_usecase_usecase_EmbeddingPort__self secondary_port
class R34_usecase_usecase_FindSimilarService_find_similar method_node
class R34_usecase_usecase_FindSimilarService__self app_service
class R37_usecase_usecase_MeasureQualityService_measure_quality method_node
class R37_usecase_usecase_MeasureQualityService__self app_service
class R33_usecase_usecase_SemanticIndexPort_insert method_node
class R33_usecase_usecase_SemanticIndexPort_search method_node
class R33_usecase_usecase_SemanticIndexPort__self secondary_port
class T42_infrastructure_infrastructure_ExtractError_Io variant_node
class T42_infrastructure_infrastructure_ExtractError__self error_type
class T46_infrastructure_infrastructure_FastEmbedAdapter_new method_node
class T46_infrastructure_infrastructure_FastEmbedAdapter__self secondary_adapter
class T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter_new method_node
class T57_infrastructure_infrastructure_LanceDbSemanticIndexAdapter__self secondary_adapter
class F93_infrastructure_infrastructure_infrastructure__semantic_dup__extractor__extract_code_fragments free_function
class F93_infrastructure_infrastructure_infrastructure__semantic_dup__extractor__extract_code_fragments function_node
```
