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
  subgraph domain_domain_module_review_v2["domain::review_v2"]
    direction TB
  subgraph R30_domain_domain_CommitHashReader["review_v2::CommitHashReader"]
    direction TB
    R30_domain_domain_CommitHashReader__self[CommitHashReader]
    R30_domain_domain_CommitHashReader_read([read])
  end
  subgraph R30_domain_domain_CommitHashWriter["review_v2::CommitHashWriter"]
    direction TB
    R30_domain_domain_CommitHashWriter__self[CommitHashWriter]
    R30_domain_domain_CommitHashWriter_write([write])
    R30_domain_domain_CommitHashWriter_clear([clear])
  end
  subgraph R30_domain_domain_ReviewExistsPort["review_v2::ReviewExistsPort"]
    direction TB
    R30_domain_domain_ReviewExistsPort__self[ReviewExistsPort]
    R30_domain_domain_ReviewExistsPort_review_json_exists([review_json_exists])
  end
  subgraph R26_domain_domain_ReviewReader["review_v2::ReviewReader"]
    direction TB
    R26_domain_domain_ReviewReader__self[ReviewReader]
    R26_domain_domain_ReviewReader_read_latest_finals([read_latest_finals])
    R26_domain_domain_ReviewReader_read_all_rounds([read_all_rounds])
  end
  subgraph R26_domain_domain_ReviewWriter["review_v2::ReviewWriter"]
    direction TB
    R26_domain_domain_ReviewWriter__self[ReviewWriter]
    R26_domain_domain_ReviewWriter_write_verdict([write_verdict])
    R26_domain_domain_ReviewWriter_write_fast_verdict([write_fast_verdict])
    R26_domain_domain_ReviewWriter_init([init])
    R26_domain_domain_ReviewWriter_reset([reset])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_review_v2["usecase::review_v2"]
    direction TB
  subgraph R26_usecase_usecase_DiffGetter["review_v2::ports::DiffGetter"]
    direction TB
    R26_usecase_usecase_DiffGetter__self[DiffGetter]
    R26_usecase_usecase_DiffGetter_list_diff_files([list_diff_files])
  end
  subgraph R28_usecase_usecase_ReviewHasher["review_v2::ports::ReviewHasher"]
    direction TB
    R28_usecase_usecase_ReviewHasher__self[ReviewHasher]
    R28_usecase_usecase_ReviewHasher_calc([calc])
  end
  subgraph R24_usecase_usecase_Reviewer["review_v2::ports::Reviewer"]
    direction TB
    R24_usecase_usecase_Reviewer__self[Reviewer]
    R24_usecase_usecase_Reviewer_review([review])
    R24_usecase_usecase_Reviewer_fast_review([fast_review])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  subgraph T44_infrastructure_infrastructure_ClaudeReviewer["review_v2::claude_reviewer::ClaudeReviewer"]
    direction TB
    T44_infrastructure_infrastructure_ClaudeReviewer__self[ClaudeReviewer]
  end
  subgraph T43_infrastructure_infrastructure_CodexReviewer["review_v2::codex_reviewer::CodexReviewer"]
    direction TB
    T43_infrastructure_infrastructure_CodexReviewer__self[CodexReviewer]
  end
  subgraph T47_infrastructure_infrastructure_FsCommitHashStore["review_v2::persistence::commit_hash_store::FsCommitHashStore"]
    direction TB
    T47_infrastructure_infrastructure_FsCommitHashStore__self[FsCommitHashStore]
  end
  subgraph T43_infrastructure_infrastructure_FsReviewStore["review_v2::persistence::review_store::FsReviewStore"]
    direction TB
    T43_infrastructure_infrastructure_FsReviewStore__self[FsReviewStore]
  end
  subgraph T43_infrastructure_infrastructure_GitDiffGetter["review_v2::diff_getter::GitDiffGetter"]
    direction TB
    T43_infrastructure_infrastructure_GitDiffGetter__self[GitDiffGetter]
  end
  subgraph T50_infrastructure_infrastructure_ScopeConfigLoadError["review_v2::scope_config_loader::ScopeConfigLoadError"]
    direction TB
    T50_infrastructure_infrastructure_ScopeConfigLoadError__self[ScopeConfigLoadError]
    T50_infrastructure_infrastructure_ScopeConfigLoadError_Io[Io]
    T50_infrastructure_infrastructure_ScopeConfigLoadError_Parse[Parse]
    T50_infrastructure_infrastructure_ScopeConfigLoadError_InvalidField[InvalidField]
    T50_infrastructure_infrastructure_ScopeConfigLoadError_Config[Config]
  end
  subgraph T48_infrastructure_infrastructure_SystemReviewHasher["review_v2::hasher::SystemReviewHasher"]
    direction TB
    T48_infrastructure_infrastructure_SystemReviewHasher__self[SystemReviewHasher]
  end
  F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config[[load_v2_scope_config]]
  end
