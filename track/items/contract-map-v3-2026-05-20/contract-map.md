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
  subgraph T34_domain_domain_CatalogueLoaderError["tddd::catalogue_ports::CatalogueLoaderError"]
    direction TB
    T34_domain_domain_CatalogueLoaderError__self[CatalogueLoaderError]
    T34_domain_domain_CatalogueLoaderError_CatalogueNotFound[CatalogueNotFound]
    T34_domain_domain_CatalogueLoaderError_LayerDiscoveryFailed[LayerDiscoveryFailed]
    T34_domain_domain_CatalogueLoaderError_DecodeFailed[DecodeFailed]
    T34_domain_domain_CatalogueLoaderError_SymlinkRejected[SymlinkRejected]
    T34_domain_domain_CatalogueLoaderError_IoError[IoError]
    T34_domain_domain_CatalogueLoaderError_TopologicalSortFailed[TopologicalSortFailed]
  end
  subgraph T32_domain_domain_ContractMapContent["tddd::contract_map_content::ContractMapContent"]
    direction TB
    T32_domain_domain_ContractMapContent__self[ContractMapContent]
    T32_domain_domain_ContractMapContent_new([new])
    T32_domain_domain_ContractMapContent_into_string([into_string])
  end
  subgraph T38_domain_domain_ContractMapRenderOptions["tddd::contract_map_options::ContractMapRenderOptions"]
    direction TB
    T38_domain_domain_ContractMapRenderOptions__self[ContractMapRenderOptions]
    T38_domain_domain_ContractMapRenderOptions_empty([empty])
  end
  subgraph T38_domain_domain_ContractMapRendererError["tddd::ContractMapRendererError"]
    direction TB
    T38_domain_domain_ContractMapRendererError__self[ContractMapRendererError]
    T38_domain_domain_ContractMapRendererError_StyleConfigNotFound[StyleConfigNotFound]
    T38_domain_domain_ContractMapRendererError_StyleConfigInvalid[StyleConfigInvalid]
    T38_domain_domain_ContractMapRendererError_RenderFailed[RenderFailed]
  end
  subgraph T36_domain_domain_ContractMapWriterError["tddd::catalogue_ports::ContractMapWriterError"]
    direction TB
    T36_domain_domain_ContractMapWriterError__self[ContractMapWriterError]
    T36_domain_domain_ContractMapWriterError_IoError[IoError]
    T36_domain_domain_ContractMapWriterError_SymlinkRejected[SymlinkRejected]
    T36_domain_domain_ContractMapWriterError_TrackNotFound[TrackNotFound]
  end
  subgraph R29_domain_domain_CatalogueLoader["tddd::catalogue_ports::CatalogueLoader"]
    direction TB
    R29_domain_domain_CatalogueLoader__self[CatalogueLoader]
    R29_domain_domain_CatalogueLoader_load_all([load_all])
  end
  subgraph R33_domain_domain_ContractMapRenderer["tddd::ContractMapRenderer"]
    direction TB
    R33_domain_domain_ContractMapRenderer__self[ContractMapRenderer]
    R33_domain_domain_ContractMapRenderer_render([render])
  end
  subgraph R31_domain_domain_ContractMapWriter["tddd::catalogue_ports::ContractMapWriter"]
    direction TB
    R31_domain_domain_ContractMapWriter__self[ContractMapWriter]
    R31_domain_domain_ContractMapWriter_write([write])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_contract_map_workflow["usecase::contract_map_workflow"]
    direction TB
  subgraph T40_usecase_usecase_RenderContractMapCommand["contract_map_workflow::RenderContractMapCommand"]
    direction TB
    T40_usecase_usecase_RenderContractMapCommand__self[RenderContractMapCommand]
  end
  subgraph T38_usecase_usecase_RenderContractMapError["contract_map_workflow::RenderContractMapError"]
    direction TB
    T38_usecase_usecase_RenderContractMapError__self[RenderContractMapError]
    T38_usecase_usecase_RenderContractMapError_CatalogueLoaderFailed[CatalogueLoaderFailed]
    T38_usecase_usecase_RenderContractMapError_ContractMapWriterFailed[ContractMapWriterFailed]
    T38_usecase_usecase_RenderContractMapError_EmptyCatalogue[EmptyCatalogue]
    T38_usecase_usecase_RenderContractMapError_LayerNotFound[LayerNotFound]
    T38_usecase_usecase_RenderContractMapError_InvalidTrackId[InvalidTrackId]
    T38_usecase_usecase_RenderContractMapError_RendererFailed[RendererFailed]
  end
  subgraph T43_usecase_usecase_RenderContractMapInteractor["contract_map_workflow::RenderContractMapInteractor"]
    direction TB
    T43_usecase_usecase_RenderContractMapInteractor__self[RenderContractMapInteractor]
    T43_usecase_usecase_RenderContractMapInteractor_new([new])
  end
  subgraph T39_usecase_usecase_RenderContractMapOutput["contract_map_workflow::RenderContractMapOutput"]
    direction TB
    T39_usecase_usecase_RenderContractMapOutput__self[RenderContractMapOutput]
  end
  subgraph R33_usecase_usecase_RenderContractMap["contract_map_workflow::RenderContractMap"]
    direction TB
    R33_usecase_usecase_RenderContractMap__self[RenderContractMap]
    R33_usecase_usecase_RenderContractMap_execute([execute])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_tddd["infrastructure::tddd"]
    direction TB
  subgraph T56_infrastructure_infrastructure_ContractMapRendererAdapter["tddd::contract_map_renderer_adapter::ContractMapRendererAdapter"]
    direction TB
    T56_infrastructure_infrastructure_ContractMapRendererAdapter__self[ContractMapRendererAdapter]
    T56_infrastructure_infrastructure_ContractMapRendererAdapter_new([new])
  end
  subgraph T47_infrastructure_infrastructure_FsCatalogueLoader["tddd::contract_map_adapter::FsCatalogueLoader"]
    direction TB
    T47_infrastructure_infrastructure_FsCatalogueLoader__self[FsCatalogueLoader]
    T47_infrastructure_infrastructure_FsCatalogueLoader_new([new])
  end
  subgraph T49_infrastructure_infrastructure_FsContractMapWriter["tddd::contract_map_adapter::FsContractMapWriter"]
    direction TB
    T49_infrastructure_infrastructure_FsContractMapWriter__self[FsContractMapWriter]
    T49_infrastructure_infrastructure_FsContractMapWriter_new([new])
  end
  end
