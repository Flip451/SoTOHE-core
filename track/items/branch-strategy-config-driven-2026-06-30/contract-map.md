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
  subgraph domain_domain_module_branch_strategy["domain::branch_strategy"]
    direction TB
  subgraph T36_domain_domain_BranchStrategySnapshot["branch_strategy::BranchStrategySnapshot"]
    direction TB
    T36_domain_domain_BranchStrategySnapshot__self[BranchStrategySnapshot]
    T36_domain_domain_BranchStrategySnapshot_new([new])
    T36_domain_domain_BranchStrategySnapshot_base_branch([base_branch])
    T36_domain_domain_BranchStrategySnapshot_merge_target([merge_target])
    T36_domain_domain_BranchStrategySnapshot_merge_method([merge_method])
  end
  subgraph T25_domain_domain_MergeMethod["branch_strategy::MergeMethod"]
    direction TB
    T25_domain_domain_MergeMethod__self[MergeMethod]
    T25_domain_domain_MergeMethod_Squash[Squash]
    T25_domain_domain_MergeMethod_Merge[Merge]
    T25_domain_domain_MergeMethod_Rebase[Rebase]
  end
  end
  subgraph domain_domain_module_track["domain::track"]
    direction TB
  subgraph T27_domain_domain_TrackMetadata["track::TrackMetadata"]
    direction TB
    T27_domain_domain_TrackMetadata__self[TrackMetadata]
    T27_domain_domain_TrackMetadata_new([new])
    T27_domain_domain_TrackMetadata_with_branch([with_branch])
    T27_domain_domain_TrackMetadata_id([id])
    T27_domain_domain_TrackMetadata_branch([branch])
    T27_domain_domain_TrackMetadata_is_activated([is_activated])
    T27_domain_domain_TrackMetadata_set_branch([set_branch])
    T27_domain_domain_TrackMetadata_title([title])
    T27_domain_domain_TrackMetadata_status_override([status_override])
    T27_domain_domain_TrackMetadata_set_status_override([set_status_override])
    T27_domain_domain_TrackMetadata_branch_strategy_snapshot([branch_strategy_snapshot])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_branch_strategy["usecase::branch_strategy"]
    direction TB
  subgraph R34_usecase_usecase_BranchStrategyPort["branch_strategy::BranchStrategyPort"]
    direction TB
    R34_usecase_usecase_BranchStrategyPort__self[BranchStrategyPort]
    R34_usecase_usecase_BranchStrategyPort_base_branch([base_branch])
    R34_usecase_usecase_BranchStrategyPort_merge_target([merge_target])
    R34_usecase_usecase_BranchStrategyPort_merge_method([merge_method])
    R34_usecase_usecase_BranchStrategyPort_track_prefix([track_prefix])
  end
  end
  subgraph usecase_usecase_module_dry_driver["usecase::dry_driver"]
    direction TB
  subgraph T39_usecase_usecase_DryCheckApprovedOutcome["dry_driver::DryCheckApprovedOutcome"]
    direction TB
    T39_usecase_usecase_DryCheckApprovedOutcome__self[DryCheckApprovedOutcome]
    T39_usecase_usecase_DryCheckApprovedOutcome_Approved[Approved]
    T39_usecase_usecase_DryCheckApprovedOutcome_Blocked[Blocked]
    T39_usecase_usecase_DryCheckApprovedOutcome_Failure[Failure]
  end
  subgraph T35_usecase_usecase_DryDriverInteractor["dry_driver::DryDriverInteractor"]
    direction TB
    T35_usecase_usecase_DryDriverInteractor__self[DryDriverInteractor]
  end
  subgraph T38_usecase_usecase_DryWriteFindingSummary["dry_driver::DryWriteFindingSummary"]
    direction TB
    T38_usecase_usecase_DryWriteFindingSummary__self[DryWriteFindingSummary]
  end
  subgraph T31_usecase_usecase_DryWriteOutcome["dry_driver::DryWriteOutcome"]
    direction TB
    T31_usecase_usecase_DryWriteOutcome__self[DryWriteOutcome]
    T31_usecase_usecase_DryWriteOutcome_Success[Success]
    T31_usecase_usecase_DryWriteOutcome_Failure[Failure]
  end
  subgraph R29_usecase_usecase_DryDriverPort["dry_driver::DryDriverPort"]
    direction TB
    R29_usecase_usecase_DryDriverPort__self[DryDriverPort]
    R29_usecase_usecase_DryDriverPort_dry_write([dry_write])
    R29_usecase_usecase_DryDriverPort_dry_results([dry_results])
    R29_usecase_usecase_DryDriverPort_dry_check_approved([dry_check_approved])
    R29_usecase_usecase_DryDriverPort_dry_fix_local([dry_fix_local])
  end
  subgraph R32_usecase_usecase_DryDriverService["dry_driver::DryDriverService"]
    direction TB
    R32_usecase_usecase_DryDriverService__self[DryDriverService]
    R32_usecase_usecase_DryDriverService_dry_write([dry_write])
    R32_usecase_usecase_DryDriverService_dry_results([dry_results])
    R32_usecase_usecase_DryDriverService_dry_check_approved([dry_check_approved])
    R32_usecase_usecase_DryDriverService_dry_fix_local([dry_fix_local])
  end
  end
  subgraph usecase_usecase_module_fixpoint_resolve["usecase::fixpoint_resolve"]
    direction TB
  subgraph R36_usecase_usecase_DiffBaseResolverPort["fixpoint_resolve::DiffBaseResolverPort"]
    direction TB
    R36_usecase_usecase_DiffBaseResolverPort__self[DiffBaseResolverPort]
    R36_usecase_usecase_DiffBaseResolverPort_resolve_diff_base([resolve_diff_base])
  end
  subgraph R38_usecase_usecase_DryApprovalFactoryPort["fixpoint_resolve::DryApprovalFactoryPort"]
    direction TB
    R38_usecase_usecase_DryApprovalFactoryPort__self[DryApprovalFactoryPort]
    R38_usecase_usecase_DryApprovalFactoryPort_build_approval([build_approval])
  end
  subgraph R38_usecase_usecase_FixpointDryGateService["fixpoint_resolve::FixpointDryGateService"]
    direction TB
    R38_usecase_usecase_FixpointDryGateService__self[FixpointDryGateService]
    R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate([resolve_dry_gate])
  end
  subgraph R38_usecase_usecase_RefVerifyGateStatePort["fixpoint_resolve::RefVerifyGateStatePort"]
    direction TB
    R38_usecase_usecase_RefVerifyGateStatePort__self[RefVerifyGateStatePort]
    R38_usecase_usecase_RefVerifyGateStatePort_ref_verify_status([ref_verify_status])
  end
  subgraph R35_usecase_usecase_ReviewGateStatePort["fixpoint_resolve::ReviewGateStatePort"]
    direction TB
    R35_usecase_usecase_ReviewGateStatePort__self[ReviewGateStatePort]
    R35_usecase_usecase_ReviewGateStatePort_review_status([review_status])
  end
  end
  subgraph usecase_usecase_module_fixpoint_resolve_driver["usecase::fixpoint_resolve_driver"]
    direction TB
  subgraph T41_usecase_usecase_DryCheckConfigLoaderError["fixpoint_resolve_driver::DryCheckConfigLoaderError"]
    direction TB
    T41_usecase_usecase_DryCheckConfigLoaderError__self[DryCheckConfigLoaderError]
    T41_usecase_usecase_DryCheckConfigLoaderError_Unavailable[Unavailable]
  end
  subgraph T42_usecase_usecase_FixpointResolveDriverInput["fixpoint_resolve_driver::FixpointResolveDriverInput"]
    direction TB
    T42_usecase_usecase_FixpointResolveDriverInput__self[FixpointResolveDriverInput]
  end
  subgraph T47_usecase_usecase_FixpointResolveDriverInteractor["fixpoint_resolve_driver::FixpointResolveDriverInteractor"]
    direction TB
    T47_usecase_usecase_FixpointResolveDriverInteractor__self[FixpointResolveDriverInteractor]
    T47_usecase_usecase_FixpointResolveDriverInteractor_new([new])
  end
  subgraph T44_usecase_usecase_FixpointResolveDriverOutcome["fixpoint_resolve_driver::FixpointResolveDriverOutcome"]
    direction TB
    T44_usecase_usecase_FixpointResolveDriverOutcome__self[FixpointResolveDriverOutcome]
    T44_usecase_usecase_FixpointResolveDriverOutcome_RunDfp[RunDfp]
    T44_usecase_usecase_FixpointResolveDriverOutcome_RunRfp[RunRfp]
    T44_usecase_usecase_FixpointResolveDriverOutcome_RunRefVerify[RunRefVerify]
    T44_usecase_usecase_FixpointResolveDriverOutcome_Commit[Commit]
    T44_usecase_usecase_FixpointResolveDriverOutcome_Failure[Failure]
  end
  subgraph T40_usecase_usecase_FixpointWorkspaceContext["fixpoint_resolve_driver::FixpointWorkspaceContext"]
    direction TB
    T40_usecase_usecase_FixpointWorkspaceContext__self[FixpointWorkspaceContext]
  end
  subgraph T45_usecase_usecase_FixpointWorkspaceContextError["fixpoint_resolve_driver::FixpointWorkspaceContextError"]
    direction TB
    T45_usecase_usecase_FixpointWorkspaceContextError__self[FixpointWorkspaceContextError]
    T45_usecase_usecase_FixpointWorkspaceContextError_Unavailable[Unavailable]
  end
  subgraph R40_usecase_usecase_DryCheckConfigLoaderPort["fixpoint_resolve_driver::DryCheckConfigLoaderPort"]
    direction TB
    R40_usecase_usecase_DryCheckConfigLoaderPort__self[DryCheckConfigLoaderPort]
    R40_usecase_usecase_DryCheckConfigLoaderPort_load([load])
  end
  subgraph R42_usecase_usecase_FixpointDryGateFactoryPort["fixpoint_resolve_driver::FixpointDryGateFactoryPort"]
    direction TB
    R42_usecase_usecase_FixpointDryGateFactoryPort__self[FixpointDryGateFactoryPort]
    R42_usecase_usecase_FixpointDryGateFactoryPort_build([build])
  end
  subgraph R44_usecase_usecase_FixpointGateStateFactoryPort["fixpoint_resolve_driver::FixpointGateStateFactoryPort"]
    direction TB
    R44_usecase_usecase_FixpointGateStateFactoryPort__self[FixpointGateStateFactoryPort]
    R44_usecase_usecase_FixpointGateStateFactoryPort_build_review_gate([build_review_gate])
    R44_usecase_usecase_FixpointGateStateFactoryPort_build_ref_verify_gate([build_ref_verify_gate])
  end
  subgraph R44_usecase_usecase_FixpointResolveDriverService["fixpoint_resolve_driver::FixpointResolveDriverService"]
    direction TB
    R44_usecase_usecase_FixpointResolveDriverService__self[FixpointResolveDriverService]
    R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve([fixpoint_resolve])
  end
  subgraph R44_usecase_usecase_FixpointWorkspaceContextPort["fixpoint_resolve_driver::FixpointWorkspaceContextPort"]
    direction TB
    R44_usecase_usecase_FixpointWorkspaceContextPort__self[FixpointWorkspaceContextPort]
    R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context([resolve_context])
  end
  end
  subgraph usecase_usecase_module_track_service["usecase::track_service"]
    direction TB
  subgraph R28_usecase_usecase_TrackService["track_service::TrackService"]
    direction TB
    R28_usecase_usecase_TrackService__self[TrackService]
    R28_usecase_usecase_TrackService_init([init])
    R28_usecase_usecase_TrackService_transition([transition])
    R28_usecase_usecase_TrackService_resolve([resolve])
    R28_usecase_usecase_TrackService_branch_create([branch_create])
    R28_usecase_usecase_TrackService_branch_switch([branch_switch])
    R28_usecase_usecase_TrackService_views_validate([views_validate])
    R28_usecase_usecase_TrackService_views_sync([views_sync])
    R28_usecase_usecase_TrackService_add_task([add_task])
    R28_usecase_usecase_TrackService_set_override([set_override])
    R28_usecase_usecase_TrackService_clear_override([clear_override])
    R28_usecase_usecase_TrackService_next_task([next_task])
    R28_usecase_usecase_TrackService_task_counts([task_counts])
    R28_usecase_usecase_TrackService_archive([archive])
    R28_usecase_usecase_TrackService_detect_active([detect_active])
    R28_usecase_usecase_TrackService_switch_base([switch_base])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_branch_strategy["infrastructure::branch_strategy"]
    direction TB
  subgraph T55_infrastructure_infrastructure_BranchStrategyConfigError["branch_strategy::BranchStrategyConfigError"]
    direction TB
    T55_infrastructure_infrastructure_BranchStrategyConfigError__self[BranchStrategyConfigError]
    T55_infrastructure_infrastructure_BranchStrategyConfigError_Io[Io]
    T55_infrastructure_infrastructure_BranchStrategyConfigError_Parse[Parse]
  end
  subgraph T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter["branch_strategy::JsonConfigBranchStrategyAdapter"]
    direction TB
    T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self[JsonConfigBranchStrategyAdapter]
    T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new([new])
  end
  subgraph T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter["branch_strategy::SnapshotBranchStrategyAdapter"]
    direction TB
    T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self[SnapshotBranchStrategyAdapter]
    T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_dry_check["infrastructure::dry_check"]
    direction TB
  subgraph T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter["dry_check::diff_base_resolver::FsDiffBaseResolverAdapter"]
    direction TB
    T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self[FsDiffBaseResolverAdapter]
    T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter_new([new])
  end
  subgraph T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter["dry_check::approval_factory::FsDryApprovalFactoryAdapter"]
    direction TB
    T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter__self[FsDryApprovalFactoryAdapter]
  end
  end
  subgraph infrastructure_infrastructure_module_track["infrastructure::track"]
    direction TB
  subgraph T60_infrastructure_infrastructure_BranchStrategySnapshotDocument["track::codec::BranchStrategySnapshotDocument"]
    direction TB
    T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self[BranchStrategySnapshotDocument]
  end
  subgraph T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter["track::fixpoint_resolve_driver::FsDryCheckConfigLoaderAdapter"]
    direction TB
    T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter__self[FsDryCheckConfigLoaderAdapter]
  end
  subgraph T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter["track::fixpoint_resolve_driver::FsFixpointDryGateFactoryAdapter"]
    direction TB
    T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter__self[FsFixpointDryGateFactoryAdapter]
  end
  subgraph T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter["track::fixpoint_resolve_driver::FsFixpointGateStateFactoryAdapter"]
    direction TB
    T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter__self[FsFixpointGateStateFactoryAdapter]
  end
  subgraph T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter["track::fixpoint_resolve_driver::FsFixpointWorkspaceContextAdapter"]
    direction TB
    T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter__self[FsFixpointWorkspaceContextAdapter]
  end
  subgraph T54_infrastructure_infrastructure_FsReviewGateStateAdapter["track::gate_state::FsReviewGateStateAdapter"]
    direction TB
    T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self[FsReviewGateStateAdapter]
    T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new([new])
  end
  subgraph T49_infrastructure_infrastructure_MergeMethodDocument["track::codec::MergeMethodDocument"]
    direction TB
    T49_infrastructure_infrastructure_MergeMethodDocument__self[MergeMethodDocument]
    T49_infrastructure_infrastructure_MergeMethodDocument_Squash[Squash]
    T49_infrastructure_infrastructure_MergeMethodDocument_Merge[Merge]
    T49_infrastructure_infrastructure_MergeMethodDocument_Rebase[Rebase]
  end
  subgraph T45_infrastructure_infrastructure_TrackDocumentV2["track::codec::TrackDocumentV2"]
    direction TB
    T45_infrastructure_infrastructure_TrackDocumentV2__self[TrackDocumentV2]
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_dry["cli_driver::dry"]
    direction TB
  subgraph T31_cli_driver_cli_driver_DryDriver["dry::DryDriver"]
    direction TB
    T31_cli_driver_cli_driver_DryDriver__self[DryDriver]
    T31_cli_driver_cli_driver_DryDriver_new([new])
    T31_cli_driver_cli_driver_DryDriver_handle([handle])
  end
  end
  subgraph cli_driver_cli_driver_module_pr["cli_driver::pr"]
    direction TB
  subgraph T29_cli_driver_cli_driver_PrInput["pr::PrInput"]
    direction TB
    T29_cli_driver_cli_driver_PrInput__self[PrInput]
    T29_cli_driver_cli_driver_PrInput_Push[Push]
    T29_cli_driver_cli_driver_PrInput_Ensure[Ensure]
    T29_cli_driver_cli_driver_PrInput_Status[Status]
    T29_cli_driver_cli_driver_PrInput_WaitAndMerge[WaitAndMerge]
    T29_cli_driver_cli_driver_PrInput_TriggerReview[TriggerReview]
    T29_cli_driver_cli_driver_PrInput_PollReview[PollReview]
    T29_cli_driver_cli_driver_PrInput_ReviewCycle[ReviewCycle]
  end
  end
  subgraph cli_driver_cli_driver_module_track["cli_driver::track"]
    direction TB
  subgraph T33_cli_driver_cli_driver_TrackDriver["track::TrackDriver"]
    direction TB
    T33_cli_driver_cli_driver_TrackDriver__self[TrackDriver]
    T33_cli_driver_cli_driver_TrackDriver_new([new])
    T33_cli_driver_cli_driver_TrackDriver_handle([handle])
  end
  subgraph T32_cli_driver_cli_driver_TrackInput["track::TrackInput"]
    direction TB
    T32_cli_driver_cli_driver_TrackInput__self[TrackInput]
    T32_cli_driver_cli_driver_TrackInput_Init[Init]
    T32_cli_driver_cli_driver_TrackInput_Transition[Transition]
    T32_cli_driver_cli_driver_TrackInput_Resolve[Resolve]
    T32_cli_driver_cli_driver_TrackInput_BranchCreate[BranchCreate]
    T32_cli_driver_cli_driver_TrackInput_BranchSwitch[BranchSwitch]
    T32_cli_driver_cli_driver_TrackInput_ViewsValidate[ViewsValidate]
    T32_cli_driver_cli_driver_TrackInput_ViewsSync[ViewsSync]
    T32_cli_driver_cli_driver_TrackInput_AddTask[AddTask]
    T32_cli_driver_cli_driver_TrackInput_SetOverride[SetOverride]
    T32_cli_driver_cli_driver_TrackInput_ClearOverride[ClearOverride]
    T32_cli_driver_cli_driver_TrackInput_NextTask[NextTask]
    T32_cli_driver_cli_driver_TrackInput_TaskCounts[TaskCounts]
    T32_cli_driver_cli_driver_TrackInput_Archive[Archive]
    T32_cli_driver_cli_driver_TrackInput_DetectActive[DetectActive]
    T32_cli_driver_cli_driver_TrackInput_SwitchBase[SwitchBase]
    T32_cli_driver_cli_driver_TrackInput_FixpointResolve[FixpointResolve]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_dry["cli_composition::dry"]
    direction TB
  subgraph T50_cli_composition_cli_composition_DryCompositionRoot["dry::shim::DryCompositionRoot"]
    direction TB
    T50_cli_composition_cli_composition_DryCompositionRoot__self[DryCompositionRoot]
    T50_cli_composition_cli_composition_DryCompositionRoot_new([new])
    T50_cli_composition_cli_composition_DryCompositionRoot_dry_write([dry_write])
    T50_cli_composition_cli_composition_DryCompositionRoot_dry_results([dry_results])
    T50_cli_composition_cli_composition_DryCompositionRoot_dry_check_approved([dry_check_approved])
    T50_cli_composition_cli_composition_DryCompositionRoot_dry_driver([dry_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_git["cli_composition::git"]
    direction TB
  subgraph T50_cli_composition_cli_composition_GitCompositionRoot["git::GitCompositionRoot"]
    direction TB
    T50_cli_composition_cli_composition_GitCompositionRoot__self[GitCompositionRoot]
    T50_cli_composition_cli_composition_GitCompositionRoot_new([new])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_add_all([git_add_all])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_add_from_file([git_add_from_file])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_commit_from_file([git_commit_from_file])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_note_from_file([git_note_from_file])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull([git_switch_and_pull])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull_in([git_switch_and_pull_in])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_unstage([git_unstage])
    T50_cli_composition_cli_composition_GitCompositionRoot_current_branch_track_id_strict([current_branch_track_id_strict])
    T50_cli_composition_cli_composition_GitCompositionRoot_git_driver([git_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_track["cli_composition::track"]
    direction TB
  subgraph T52_cli_composition_cli_composition_TrackCompositionRoot["track::composition_root::TrackCompositionRoot"]
    direction TB
    T52_cli_composition_cli_composition_TrackCompositionRoot__self[TrackCompositionRoot]
    T52_cli_composition_cli_composition_TrackCompositionRoot_new([new])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_driver([track_driver])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task_resolved([track_add_task_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override_resolved([track_set_override_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override_resolved([track_clear_override_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task_resolved([track_next_task_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts_resolved([track_task_counts_resolved])
    T52_cli_composition_cli_composition_TrackCompositionRoot_detect_active_track_from_branch([detect_active_track_from_branch])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_commit_hash([track_set_commit_hash])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_signals([track_type_signals])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_graph([track_type_graph])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_graph([track_baseline_graph])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_contract_map([track_contract_map])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_spec_signals([track_catalogue_spec_signals])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_spec_element_hash([track_spec_element_hash])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_capture([track_baseline_capture])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_lint([track_lint])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_impl_signals([track_catalogue_impl_signals])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id([track_resolve_id])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root([track_resolve_id_from_root])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_for_write([track_resolve_id_for_write])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root_for_write([track_resolve_id_from_root_for_write])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_validate_id([track_validate_id])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_project_root([track_resolve_project_root])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_init([track_init])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_transition([track_transition])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_create([track_branch_create])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_switch([track_branch_switch])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve([track_resolve])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_validate([track_views_validate])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_sync([track_views_sync])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task([track_add_task])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override([track_set_override])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override([track_clear_override])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task([track_next_task])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts([track_task_counts])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_archive([track_archive])
    T52_cli_composition_cli_composition_TrackCompositionRoot_track_switch_base([track_switch_base])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T20_cli_cli_EnsurePrArgs["commands::pr::EnsurePrArgs"]
    direction TB
    T20_cli_cli_EnsurePrArgs__self[EnsurePrArgs]
  end
  subgraph T20_cli_cli_TrackCommand["commands::track::TrackCommand"]
    direction TB
    T20_cli_cli_TrackCommand__self[TrackCommand]
    T20_cli_cli_TrackCommand_Archive[Archive]
    T20_cli_cli_TrackCommand_Transition[Transition]
    T20_cli_cli_TrackCommand_Branch[Branch]
    T20_cli_cli_TrackCommand_Resolve[Resolve]
    T20_cli_cli_TrackCommand_Views[Views]
    T20_cli_cli_TrackCommand_AddTask[AddTask]
    T20_cli_cli_TrackCommand_SetOverride[SetOverride]
    T20_cli_cli_TrackCommand_ClearOverride[ClearOverride]
    T20_cli_cli_TrackCommand_NextTask[NextTask]
    T20_cli_cli_TrackCommand_TaskCounts[TaskCounts]
    T20_cli_cli_TrackCommand_TypeGraph[TypeGraph]
    T20_cli_cli_TrackCommand_BaselineGraph[BaselineGraph]
    T20_cli_cli_TrackCommand_ContractMap[ContractMap]
    T20_cli_cli_TrackCommand_SpecElementHash[SpecElementHash]
    T20_cli_cli_TrackCommand_BaselineCapture[BaselineCapture]
    T20_cli_cli_TrackCommand_Lint[Lint]
    T20_cli_cli_TrackCommand_CatalogueImplSignals[CatalogueImplSignals]
    T20_cli_cli_TrackCommand_FixpointResolve[FixpointResolve]
    T20_cli_cli_TrackCommand_SetCommitHash[SetCommitHash]
    T20_cli_cli_TrackCommand_SwitchBase[SwitchBase]
    T20_cli_cli_TrackCommand_items_dir([items_dir])
  end
  subgraph T24_cli_cli_WaitAndMergeArgs["commands::pr::WaitAndMergeArgs"]
    direction TB
    T24_cli_cli_WaitAndMergeArgs__self[WaitAndMergeArgs]
  end
  end
end
T36_domain_domain_BranchStrategySnapshot_new --o T25_domain_domain_MergeMethod__self
T36_domain_domain_BranchStrategySnapshot_new --> T36_domain_domain_BranchStrategySnapshot__self
T36_domain_domain_BranchStrategySnapshot_merge_method --> T25_domain_domain_MergeMethod__self
T27_domain_domain_TrackMetadata_new --o T36_domain_domain_BranchStrategySnapshot__self
T27_domain_domain_TrackMetadata_new --> T27_domain_domain_TrackMetadata__self
T27_domain_domain_TrackMetadata_with_branch --o T36_domain_domain_BranchStrategySnapshot__self
T27_domain_domain_TrackMetadata_with_branch --> T27_domain_domain_TrackMetadata__self
T27_domain_domain_TrackMetadata_branch_strategy_snapshot --> T36_domain_domain_BranchStrategySnapshot__self
R34_usecase_usecase_BranchStrategyPort_merge_method --> T25_domain_domain_MergeMethod__self
T31_usecase_usecase_DryWriteOutcome_Success --o|findings| T38_usecase_usecase_DryWriteFindingSummary__self
R29_usecase_usecase_DryDriverPort_dry_write --> T31_usecase_usecase_DryWriteOutcome__self
R29_usecase_usecase_DryDriverPort_dry_check_approved --> T39_usecase_usecase_DryCheckApprovedOutcome__self
R32_usecase_usecase_DryDriverService_dry_write --> T31_usecase_usecase_DryWriteOutcome__self
R32_usecase_usecase_DryDriverService_dry_check_approved --> T39_usecase_usecase_DryCheckApprovedOutcome__self
T47_usecase_usecase_FixpointResolveDriverInteractor_new --> T47_usecase_usecase_FixpointResolveDriverInteractor__self
R40_usecase_usecase_DryCheckConfigLoaderPort_load --> T41_usecase_usecase_DryCheckConfigLoaderError__self
R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve --o T42_usecase_usecase_FixpointResolveDriverInput__self
R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve --> T44_usecase_usecase_FixpointResolveDriverOutcome__self
R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context --> T40_usecase_usecase_FixpointWorkspaceContext__self
R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context --> T45_usecase_usecase_FixpointWorkspaceContextError__self
T35_usecase_usecase_DryDriverInteractor__self -.impl.-> R32_usecase_usecase_DryDriverService__self
T47_usecase_usecase_FixpointResolveDriverInteractor__self -.impl.-> R44_usecase_usecase_FixpointResolveDriverService__self
T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new --> T55_infrastructure_infrastructure_BranchStrategyConfigError__self
T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new --> T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self
T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new --o T36_domain_domain_BranchStrategySnapshot__self
T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new --> T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self
T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter_new --> T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self
T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self --o|merge_method| T49_infrastructure_infrastructure_MergeMethodDocument__self
T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new --> T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self
T45_infrastructure_infrastructure_TrackDocumentV2__self --o|branch_strategy_snapshot| T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self
T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self -.impl.-> R34_usecase_usecase_BranchStrategyPort__self
T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self -.impl.-> R34_usecase_usecase_BranchStrategyPort__self
T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self -.impl.-> R35_usecase_usecase_ReviewGateStatePort__self
T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self -.impl.-> R36_usecase_usecase_DiffBaseResolverPort__self
T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter__self -.impl.-> R38_usecase_usecase_DryApprovalFactoryPort__self
T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter__self -.impl.-> R44_usecase_usecase_FixpointWorkspaceContextPort__self
T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter__self -.impl.-> R40_usecase_usecase_DryCheckConfigLoaderPort__self
T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter__self -.impl.-> R42_usecase_usecase_FixpointDryGateFactoryPort__self
T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter__self -.impl.-> R44_usecase_usecase_FixpointGateStateFactoryPort__self
T31_cli_driver_cli_driver_DryDriver_new --> T31_cli_driver_cli_driver_DryDriver__self
T33_cli_driver_cli_driver_TrackDriver_new --> T33_cli_driver_cli_driver_TrackDriver__self
T33_cli_driver_cli_driver_TrackDriver_handle --o T32_cli_driver_cli_driver_TrackInput__self
T50_cli_composition_cli_composition_DryCompositionRoot_new --> T50_cli_composition_cli_composition_DryCompositionRoot__self
T50_cli_composition_cli_composition_DryCompositionRoot_dry_write --> T31_usecase_usecase_DryWriteOutcome__self
T50_cli_composition_cli_composition_DryCompositionRoot_dry_check_approved --> T39_usecase_usecase_DryCheckApprovedOutcome__self
T50_cli_composition_cli_composition_DryCompositionRoot_dry_driver --> T31_cli_driver_cli_driver_DryDriver__self
T50_cli_composition_cli_composition_GitCompositionRoot_new --> T50_cli_composition_cli_composition_GitCompositionRoot__self
T52_cli_composition_cli_composition_TrackCompositionRoot_new --> T52_cli_composition_cli_composition_TrackCompositionRoot__self
T52_cli_composition_cli_composition_TrackCompositionRoot_track_driver --> T33_cli_driver_cli_driver_TrackDriver__self
class T36_domain_domain_BranchStrategySnapshot_new method_node
class T36_domain_domain_BranchStrategySnapshot_base_branch method_node
class T36_domain_domain_BranchStrategySnapshot_merge_target method_node
class T36_domain_domain_BranchStrategySnapshot_merge_method method_node
class T36_domain_domain_BranchStrategySnapshot__self value_object
class T25_domain_domain_MergeMethod_Squash variant_node
class T25_domain_domain_MergeMethod_Merge variant_node
class T25_domain_domain_MergeMethod_Rebase variant_node
class T25_domain_domain_MergeMethod__self value_object
class T27_domain_domain_TrackMetadata_new method_node
class T27_domain_domain_TrackMetadata_with_branch method_node
class T27_domain_domain_TrackMetadata_id method_node
class T27_domain_domain_TrackMetadata_branch method_node
class T27_domain_domain_TrackMetadata_is_activated method_node
class T27_domain_domain_TrackMetadata_set_branch method_node
class T27_domain_domain_TrackMetadata_title method_node
class T27_domain_domain_TrackMetadata_status_override method_node
class T27_domain_domain_TrackMetadata_set_status_override method_node
class T27_domain_domain_TrackMetadata_branch_strategy_snapshot method_node
class T27_domain_domain_TrackMetadata__self entity
class R34_usecase_usecase_BranchStrategyPort_base_branch method_node
class R34_usecase_usecase_BranchStrategyPort_merge_target method_node
class R34_usecase_usecase_BranchStrategyPort_merge_method method_node
class R34_usecase_usecase_BranchStrategyPort_track_prefix method_node
class R34_usecase_usecase_BranchStrategyPort__self secondary_port
class T39_usecase_usecase_DryCheckApprovedOutcome_Approved variant_node
class T39_usecase_usecase_DryCheckApprovedOutcome_Blocked variant_node
class T39_usecase_usecase_DryCheckApprovedOutcome_Failure variant_node
class T39_usecase_usecase_DryCheckApprovedOutcome__self dto
class T35_usecase_usecase_DryDriverInteractor__self interactor
class T38_usecase_usecase_DryWriteFindingSummary__self dto
class T31_usecase_usecase_DryWriteOutcome_Success variant_node
class T31_usecase_usecase_DryWriteOutcome_Failure variant_node
class T31_usecase_usecase_DryWriteOutcome__self dto
class R29_usecase_usecase_DryDriverPort_dry_write method_node
class R29_usecase_usecase_DryDriverPort_dry_results method_node
class R29_usecase_usecase_DryDriverPort_dry_check_approved method_node
class R29_usecase_usecase_DryDriverPort_dry_fix_local method_node
class R29_usecase_usecase_DryDriverPort__self secondary_port
class R32_usecase_usecase_DryDriverService_dry_write method_node
class R32_usecase_usecase_DryDriverService_dry_results method_node
class R32_usecase_usecase_DryDriverService_dry_check_approved method_node
class R32_usecase_usecase_DryDriverService_dry_fix_local method_node
class R32_usecase_usecase_DryDriverService__self app_service
class R36_usecase_usecase_DiffBaseResolverPort_resolve_diff_base method_node
class R36_usecase_usecase_DiffBaseResolverPort__self secondary_port
class R38_usecase_usecase_DryApprovalFactoryPort_build_approval method_node
class R38_usecase_usecase_DryApprovalFactoryPort__self secondary_port
class R38_usecase_usecase_FixpointDryGateService_resolve_dry_gate method_node
class R38_usecase_usecase_FixpointDryGateService__self app_service
class R38_usecase_usecase_RefVerifyGateStatePort_ref_verify_status method_node
class R38_usecase_usecase_RefVerifyGateStatePort__self secondary_port
class R35_usecase_usecase_ReviewGateStatePort_review_status method_node
class R35_usecase_usecase_ReviewGateStatePort__self secondary_port
class T41_usecase_usecase_DryCheckConfigLoaderError_Unavailable variant_node
class T41_usecase_usecase_DryCheckConfigLoaderError__self error_type
class T42_usecase_usecase_FixpointResolveDriverInput__self dto
class T47_usecase_usecase_FixpointResolveDriverInteractor_new method_node
class T47_usecase_usecase_FixpointResolveDriverInteractor__self interactor
class T44_usecase_usecase_FixpointResolveDriverOutcome_RunDfp variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_RunRfp variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_RunRefVerify variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_Commit variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome_Failure variant_node
class T44_usecase_usecase_FixpointResolveDriverOutcome__self dto
class T40_usecase_usecase_FixpointWorkspaceContext__self dto
class T45_usecase_usecase_FixpointWorkspaceContextError_Unavailable variant_node
class T45_usecase_usecase_FixpointWorkspaceContextError__self error_type
class R40_usecase_usecase_DryCheckConfigLoaderPort_load method_node
class R40_usecase_usecase_DryCheckConfigLoaderPort__self secondary_port
class R42_usecase_usecase_FixpointDryGateFactoryPort_build method_node
class R42_usecase_usecase_FixpointDryGateFactoryPort__self secondary_port
class R44_usecase_usecase_FixpointGateStateFactoryPort_build_review_gate method_node
class R44_usecase_usecase_FixpointGateStateFactoryPort_build_ref_verify_gate method_node
class R44_usecase_usecase_FixpointGateStateFactoryPort__self secondary_port
class R44_usecase_usecase_FixpointResolveDriverService_fixpoint_resolve method_node
class R44_usecase_usecase_FixpointResolveDriverService__self app_service
class R44_usecase_usecase_FixpointWorkspaceContextPort_resolve_context method_node
class R44_usecase_usecase_FixpointWorkspaceContextPort__self secondary_port
class R28_usecase_usecase_TrackService_init method_node
class R28_usecase_usecase_TrackService_transition method_node
class R28_usecase_usecase_TrackService_resolve method_node
class R28_usecase_usecase_TrackService_branch_create method_node
class R28_usecase_usecase_TrackService_branch_switch method_node
class R28_usecase_usecase_TrackService_views_validate method_node
class R28_usecase_usecase_TrackService_views_sync method_node
class R28_usecase_usecase_TrackService_add_task method_node
class R28_usecase_usecase_TrackService_set_override method_node
class R28_usecase_usecase_TrackService_clear_override method_node
class R28_usecase_usecase_TrackService_next_task method_node
class R28_usecase_usecase_TrackService_task_counts method_node
class R28_usecase_usecase_TrackService_archive method_node
class R28_usecase_usecase_TrackService_detect_active method_node
class R28_usecase_usecase_TrackService_switch_base method_node
class R28_usecase_usecase_TrackService__self app_service
class T55_infrastructure_infrastructure_BranchStrategyConfigError_Io variant_node
class T55_infrastructure_infrastructure_BranchStrategyConfigError_Parse variant_node
class T55_infrastructure_infrastructure_BranchStrategyConfigError__self error_type
class T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter_new method_node
class T61_infrastructure_infrastructure_JsonConfigBranchStrategyAdapter__self secondary_adapter
class T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter_new method_node
class T59_infrastructure_infrastructure_SnapshotBranchStrategyAdapter__self secondary_adapter
class T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter_new method_node
class T55_infrastructure_infrastructure_FsDiffBaseResolverAdapter__self secondary_adapter
class T57_infrastructure_infrastructure_FsDryApprovalFactoryAdapter__self secondary_adapter
class T60_infrastructure_infrastructure_BranchStrategySnapshotDocument__self dto
class T59_infrastructure_infrastructure_FsDryCheckConfigLoaderAdapter__self secondary_adapter
class T61_infrastructure_infrastructure_FsFixpointDryGateFactoryAdapter__self secondary_adapter
class T63_infrastructure_infrastructure_FsFixpointGateStateFactoryAdapter__self secondary_adapter
class T63_infrastructure_infrastructure_FsFixpointWorkspaceContextAdapter__self secondary_adapter
class T54_infrastructure_infrastructure_FsReviewGateStateAdapter_new method_node
class T54_infrastructure_infrastructure_FsReviewGateStateAdapter__self secondary_adapter
class T49_infrastructure_infrastructure_MergeMethodDocument_Squash variant_node
class T49_infrastructure_infrastructure_MergeMethodDocument_Merge variant_node
class T49_infrastructure_infrastructure_MergeMethodDocument_Rebase variant_node
class T49_infrastructure_infrastructure_MergeMethodDocument__self dto
class T45_infrastructure_infrastructure_TrackDocumentV2__self dto
class T31_cli_driver_cli_driver_DryDriver_new method_node
class T31_cli_driver_cli_driver_DryDriver_handle method_node
class T29_cli_driver_cli_driver_PrInput_Push variant_node
class T29_cli_driver_cli_driver_PrInput_Ensure variant_node
class T29_cli_driver_cli_driver_PrInput_Status variant_node
class T29_cli_driver_cli_driver_PrInput_WaitAndMerge variant_node
class T29_cli_driver_cli_driver_PrInput_TriggerReview variant_node
class T29_cli_driver_cli_driver_PrInput_PollReview variant_node
class T29_cli_driver_cli_driver_PrInput_ReviewCycle variant_node
class T29_cli_driver_cli_driver_PrInput__self dto
class T33_cli_driver_cli_driver_TrackDriver_new method_node
class T33_cli_driver_cli_driver_TrackDriver_handle method_node
class T32_cli_driver_cli_driver_TrackInput_Init variant_node
class T32_cli_driver_cli_driver_TrackInput_Transition variant_node
class T32_cli_driver_cli_driver_TrackInput_Resolve variant_node
class T32_cli_driver_cli_driver_TrackInput_BranchCreate variant_node
class T32_cli_driver_cli_driver_TrackInput_BranchSwitch variant_node
class T32_cli_driver_cli_driver_TrackInput_ViewsValidate variant_node
class T32_cli_driver_cli_driver_TrackInput_ViewsSync variant_node
class T32_cli_driver_cli_driver_TrackInput_AddTask variant_node
class T32_cli_driver_cli_driver_TrackInput_SetOverride variant_node
class T32_cli_driver_cli_driver_TrackInput_ClearOverride variant_node
class T32_cli_driver_cli_driver_TrackInput_NextTask variant_node
class T32_cli_driver_cli_driver_TrackInput_TaskCounts variant_node
class T32_cli_driver_cli_driver_TrackInput_Archive variant_node
class T32_cli_driver_cli_driver_TrackInput_DetectActive variant_node
class T32_cli_driver_cli_driver_TrackInput_SwitchBase variant_node
class T32_cli_driver_cli_driver_TrackInput_FixpointResolve variant_node
class T32_cli_driver_cli_driver_TrackInput__self dto
class T50_cli_composition_cli_composition_DryCompositionRoot_new method_node
class T50_cli_composition_cli_composition_DryCompositionRoot_dry_write method_node
class T50_cli_composition_cli_composition_DryCompositionRoot_dry_results method_node
class T50_cli_composition_cli_composition_DryCompositionRoot_dry_check_approved method_node
class T50_cli_composition_cli_composition_DryCompositionRoot_dry_driver method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_new method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_add_all method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_add_from_file method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_commit_from_file method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_note_from_file method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_switch_and_pull_in method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_unstage method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_current_branch_track_id_strict method_node
class T50_cli_composition_cli_composition_GitCompositionRoot_git_driver method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_new method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_driver method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts_resolved method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_detect_active_track_from_branch method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_commit_hash method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_signals method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_type_graph method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_graph method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_contract_map method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_spec_signals method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_spec_element_hash method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_baseline_capture method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_lint method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_catalogue_impl_signals method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_for_write method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_id_from_root_for_write method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_validate_id method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve_project_root method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_init method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_transition method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_create method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_branch_switch method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_resolve method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_validate method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_views_sync method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_add_task method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_set_override method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_clear_override method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_next_task method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_task_counts method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_archive method_node
class T52_cli_composition_cli_composition_TrackCompositionRoot_track_switch_base method_node
class T20_cli_cli_EnsurePrArgs__self dto
class T20_cli_cli_TrackCommand_Archive variant_node
class T20_cli_cli_TrackCommand_Transition variant_node
class T20_cli_cli_TrackCommand_Branch variant_node
class T20_cli_cli_TrackCommand_Resolve variant_node
class T20_cli_cli_TrackCommand_Views variant_node
class T20_cli_cli_TrackCommand_AddTask variant_node
class T20_cli_cli_TrackCommand_SetOverride variant_node
class T20_cli_cli_TrackCommand_ClearOverride variant_node
class T20_cli_cli_TrackCommand_NextTask variant_node
class T20_cli_cli_TrackCommand_TaskCounts variant_node
class T20_cli_cli_TrackCommand_TypeGraph variant_node
class T20_cli_cli_TrackCommand_BaselineGraph variant_node
class T20_cli_cli_TrackCommand_ContractMap variant_node
class T20_cli_cli_TrackCommand_SpecElementHash variant_node
class T20_cli_cli_TrackCommand_BaselineCapture variant_node
class T20_cli_cli_TrackCommand_Lint variant_node
class T20_cli_cli_TrackCommand_CatalogueImplSignals variant_node
class T20_cli_cli_TrackCommand_FixpointResolve variant_node
class T20_cli_cli_TrackCommand_SetCommitHash variant_node
class T20_cli_cli_TrackCommand_SwitchBase variant_node
class T20_cli_cli_TrackCommand_items_dir method_node
class T20_cli_cli_TrackCommand__self dto
class T24_cli_cli_WaitAndMergeArgs__self dto
```
