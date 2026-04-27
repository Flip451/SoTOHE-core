```mermaid
flowchart LR
    classDef secondary_adapter fill:#fafafa,stroke:#999,stroke-dasharray: 4 4
    classDef command fill:#e3f2fd,stroke:#1976d2
    classDef query fill:#f3e5f5,stroke:#8e24aa
    classDef factory fill:#fff8e1,stroke:#f9a825
    classDef free_function fill:#f1f8e9,stroke:#558b2f
    subgraph domain [domain]
        L6_domain_VerifyOutcome(VerifyOutcome)
        L6_domain_VerifyFinding(VerifyFinding)
        L6_domain_Severity{{Severity}}
    end
    subgraph usecase [usecase]
    end
    subgraph infrastructure [infrastructure]
        L14_infrastructure_TdddLayerBinding(TdddLayerBinding)
        L14_infrastructure_PlanArtifactRefsError>PlanArtifactRefsError]
    end
```
