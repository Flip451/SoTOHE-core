<!-- Generated from infrastructure::git_cli cluster TypeGraph — DO NOT EDIT DIRECTLY -->
# infrastructure::git_cli Type Graph

Types: 3 in cluster, 1 intra-cluster edges

```mermaid
flowchart LR
    classDef structNode fill:#f3e5f5,stroke:#7b1fa2
    classDef enumNode fill:#e1f5fe,stroke:#0288d1
    classDef ghostNode fill:#f5f5f5,stroke:#9e9e9e,color:#757575
    classDef traitNode fill:#e8f5e9,stroke:#388e3c

    GitError{{GitError}}:::enumNode
    SystemGitRepo[SystemGitRepo]:::structNode
    TrackBranchRecord[TrackBranchRecord]:::structNode
    _trait_GitRepository([GitRepository]):::traitNode

    SystemGitRepo -.->|impl| _trait_GitRepository
```
