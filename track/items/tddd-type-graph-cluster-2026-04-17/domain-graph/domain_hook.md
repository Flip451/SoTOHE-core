<!-- Generated from domain::hook cluster TypeGraph — DO NOT EDIT DIRECTLY -->
# domain::hook Type Graph

Types: 5 in cluster, 0 intra-cluster edges, 1 cross-cluster references

```mermaid
flowchart LR
    classDef structNode fill:#f3e5f5,stroke:#7b1fa2
    classDef enumNode fill:#e1f5fe,stroke:#0288d1
    classDef ghostNode fill:#f5f5f5,stroke:#9e9e9e,color:#757575

    HookContext[HookContext]:::structNode
    HookError{{HookError}}:::enumNode
    HookInput[HookInput]:::structNode
    HookName{{HookName}}:::enumNode
    HookVerdict[HookVerdict]:::structNode
    _xref_domain__decision_Decision["→ domain::decision::Decision"]:::ghostNode

    HookVerdict ---|decision| _xref_domain__decision_Decision
```