end
T32_domain_domain_ContractMapContent_new --> T32_domain_domain_ContractMapContent__self
T38_domain_domain_ContractMapRenderOptions_empty --> T38_domain_domain_ContractMapRenderOptions__self
R29_domain_domain_CatalogueLoader_load_all --> T34_domain_domain_CatalogueLoaderError__self
R33_domain_domain_ContractMapRenderer_render --o T38_domain_domain_ContractMapRenderOptions__self
R33_domain_domain_ContractMapRenderer_render --> T32_domain_domain_ContractMapContent__self
R33_domain_domain_ContractMapRenderer_render --> T38_domain_domain_ContractMapRendererError__self
R31_domain_domain_ContractMapWriter_write --o T32_domain_domain_ContractMapContent__self
R31_domain_domain_ContractMapWriter_write --> T36_domain_domain_ContractMapWriterError__self
T38_usecase_usecase_RenderContractMapError_CatalogueLoaderFailed --o T34_domain_domain_CatalogueLoaderError__self
T38_usecase_usecase_RenderContractMapError_ContractMapWriterFailed --o T36_domain_domain_ContractMapWriterError__self
T38_usecase_usecase_RenderContractMapError_RendererFailed --o T38_domain_domain_ContractMapRendererError__self
T43_usecase_usecase_RenderContractMapInteractor_new --> T43_usecase_usecase_RenderContractMapInteractor__self
R33_usecase_usecase_RenderContractMap_execute --o T40_usecase_usecase_RenderContractMapCommand__self
R33_usecase_usecase_RenderContractMap_execute --> T38_usecase_usecase_RenderContractMapError__self
R33_usecase_usecase_RenderContractMap_execute --> T39_usecase_usecase_RenderContractMapOutput__self
T43_usecase_usecase_RenderContractMapInteractor__self -.impl.-> R33_usecase_usecase_RenderContractMap__self
T56_infrastructure_infrastructure_ContractMapRendererAdapter_new --> T56_infrastructure_infrastructure_ContractMapRendererAdapter__self
T47_infrastructure_infrastructure_FsCatalogueLoader_new --> T47_infrastructure_infrastructure_FsCatalogueLoader__self
T49_infrastructure_infrastructure_FsContractMapWriter_new --> T49_infrastructure_infrastructure_FsContractMapWriter__self
T56_infrastructure_infrastructure_ContractMapRendererAdapter__self -.impl.-> R33_domain_domain_ContractMapRenderer__self
T47_infrastructure_infrastructure_FsCatalogueLoader__self -.impl.-> R29_domain_domain_CatalogueLoader__self
T49_infrastructure_infrastructure_FsContractMapWriter__self -.impl.-> R31_domain_domain_ContractMapWriter__self
class T34_domain_domain_CatalogueLoaderError_CatalogueNotFound variant_node
class T34_domain_domain_CatalogueLoaderError_LayerDiscoveryFailed variant_node
class T34_domain_domain_CatalogueLoaderError_DecodeFailed variant_node
class T34_domain_domain_CatalogueLoaderError_SymlinkRejected variant_node
class T34_domain_domain_CatalogueLoaderError_IoError variant_node
class T34_domain_domain_CatalogueLoaderError_TopologicalSortFailed variant_node
class T34_domain_domain_CatalogueLoaderError__self error_type
class T32_domain_domain_ContractMapContent_new method_node
class T32_domain_domain_ContractMapContent_into_string method_node
class T32_domain_domain_ContractMapContent__self value_object
class T38_domain_domain_ContractMapRenderOptions_empty method_node
class T38_domain_domain_ContractMapRenderOptions__self value_object
class T38_domain_domain_ContractMapRendererError_StyleConfigNotFound variant_node
class T38_domain_domain_ContractMapRendererError_StyleConfigInvalid variant_node
class T38_domain_domain_ContractMapRendererError_RenderFailed variant_node
class T38_domain_domain_ContractMapRendererError__self error_type
class T36_domain_domain_ContractMapWriterError_IoError variant_node
class T36_domain_domain_ContractMapWriterError_SymlinkRejected variant_node
class T36_domain_domain_ContractMapWriterError_TrackNotFound variant_node
class T36_domain_domain_ContractMapWriterError__self error_type
class R29_domain_domain_CatalogueLoader_load_all method_node
class R29_domain_domain_CatalogueLoader__self secondary_port
class R33_domain_domain_ContractMapRenderer_render method_node
class R33_domain_domain_ContractMapRenderer__self secondary_port
class R31_domain_domain_ContractMapWriter_write method_node
class R31_domain_domain_ContractMapWriter__self secondary_port
class T40_usecase_usecase_RenderContractMapCommand__self command
class T38_usecase_usecase_RenderContractMapError_CatalogueLoaderFailed variant_node
class T38_usecase_usecase_RenderContractMapError_ContractMapWriterFailed variant_node
class T38_usecase_usecase_RenderContractMapError_EmptyCatalogue variant_node
class T38_usecase_usecase_RenderContractMapError_LayerNotFound variant_node
class T38_usecase_usecase_RenderContractMapError_InvalidTrackId variant_node
class T38_usecase_usecase_RenderContractMapError_RendererFailed variant_node
class T38_usecase_usecase_RenderContractMapError__self error_type
class T43_usecase_usecase_RenderContractMapInteractor_new method_node
class T43_usecase_usecase_RenderContractMapInteractor__self interactor
class T39_usecase_usecase_RenderContractMapOutput__self dto
class R33_usecase_usecase_RenderContractMap_execute method_node
class R33_usecase_usecase_RenderContractMap__self app_service
class T56_infrastructure_infrastructure_ContractMapRendererAdapter_new method_node
class T56_infrastructure_infrastructure_ContractMapRendererAdapter__self secondary_adapter
class T47_infrastructure_infrastructure_FsCatalogueLoader_new method_node
class T47_infrastructure_infrastructure_FsCatalogueLoader__self secondary_adapter
class T49_infrastructure_infrastructure_FsContractMapWriter_new method_node
class T49_infrastructure_infrastructure_FsContractMapWriter__self secondary_adapter
```
