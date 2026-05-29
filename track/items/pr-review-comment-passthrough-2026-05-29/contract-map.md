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
  subgraph usecase_usecase_module_pr_review["usecase::pr_review"]
    direction TB
  subgraph T29_usecase_usecase_PrReviewError["pr_review::PrReviewError"]
    direction TB
    T29_usecase_usecase_PrReviewError__self[PrReviewError]
    T29_usecase_usecase_PrReviewError_UnsupportedProvider[UnsupportedProvider]
  end
  subgraph T31_usecase_usecase_PrReviewFinding["pr_review::PrReviewFinding"]
    direction TB
    T31_usecase_usecase_PrReviewFinding__self[PrReviewFinding]
  end
  subgraph T30_usecase_usecase_PrReviewResult["pr_review::PrReviewResult"]
    direction TB
    T30_usecase_usecase_PrReviewResult__self[PrReviewResult]
  end
  F56_usecase_usecase_usecase__pr_review__parse_paginated_json[[parse_paginated_json]]
  F49_usecase_usecase_usecase__pr_review__sanitize_text[[sanitize_text]]
  F62_usecase_usecase_usecase__pr_review__validate_reviewer_provider[[validate_reviewer_provider]]
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
end
T30_usecase_usecase_PrReviewResult__self --o|findings| T31_usecase_usecase_PrReviewFinding__self
F62_usecase_usecase_usecase__pr_review__validate_reviewer_provider --> T29_usecase_usecase_PrReviewError__self
class T29_usecase_usecase_PrReviewError_UnsupportedProvider variant_node
class T29_usecase_usecase_PrReviewError__self error_type
class T31_usecase_usecase_PrReviewFinding__self dto
class T30_usecase_usecase_PrReviewResult__self dto
class F56_usecase_usecase_usecase__pr_review__parse_paginated_json free_function
class F56_usecase_usecase_usecase__pr_review__parse_paginated_json function_node
class F49_usecase_usecase_usecase__pr_review__sanitize_text free_function
class F49_usecase_usecase_usecase__pr_review__sanitize_text function_node
class F62_usecase_usecase_usecase__pr_review__validate_reviewer_provider free_function
class F62_usecase_usecase_usecase__pr_review__validate_reviewer_provider function_node
```
