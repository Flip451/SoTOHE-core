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
        L6_domain_EnumVariantDeclaration(EnumVariantDeclaration)
        L6_domain_MemberDeclaration{{MemberDeclaration}}
        L6_domain_render__contract__map[render_contract_map]:::free_function
    end
    subgraph usecase [usecase]
    end
    subgraph infrastructure [infrastructure]
        L14_infrastructure_build__type__graph[build_type_graph]:::free_function
        L14_infrastructure_render__type__graph__flat[render_type_graph_flat]:::free_function
        L14_infrastructure_render__type__graph__clustered[render_type_graph_clustered]:::free_function
    end
```
