```mermaid
flowchart LR
    classDef secondary_adapter fill:#fafafa,stroke:#999,stroke-dasharray: 4 4
    classDef command fill:#e3f2fd,stroke:#1976d2
    classDef query fill:#f3e5f5,stroke:#8e24aa
    classDef factory fill:#fff8e1,stroke:#f9a825
    subgraph domain [domain]
        domain_LayerId(LayerId)
        domain_ContractMapContent(ContractMapContent)
        domain_ContractMapRenderOptions(ContractMapRenderOptions)
        domain_CatalogueLoader[[CatalogueLoader]]
        domain_ContractMapWriter[[ContractMapWriter]]
        domain_CatalogueLoaderError>CatalogueLoaderError]
        domain_ContractMapWriterError>ContractMapWriterError]
        domain_TypeCatalogueDocument(TypeCatalogueDocument)
        domain_ValidationError>ValidationError]
        domain_TrackId(TrackId)
        domain_TaskId(TaskId)
        domain_CommitHash(CommitHash)
        domain_TrackBranch(TrackBranch)
        domain_NonEmptyString(NonEmptyString)
        domain_ReviewGroupName(ReviewGroupName)
    end
    subgraph usecase [usecase]
        usecase_RenderContractMap[/RenderContractMap\]
        usecase_RenderContractMapCommand[RenderContractMapCommand]:::command
        usecase_RenderContractMapOutput[RenderContractMapOutput]
        usecase_RenderContractMapError>RenderContractMapError]
        usecase_RenderContractMapInteractor[\RenderContractMapInteractor/]
    end
    subgraph infrastructure [infrastructure]
        infrastructure_FsCatalogueLoader[FsCatalogueLoader]:::secondary_adapter
        infrastructure_FsContractMapWriter[FsContractMapWriter]:::secondary_adapter
        infrastructure_LoadAllCataloguesError>LoadAllCataloguesError]
    end
    domain_CatalogueLoader -->|"load_all"| domain_CatalogueLoaderError
    domain_CatalogueLoader -->|"load_all"| domain_LayerId
    domain_CatalogueLoader -->|"load_all"| domain_TypeCatalogueDocument
    domain_CatalogueLoader -->|"load_all(track_id)"| domain_TrackId
    domain_ContractMapWriter -->|"write"| domain_ContractMapWriterError
    domain_ContractMapWriter -->|"write(content)"| domain_ContractMapContent
    domain_ContractMapWriter -->|"write(track_id)"| domain_TrackId
    infrastructure_FsCatalogueLoader -.impl.-> domain_CatalogueLoader
    infrastructure_FsContractMapWriter -.impl.-> domain_ContractMapWriter
    usecase_RenderContractMap -->|"execute"| usecase_RenderContractMapError
    usecase_RenderContractMap -->|"execute"| usecase_RenderContractMapOutput
    usecase_RenderContractMap -->|"execute(cmd)"| usecase_RenderContractMapCommand
```
