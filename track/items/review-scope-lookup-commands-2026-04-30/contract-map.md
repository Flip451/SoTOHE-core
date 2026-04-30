<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
flowchart LR
    classDef secondary_adapter fill:#fafafa,stroke:#999,stroke-dasharray: 4 4
    classDef command fill:#e3f2fd,stroke:#1976d2
    classDef query fill:#f3e5f5,stroke:#8e24aa
    classDef factory fill:#fff8e1,stroke:#f9a825
    classDef free_function fill:#f1f8e9,stroke:#558b2f
    classDef domain_service fill:#fce4ec,stroke:#c62828
    subgraph domain [domain]
        L6_domain_ReviewScopeConfig[ReviewScopeConfig]:::domain_service
        L6_domain_FilePath(FilePath)
        L6_domain_ScopeName{{ScopeName}}
        L6_domain_MainScopeName(MainScopeName)
    end
    subgraph usecase [usecase]
        L7_usecase_ScopeQueryService[/ScopeQueryService\]
        L7_usecase_ScopeQueryInteractor[\ScopeQueryInteractor/]
        L7_usecase_ScopeQueryError>ScopeQueryError]
        L7_usecase_ScopeClassification{{ScopeClassification}}
        L7_usecase_PathClassification(PathClassification)
        L7_usecase_DiffGetter[[DiffGetter]]
    end
    subgraph infrastructure [infrastructure]
        L14_infrastructure_GitDiffGetter[GitDiffGetter]:::secondary_adapter
    end
    L14_infrastructure_GitDiffGetter -->|"list_diff_files"| L6_domain_FilePath
    L14_infrastructure_GitDiffGetter -.impl.-> L7_usecase_DiffGetter
    L6_domain_ReviewScopeConfig -->|"all_scope_names"| L6_domain_ScopeName
    L6_domain_ReviewScopeConfig -->|"classify"| L6_domain_FilePath
    L6_domain_ReviewScopeConfig -->|"classify"| L6_domain_ScopeName
    L6_domain_ReviewScopeConfig -->|"classify(files)"| L6_domain_FilePath
    L6_domain_ReviewScopeConfig -->|"contains_scope(scope)"| L6_domain_ScopeName
    L7_usecase_DiffGetter -->|"list_diff_files"| L6_domain_FilePath
    L7_usecase_ScopeQueryInteractor -.impl.-> L7_usecase_ScopeQueryService
    L7_usecase_ScopeQueryService -->|"classify"| L7_usecase_PathClassification
    L7_usecase_ScopeQueryService -->|"classify"| L7_usecase_ScopeQueryError
    L7_usecase_ScopeQueryService -->|"classify(paths)"| L6_domain_FilePath
    L7_usecase_ScopeQueryService -->|"files"| L6_domain_FilePath
    L7_usecase_ScopeQueryService -->|"files"| L7_usecase_ScopeQueryError
    L7_usecase_ScopeQueryService -->|"files(scope)"| L6_domain_ScopeName
```