end
F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config --> T50_infrastructure_infrastructure_ScopeConfigLoadError__self
T43_infrastructure_infrastructure_CodexReviewer__self -.impl.-> R24_usecase_usecase_Reviewer__self
T44_infrastructure_infrastructure_ClaudeReviewer__self -.impl.-> R24_usecase_usecase_Reviewer__self
T43_infrastructure_infrastructure_GitDiffGetter__self -.impl.-> R26_usecase_usecase_DiffGetter__self
T48_infrastructure_infrastructure_SystemReviewHasher__self -.impl.-> R28_usecase_usecase_ReviewHasher__self
T43_infrastructure_infrastructure_FsReviewStore__self -.impl.-> R26_domain_domain_ReviewReader__self
T43_infrastructure_infrastructure_FsReviewStore__self -.impl.-> R26_domain_domain_ReviewWriter__self
T43_infrastructure_infrastructure_FsReviewStore__self -.impl.-> R30_domain_domain_ReviewExistsPort__self
T47_infrastructure_infrastructure_FsCommitHashStore__self -.impl.-> R30_domain_domain_CommitHashReader__self
T47_infrastructure_infrastructure_FsCommitHashStore__self -.impl.-> R30_domain_domain_CommitHashWriter__self
class R30_domain_domain_CommitHashReader_read method_node
class R30_domain_domain_CommitHashReader__self secondary_port
class R30_domain_domain_CommitHashWriter_write method_node
class R30_domain_domain_CommitHashWriter_clear method_node
class R30_domain_domain_CommitHashWriter__self secondary_port
class R30_domain_domain_ReviewExistsPort_review_json_exists method_node
class R30_domain_domain_ReviewExistsPort__self secondary_port
class R26_domain_domain_ReviewReader_read_latest_finals method_node
class R26_domain_domain_ReviewReader_read_all_rounds method_node
class R26_domain_domain_ReviewReader__self secondary_port
class R26_domain_domain_ReviewWriter_write_verdict method_node
class R26_domain_domain_ReviewWriter_write_fast_verdict method_node
class R26_domain_domain_ReviewWriter_init method_node
class R26_domain_domain_ReviewWriter_reset method_node
class R26_domain_domain_ReviewWriter__self secondary_port
class R26_usecase_usecase_DiffGetter_list_diff_files method_node
class R26_usecase_usecase_DiffGetter__self secondary_port
class R28_usecase_usecase_ReviewHasher_calc method_node
class R28_usecase_usecase_ReviewHasher__self secondary_port
class R24_usecase_usecase_Reviewer_review method_node
class R24_usecase_usecase_Reviewer_fast_review method_node
class R24_usecase_usecase_Reviewer__self secondary_port
class T44_infrastructure_infrastructure_ClaudeReviewer__self secondary_adapter
class T43_infrastructure_infrastructure_CodexReviewer__self secondary_adapter
class T47_infrastructure_infrastructure_FsCommitHashStore__self secondary_adapter
class T43_infrastructure_infrastructure_FsReviewStore__self secondary_adapter
class T43_infrastructure_infrastructure_GitDiffGetter__self secondary_adapter
class T50_infrastructure_infrastructure_ScopeConfigLoadError_Io variant_node
class T50_infrastructure_infrastructure_ScopeConfigLoadError_Parse variant_node
class T50_infrastructure_infrastructure_ScopeConfigLoadError_InvalidField variant_node
class T50_infrastructure_infrastructure_ScopeConfigLoadError_Config variant_node
class T50_infrastructure_infrastructure_ScopeConfigLoadError__self error_type
class T48_infrastructure_infrastructure_SystemReviewHasher__self secondary_adapter
class F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config free_function
class F98_infrastructure_infrastructure_infrastructure__review_v2__scope_config_loader__load_v2_scope_config function_node
```
