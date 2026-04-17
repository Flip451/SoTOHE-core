<!-- Generated from usecase::pr_review cluster TypeGraph — DO NOT EDIT DIRECTLY -->
# usecase::pr_review Type Graph

Types: 3 in cluster, 1 intra-cluster edges

```mermaid
flowchart LR
    classDef structNode fill:#f3e5f5,stroke:#7b1fa2
    classDef enumNode fill:#e1f5fe,stroke:#0288d1
    classDef ghostNode fill:#f5f5f5,stroke:#9e9e9e,color:#757575

    PrReviewError{{PrReviewError}}:::enumNode
    PrReviewFinding[PrReviewFinding]:::structNode
    PrReviewResult[PrReviewResult]:::structNode

    PrReviewResult ---|findings| PrReviewFinding
```
