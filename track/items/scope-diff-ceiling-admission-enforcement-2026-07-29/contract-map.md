<!-- Generated contract-map-renderer — DO NOT EDIT DIRECTLY -->
```mermaid
---
config:
  layout: elk
---
flowchart LR
classDef aggregate_root fill:#ede9fe,stroke:#4c1d95,stroke-width:2px
classDef app_service fill:#ecfdf5,stroke:#059669,stroke-width:2px
classDef command fill:#fff7ed,stroke:#c2410c,stroke-width:1px
classDef composition_root fill:#e0e7ff,stroke:#3730a3,stroke-width:2px
classDef domain_event fill:#fce7f3,stroke:#9d174d,stroke-width:1px
classDef domain_service fill:#fee2e2,stroke:#991b1b,stroke-width:1px
classDef dto fill:#f8fafc,stroke:#64748b,stroke-width:1px
classDef entity fill:#dbeafe,stroke:#1e40af,stroke-width:2px
classDef error_type fill:#fef2f2,stroke:#b91c1c,stroke-width:1px,stroke-dasharray:4 2
classDef event_policy fill:#fef3c7,stroke:#92400e,stroke-width:1px
classDef factory fill:#e0f2fe,stroke:#0369a1,stroke-width:1px
classDef free_function fill:#f5f3ff,stroke:#7c3aed,stroke-width:1px
classDef function_node fill:#f5f3ff,stroke:#a78bfa,stroke-width:1px
classDef interactor fill:#f0fdfa,stroke:#0d9488,stroke-width:1px
classDef method_node fill:#f8fafc,stroke:#cbd5e1,stroke-width:1px
classDef primary_adapter fill:#ecfccb,stroke:#3f6212,stroke-width:1px
classDef query fill:#f0f9ff,stroke:#0369a1,stroke-width:1px
classDef repository fill:#f5f3ff,stroke:#6d28d9,stroke-width:1px
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
  subgraph domain_domain_module_batch_plan["domain::batch_plan"]
    direction TB
  subgraph T31_domain_domain_AdmissionDecision["batch_plan::AdmissionDecision"]
    direction TB
    T31_domain_domain_AdmissionDecision__self[AdmissionDecision]
    T31_domain_domain_AdmissionDecision_Admitted[Admitted]
    T31_domain_domain_AdmissionDecision_Rejected[Rejected]
    T31_domain_domain_AdmissionDecision_is_admitted([is_admitted])
    T31_domain_domain_AdmissionDecision_rejection([rejection])
  end
  subgraph T38_domain_domain_AdmissionEvaluationError["batch_plan::AdmissionEvaluationError"]
    direction TB
    T38_domain_domain_AdmissionEvaluationError__self[AdmissionEvaluationError]
    T38_domain_domain_AdmissionEvaluationError_MissingTaskEstimate[MissingTaskEstimate]
  end
  subgraph T32_domain_domain_AdmissionRejection["batch_plan::AdmissionRejection"]
    direction TB
    T32_domain_domain_AdmissionRejection__self[AdmissionRejection]
    T32_domain_domain_AdmissionRejection_NotCurrentBatchMember[NotCurrentBatchMember]
    T32_domain_domain_AdmissionRejection_ScopeCeilingWouldBeExceeded[ScopeCeilingWouldBeExceeded]
  end
  subgraph T30_domain_domain_BatchDeclaration["batch_plan::BatchDeclaration"]
    direction TB
    T30_domain_domain_BatchDeclaration__self[BatchDeclaration]
    T30_domain_domain_BatchDeclaration_new([new])
    T30_domain_domain_BatchDeclaration_id([id])
    T30_domain_domain_BatchDeclaration_task_ids([task_ids])
    T30_domain_domain_BatchDeclaration_contains([contains])
  end
  subgraph T21_domain_domain_BatchId["batch_plan::BatchId"]
    direction TB
    T21_domain_domain_BatchId__self[BatchId]
    T21_domain_domain_BatchId_try_new([try_new])
    T21_domain_domain_BatchId_as_str([as_str])
  end
  subgraph T31_domain_domain_BatchPlanDocument["batch_plan::BatchPlanDocument"]
    direction TB
    T31_domain_domain_BatchPlanDocument__self[BatchPlanDocument]
    T31_domain_domain_BatchPlanDocument_new([new])
    T31_domain_domain_BatchPlanDocument_track_id([track_id])
    T31_domain_domain_BatchPlanDocument_task_estimates([task_estimates])
    T31_domain_domain_BatchPlanDocument_batches([batches])
    T31_domain_domain_BatchPlanDocument_estimate_for([estimate_for])
    T31_domain_domain_BatchPlanDocument_batch_of([batch_of])
    T31_domain_domain_BatchPlanDocument_scope_total([scope_total])
    T31_domain_domain_BatchPlanDocument_current_batch([current_batch])
  end
  subgraph T34_domain_domain_BatchPlanGateOutcome["batch_plan::BatchPlanGateOutcome"]
    direction TB
    T34_domain_domain_BatchPlanGateOutcome__self[BatchPlanGateOutcome]
    T34_domain_domain_BatchPlanGateOutcome_Passed[Passed]
    T34_domain_domain_BatchPlanGateOutcome_Blocked[Blocked]
    T34_domain_domain_BatchPlanGateOutcome_from_violations([from_violations])
    T34_domain_domain_BatchPlanGateOutcome_violations([violations])
  end
  subgraph T36_domain_domain_BatchPlanGateViolation["batch_plan::BatchPlanGateViolation"]
    direction TB
    T36_domain_domain_BatchPlanGateViolation__self[BatchPlanGateViolation]
    T36_domain_domain_BatchPlanGateViolation_CeilingExceeded[CeilingExceeded]
    T36_domain_domain_BatchPlanGateViolation_OversizeScopeHasMultipleContributors[OversizeScopeHasMultipleContributors]
    T36_domain_domain_BatchPlanGateViolation_UnknownTaskRef[UnknownTaskRef]
    T36_domain_domain_BatchPlanGateViolation_UnplannedTask[UnplannedTask]
    T36_domain_domain_BatchPlanGateViolation_DependencyInLaterBatch[DependencyInLaterBatch]
  end
  subgraph T38_domain_domain_BatchPlanValidationError["batch_plan::BatchPlanValidationError"]
    direction TB
    T38_domain_domain_BatchPlanValidationError__self[BatchPlanValidationError]
    T38_domain_domain_BatchPlanValidationError_EmptyJustification[EmptyJustification]
    T38_domain_domain_BatchPlanValidationError_EmptyBatchId[EmptyBatchId]
    T38_domain_domain_BatchPlanValidationError_EmptyBatch[EmptyBatch]
    T38_domain_domain_BatchPlanValidationError_DuplicateTaskEstimate[DuplicateTaskEstimate]
    T38_domain_domain_BatchPlanValidationError_DuplicateScopeEstimate[DuplicateScopeEstimate]
    T38_domain_domain_BatchPlanValidationError_DuplicateBatchId[DuplicateBatchId]
    T38_domain_domain_BatchPlanValidationError_MissingTaskEstimate[MissingTaskEstimate]
    T38_domain_domain_BatchPlanValidationError_UnassignedTask[UnassignedTask]
    T38_domain_domain_BatchPlanValidationError_DuplicateBatchMembership[DuplicateBatchMembership]
  end
  subgraph T41_domain_domain_IndivisibilityJustification["batch_plan::IndivisibilityJustification"]
    direction TB
    T41_domain_domain_IndivisibilityJustification__self[IndivisibilityJustification]
    T41_domain_domain_IndivisibilityJustification_try_new([try_new])
    T41_domain_domain_IndivisibilityJustification_as_str([as_str])
  end
  subgraph T23_domain_domain_LineCount["batch_plan::LineCount"]
    direction TB
    T23_domain_domain_LineCount__self[LineCount]
    T23_domain_domain_LineCount_new([new])
    T23_domain_domain_LineCount_value([value])
    T23_domain_domain_LineCount_saturating_add([saturating_add])
  end
  subgraph T31_domain_domain_MeasuredScopeDiff["batch_plan::MeasuredScopeDiff"]
    direction TB
    T31_domain_domain_MeasuredScopeDiff__self[MeasuredScopeDiff]
    T31_domain_domain_MeasuredScopeDiff_new([new])
    T31_domain_domain_MeasuredScopeDiff_scope([scope])
    T31_domain_domain_MeasuredScopeDiff_lines([lines])
  end
  subgraph T36_domain_domain_NonEmptyGateViolations["batch_plan::NonEmptyGateViolations"]
    direction TB
    T36_domain_domain_NonEmptyGateViolations__self[NonEmptyGateViolations]
    T36_domain_domain_NonEmptyGateViolations_try_new([try_new])
    T36_domain_domain_NonEmptyGateViolations_as_slice([as_slice])
    T36_domain_domain_NonEmptyGateViolations_into_vec([into_vec])
  end
  subgraph T30_domain_domain_NonZeroLineCount["batch_plan::NonZeroLineCount"]
    direction TB
    T30_domain_domain_NonZeroLineCount__self[NonZeroLineCount]
    T30_domain_domain_NonZeroLineCount_try_new([try_new])
    T30_domain_domain_NonZeroLineCount_get([get])
  end
  subgraph T26_domain_domain_ScopeCeiling["batch_plan::ScopeCeiling"]
    direction TB
    T26_domain_domain_ScopeCeiling__self[ScopeCeiling]
    T26_domain_domain_ScopeCeiling_Unconstrained[Unconstrained]
    T26_domain_domain_ScopeCeiling_Limited[Limited]
    T26_domain_domain_ScopeCeiling_resolve([resolve])
    T26_domain_domain_ScopeCeiling_admits([admits])
    T26_domain_domain_ScopeCeiling_limit([limit])
  end
  subgraph T31_domain_domain_ScopeLineEstimate["batch_plan::ScopeLineEstimate"]
    direction TB
    T31_domain_domain_ScopeLineEstimate__self[ScopeLineEstimate]
    T31_domain_domain_ScopeLineEstimate_new([new])
    T31_domain_domain_ScopeLineEstimate_scope([scope])
    T31_domain_domain_ScopeLineEstimate_production_lines([production_lines])
    T31_domain_domain_ScopeLineEstimate_test_lines([test_lines])
    T31_domain_domain_ScopeLineEstimate_total([total])
  end
  subgraph T31_domain_domain_TaskDecomposition["batch_plan::TaskDecomposition"]
    direction TB
    T31_domain_domain_TaskDecomposition__self[TaskDecomposition]
    T31_domain_domain_TaskDecomposition_Decomposable[Decomposable]
    T31_domain_domain_TaskDecomposition_Indivisible[Indivisible]
    T31_domain_domain_TaskDecomposition_justification([justification])
    T31_domain_domain_TaskDecomposition_is_indivisible([is_indivisible])
  end
  subgraph T26_domain_domain_TaskEstimate["batch_plan::TaskEstimate"]
    direction TB
    T26_domain_domain_TaskEstimate__self[TaskEstimate]
    T26_domain_domain_TaskEstimate_new([new])
    T26_domain_domain_TaskEstimate_task_id([task_id])
    T26_domain_domain_TaskEstimate_scope_estimates([scope_estimates])
    T26_domain_domain_TaskEstimate_decomposition([decomposition])
    T26_domain_domain_TaskEstimate_estimate_for([estimate_for])
  end
  F50_domain_domain_domain__batch_plan__check_batch_plan[[check_batch_plan]]
  F52_domain_domain_domain__batch_plan__evaluate_admission[[evaluate_admission]]
  end
  subgraph domain_domain_module_error["domain::error"]
    direction TB
  subgraph T29_domain_domain_ValidationError["error::ValidationError"]
    direction TB
    T29_domain_domain_ValidationError__self[ValidationError]
    T29_domain_domain_ValidationError_EmptyString[EmptyString]
    T29_domain_domain_ValidationError_InvalidTrackId[InvalidTrackId]
    T29_domain_domain_ValidationError_InvalidTaskId[InvalidTaskId]
    T29_domain_domain_ValidationError_InvalidCommitHash[InvalidCommitHash]
    T29_domain_domain_ValidationError_InvalidTimestamp[InvalidTimestamp]
    T29_domain_domain_ValidationError_InvalidTrackBranch[InvalidTrackBranch]
    T29_domain_domain_ValidationError_BranchIdMismatch[BranchIdMismatch]
    T29_domain_domain_ValidationError_StatusOverrideMismatch[StatusOverrideMismatch]
    T29_domain_domain_ValidationError_EmptyTrackTitle[EmptyTrackTitle]
    T29_domain_domain_ValidationError_EmptyTaskDescription[EmptyTaskDescription]
    T29_domain_domain_ValidationError_EmptyPlanSectionId[EmptyPlanSectionId]
    T29_domain_domain_ValidationError_EmptyPlanSectionTitle[EmptyPlanSectionTitle]
    T29_domain_domain_ValidationError_DuplicateTaskId[DuplicateTaskId]
    T29_domain_domain_ValidationError_DuplicatePlanSectionId[DuplicatePlanSectionId]
    T29_domain_domain_ValidationError_UnknownTaskReference[UnknownTaskReference]
    T29_domain_domain_ValidationError_DuplicateTaskReference[DuplicateTaskReference]
    T29_domain_domain_ValidationError_UnreferencedTask[UnreferencedTask]
    T29_domain_domain_ValidationError_OverrideIncompatibleWithResolvedTasks[OverrideIncompatibleWithResolvedTasks]
    T29_domain_domain_ValidationError_TrackActivationRequiresPlanningOnly[TrackActivationRequiresPlanningOnly]
    T29_domain_domain_ValidationError_TrackActivationRequiresSchemaV3[TrackActivationRequiresSchemaV3]
    T29_domain_domain_ValidationError_TrackAlreadyMaterialized[TrackAlreadyMaterialized]
    T29_domain_domain_ValidationError_UnsupportedTargetStatus[UnsupportedTargetStatus]
    T29_domain_domain_ValidationError_SectionNotFound[SectionNotFound]
    T29_domain_domain_ValidationError_NoSectionsAvailable[NoSectionsAvailable]
    T29_domain_domain_ValidationError_TaskDescriptionMutated[TaskDescriptionMutated]
    T29_domain_domain_ValidationError_TaskRemoved[TaskRemoved]
    T29_domain_domain_ValidationError_DuplicateElementId[DuplicateElementId]
    T29_domain_domain_ValidationError_InvalidLayerId[InvalidLayerId]
    T29_domain_domain_ValidationError_InvalidSpecElementId[InvalidSpecElementId]
    T29_domain_domain_ValidationError_EmptyAdrAnchor[EmptyAdrAnchor]
    T29_domain_domain_ValidationError_EmptyConventionAnchor[EmptyConventionAnchor]
    T29_domain_domain_ValidationError_InvalidContentHash[InvalidContentHash]
    T29_domain_domain_ValidationError_EmptyInformalGroundSummary[EmptyInformalGroundSummary]
    T29_domain_domain_ValidationError_MultiLineInformalGroundSummary[MultiLineInformalGroundSummary]
    T29_domain_domain_ValidationError_EmptyDecisionGroundRef[EmptyDecisionGroundRef]
    T29_domain_domain_ValidationError_InvalidObligationMinimum[InvalidObligationMinimum]
    T29_domain_domain_ValidationError_InvalidDetectionRate[InvalidDetectionRate]
    T29_domain_domain_ValidationError_UnknownDependencyReference[UnknownDependencyReference]
    T29_domain_domain_ValidationError_DependencyCycle[DependencyCycle]
    T29_domain_domain_ValidationError_PlanOrderViolatesDependency[PlanOrderViolatesDependency]
  end
  end
  subgraph domain_domain_module_review_v2["domain::review_v2"]
    direction TB
  subgraph T31_domain_domain_ReviewScopeConfig["review_v2::scope_config::ReviewScopeConfig"]
    direction TB
    T31_domain_domain_ReviewScopeConfig__self[ReviewScopeConfig]
    T31_domain_domain_ReviewScopeConfig_new([new])
    T31_domain_domain_ReviewScopeConfig_diff_ceiling_for_scope([diff_ceiling_for_scope])
    T31_domain_domain_ReviewScopeConfig_classify([classify])
    T31_domain_domain_ReviewScopeConfig_get_scope_names([get_scope_names])
    T31_domain_domain_ReviewScopeConfig_contains_scope([contains_scope])
    T31_domain_domain_ReviewScopeConfig_all_scope_names([all_scope_names])
    T31_domain_domain_ReviewScopeConfig_briefing_file_for_scope([briefing_file_for_scope])
  end
  end
  subgraph domain_domain_module_track["domain::track"]
    direction TB
  subgraph T23_domain_domain_TrackTask["track::TrackTask"]
    direction TB
    T23_domain_domain_TrackTask__self[TrackTask]
    T23_domain_domain_TrackTask_new([new])
    T23_domain_domain_TrackTask_with_status([with_status])
    T23_domain_domain_TrackTask_with_dependencies([with_dependencies])
    T23_domain_domain_TrackTask_depends_on([depends_on])
    T23_domain_domain_TrackTask_id([id])
    T23_domain_domain_TrackTask_description([description])
    T23_domain_domain_TrackTask_status([status])
    T23_domain_domain_TrackTask_transition([transition])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_batch_plan["usecase::batch_plan"]
    direction TB
  subgraph T37_usecase_usecase_BatchPlanCheckCommand["batch_plan::BatchPlanCheckCommand"]
    direction TB
    T37_usecase_usecase_BatchPlanCheckCommand__self[BatchPlanCheckCommand]
  end
  subgraph T35_usecase_usecase_BatchPlanCheckError["batch_plan::BatchPlanCheckError"]
    direction TB
    T35_usecase_usecase_BatchPlanCheckError__self[BatchPlanCheckError]
    T35_usecase_usecase_BatchPlanCheckError_BatchPlanNotFound[BatchPlanNotFound]
    T35_usecase_usecase_BatchPlanCheckError_BatchPlanReadFailed[BatchPlanReadFailed]
    T35_usecase_usecase_BatchPlanCheckError_ImplPlanNotFound[ImplPlanNotFound]
    T35_usecase_usecase_BatchPlanCheckError_ImplPlanReadFailed[ImplPlanReadFailed]
    T35_usecase_usecase_BatchPlanCheckError_ScopeConfigReadFailed[ScopeConfigReadFailed]
  end
  subgraph T40_usecase_usecase_BatchPlanCheckInteractor["batch_plan::BatchPlanCheckInteractor"]
    direction TB
    T40_usecase_usecase_BatchPlanCheckInteractor__self[BatchPlanCheckInteractor]
    T40_usecase_usecase_BatchPlanCheckInteractor_new([new])
  end
  subgraph T34_usecase_usecase_BatchPlanReadError["batch_plan::BatchPlanReadError"]
    direction TB
    T34_usecase_usecase_BatchPlanReadError__self[BatchPlanReadError]
    T34_usecase_usecase_BatchPlanReadError_NotFound[NotFound]
    T34_usecase_usecase_BatchPlanReadError_ReadFailed[ReadFailed]
  end
  subgraph T36_usecase_usecase_PlannedTaskReadError["batch_plan::PlannedTaskReadError"]
    direction TB
    T36_usecase_usecase_PlannedTaskReadError__self[PlannedTaskReadError]
    T36_usecase_usecase_PlannedTaskReadError_NotFound[NotFound]
    T36_usecase_usecase_PlannedTaskReadError_ReadFailed[ReadFailed]
  end
  subgraph T36_usecase_usecase_ScopeConfigReadError["batch_plan::ScopeConfigReadError"]
    direction TB
    T36_usecase_usecase_ScopeConfigReadError__self[ScopeConfigReadError]
    T36_usecase_usecase_ScopeConfigReadError_ReadFailed[ReadFailed]
  end
  subgraph T37_usecase_usecase_ScopeDiffMeasureError["batch_plan::ScopeDiffMeasureError"]
    direction TB
    T37_usecase_usecase_ScopeDiffMeasureError__self[ScopeDiffMeasureError]
    T37_usecase_usecase_ScopeDiffMeasureError_MeasureFailed[MeasureFailed]
  end
  subgraph R37_usecase_usecase_BatchPlanCheckService["batch_plan::BatchPlanCheckService"]
    direction TB
    R37_usecase_usecase_BatchPlanCheckService__self[BatchPlanCheckService]
    R37_usecase_usecase_BatchPlanCheckService_check([check])
  end
  subgraph R35_usecase_usecase_BatchPlanReaderPort["batch_plan::BatchPlanReaderPort"]
    direction TB
    R35_usecase_usecase_BatchPlanReaderPort__self[BatchPlanReaderPort]
    R35_usecase_usecase_BatchPlanReaderPort_read([read])
  end
  subgraph R37_usecase_usecase_PlannedTaskReaderPort["batch_plan::PlannedTaskReaderPort"]
    direction TB
    R37_usecase_usecase_PlannedTaskReaderPort__self[PlannedTaskReaderPort]
    R37_usecase_usecase_PlannedTaskReaderPort_read_planned_tasks([read_planned_tasks])
  end
  subgraph R37_usecase_usecase_ScopeConfigReaderPort["batch_plan::ScopeConfigReaderPort"]
    direction TB
    R37_usecase_usecase_ScopeConfigReaderPort__self[ScopeConfigReaderPort]
    R37_usecase_usecase_ScopeConfigReaderPort_read([read])
  end
  subgraph R36_usecase_usecase_ScopeDiffMeasurePort["batch_plan::ScopeDiffMeasurePort"]
    direction TB
    R36_usecase_usecase_ScopeDiffMeasurePort__self[ScopeDiffMeasurePort]
    R36_usecase_usecase_ScopeDiffMeasurePort_measure_scope_diff([measure_scope_diff])
  end
  end
  subgraph usecase_usecase_module_task_ops["usecase::task_ops"]
    direction TB
  subgraph T39_usecase_usecase_TaskOperationInteractor["task_ops::TaskOperationInteractor"]
    direction TB
    T39_usecase_usecase_TaskOperationInteractor__self[TaskOperationInteractor]
    T39_usecase_usecase_TaskOperationInteractor_new([new])
  end
  subgraph T37_usecase_usecase_TaskTransitionOutcome["task_ops::TaskTransitionOutcome"]
    direction TB
    T37_usecase_usecase_TaskTransitionOutcome__self[TaskTransitionOutcome]
    T37_usecase_usecase_TaskTransitionOutcome_Transitioned[Transitioned]
    T37_usecase_usecase_TaskTransitionOutcome_Rejected[Rejected]
    T37_usecase_usecase_TaskTransitionOutcome_rejection([rejection])
  end
  subgraph R36_usecase_usecase_TaskOperationService["task_ops::TaskOperationService"]
    direction TB
    R36_usecase_usecase_TaskOperationService__self[TaskOperationService]
    R36_usecase_usecase_TaskOperationService_transition_task([transition_task])
    R36_usecase_usecase_TaskOperationService_add_task([add_task])
    R36_usecase_usecase_TaskOperationService_set_override([set_override])
    R36_usecase_usecase_TaskOperationService_clear_override([clear_override])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_batch_plan_codec["infrastructure::batch_plan_codec"]
    direction TB
  subgraph T49_infrastructure_infrastructure_BatchDeclarationDto["batch_plan_codec::BatchDeclarationDto"]
    direction TB
    T49_infrastructure_infrastructure_BatchDeclarationDto__self[BatchDeclarationDto]
  end
  subgraph T49_infrastructure_infrastructure_BatchPlanCodecError["batch_plan_codec::BatchPlanCodecError"]
    direction TB
    T49_infrastructure_infrastructure_BatchPlanCodecError__self[BatchPlanCodecError]
    T49_infrastructure_infrastructure_BatchPlanCodecError_InvalidJson[InvalidJson]
    T49_infrastructure_infrastructure_BatchPlanCodecError_UnsupportedSchemaVersion[UnsupportedSchemaVersion]
    T49_infrastructure_infrastructure_BatchPlanCodecError_InvalidDocument[InvalidDocument]
  end
  subgraph T50_infrastructure_infrastructure_BatchPlanDocumentDto["batch_plan_codec::BatchPlanDocumentDto"]
    direction TB
    T50_infrastructure_infrastructure_BatchPlanDocumentDto__self[BatchPlanDocumentDto]
  end
  subgraph T50_infrastructure_infrastructure_ScopeLineEstimateDto["batch_plan_codec::ScopeLineEstimateDto"]
    direction TB
    T50_infrastructure_infrastructure_ScopeLineEstimateDto__self[ScopeLineEstimateDto]
  end
  subgraph T45_infrastructure_infrastructure_TaskEstimateDto["batch_plan_codec::TaskEstimateDto"]
    direction TB
    T45_infrastructure_infrastructure_TaskEstimateDto__self[TaskEstimateDto]
  end
  F70_infrastructure_infrastructure_infrastructure__batch_plan_codec__decode[[decode]]
  end
  subgraph infrastructure_infrastructure_module_batch_plan_reader["infrastructure::batch_plan_reader"]
    direction TB
  subgraph T47_infrastructure_infrastructure_FsBatchPlanReader["batch_plan_reader::FsBatchPlanReader"]
    direction TB
    T47_infrastructure_infrastructure_FsBatchPlanReader__self[FsBatchPlanReader]
    T47_infrastructure_infrastructure_FsBatchPlanReader_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_impl_plan_codec["infrastructure::impl_plan_codec"]
    direction TB
  subgraph T45_infrastructure_infrastructure_ImplPlanTaskDto["impl_plan_codec::ImplPlanTaskDto"]
    direction TB
    T45_infrastructure_infrastructure_ImplPlanTaskDto__self[ImplPlanTaskDto]
  end
  end
  subgraph infrastructure_infrastructure_module_planned_task_reader["infrastructure::planned_task_reader"]
    direction TB
  subgraph T49_infrastructure_infrastructure_FsPlannedTaskReader["planned_task_reader::FsPlannedTaskReader"]
    direction TB
    T49_infrastructure_infrastructure_FsPlannedTaskReader__self[FsPlannedTaskReader]
    T49_infrastructure_infrastructure_FsPlannedTaskReader_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_review_scope_config_reader["infrastructure::review_scope_config_reader"]
    direction TB
  subgraph T55_infrastructure_infrastructure_FsReviewScopeConfigReader["review_scope_config_reader::FsReviewScopeConfigReader"]
    direction TB
    T55_infrastructure_infrastructure_FsReviewScopeConfigReader__self[FsReviewScopeConfigReader]
    T55_infrastructure_infrastructure_FsReviewScopeConfigReader_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_scope_diff_measure["infrastructure::scope_diff_measure"]
    direction TB
  subgraph T50_infrastructure_infrastructure_GitScopeDiffMeasurer["scope_diff_measure::GitScopeDiffMeasurer"]
    direction TB
    T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self[GitScopeDiffMeasurer]
    T50_infrastructure_infrastructure_GitScopeDiffMeasurer_new([new])
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_batch_plan["cli_driver::batch_plan"]
    direction TB
  subgraph T37_cli_driver_cli_driver_BatchPlanDriver["batch_plan::BatchPlanDriver"]
    direction TB
    T37_cli_driver_cli_driver_BatchPlanDriver__self[BatchPlanDriver]
    T37_cli_driver_cli_driver_BatchPlanDriver_new([new])
    T37_cli_driver_cli_driver_BatchPlanDriver_handle([handle])
  end
  subgraph T36_cli_driver_cli_driver_BatchPlanInput["batch_plan::BatchPlanInput"]
    direction TB
    T36_cli_driver_cli_driver_BatchPlanInput__self[BatchPlanInput]
    T36_cli_driver_cli_driver_BatchPlanInput_Check[Check]
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_batch_plan["cli_composition::batch_plan"]
    direction TB
  subgraph T56_cli_composition_cli_composition_BatchPlanCompositionRoot["batch_plan::BatchPlanCompositionRoot"]
    direction TB
    T56_cli_composition_cli_composition_BatchPlanCompositionRoot__self[BatchPlanCompositionRoot]
    T56_cli_composition_cli_composition_BatchPlanCompositionRoot_new([new])
    T56_cli_composition_cli_composition_BatchPlanCompositionRoot_batch_plan_driver([batch_plan_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
  subgraph T18_cli_cli_CliCommand["CliCommand"]
    direction TB
    T18_cli_cli_CliCommand__self[CliCommand]
    T18_cli_cli_CliCommand_Arch[Arch]
    T18_cli_cli_CliCommand_AdrBaseline[AdrBaseline]
    T18_cli_cli_CliCommand_Conventions[Conventions]
    T18_cli_cli_CliCommand_Domain[Domain]
    T18_cli_cli_CliCommand_Guard[Guard]
    T18_cli_cli_CliCommand_Hook[Hook]
    T18_cli_cli_CliCommand_Maintenance[Maintenance]
    T18_cli_cli_CliCommand_Track[Track]
    T18_cli_cli_CliCommand_Git[Git]
    T18_cli_cli_CliCommand_Pr[Pr]
    T18_cli_cli_CliCommand_Capability[Capability]
    T18_cli_cli_CliCommand_Review[Review]
    T18_cli_cli_CliCommand_File[File]
    T18_cli_cli_CliCommand_Verify[Verify]
    T18_cli_cli_CliCommand_FindSimilar[FindSimilar]
    T18_cli_cli_CliCommand_DupIndex[DupIndex]
    T18_cli_cli_CliCommand_DupCheck[DupCheck]
    T18_cli_cli_CliCommand_Telemetry[Telemetry]
    T18_cli_cli_CliCommand_Dry[Dry]
    T18_cli_cli_CliCommand_RefVerify[RefVerify]
    T18_cli_cli_CliCommand_TestObligation[TestObligation]
    T18_cli_cli_CliCommand_Signal[Signal]
    T18_cli_cli_CliCommand_TaskContract[TaskContract]
    T18_cli_cli_CliCommand_Catalog[Catalog]
    T18_cli_cli_CliCommand_CatalogueLint[CatalogueLint]
    T18_cli_cli_CliCommand_Template[Template]
    T18_cli_cli_CliCommand_CodexRuntime[CodexRuntime]
    T18_cli_cli_CliCommand_BatchPlan[BatchPlan]
  end
  subgraph cli_cli_module_commands["cli::commands"]
    direction TB
  subgraph T26_cli_cli_BatchPlanCheckArgs["commands::batch_plan::BatchPlanCheckArgs"]
    direction TB
    T26_cli_cli_BatchPlanCheckArgs__self[BatchPlanCheckArgs]
  end
  subgraph T24_cli_cli_BatchPlanCommand["commands::batch_plan::BatchPlanCommand"]
    direction TB
    T24_cli_cli_BatchPlanCommand__self[BatchPlanCommand]
    T24_cli_cli_BatchPlanCommand_Check[Check]
  end
  F42_cli_cli_cli__commands__batch_plan__execute[[execute]]
  end
end
T31_domain_domain_AdmissionDecision_Rejected --o T32_domain_domain_AdmissionRejection__self
T31_domain_domain_AdmissionDecision_rejection --> T32_domain_domain_AdmissionRejection__self
T32_domain_domain_AdmissionRejection_NotCurrentBatchMember --o|task_batch| T21_domain_domain_BatchId__self
T32_domain_domain_AdmissionRejection_NotCurrentBatchMember --o|current_batch| T21_domain_domain_BatchId__self
T32_domain_domain_AdmissionRejection_ScopeCeilingWouldBeExceeded --o|prior_contribution| T30_domain_domain_NonZeroLineCount__self
T32_domain_domain_AdmissionRejection_ScopeCeilingWouldBeExceeded --o|candidate_estimate| T23_domain_domain_LineCount__self
T32_domain_domain_AdmissionRejection_ScopeCeilingWouldBeExceeded --o|ceiling| T23_domain_domain_LineCount__self
T30_domain_domain_BatchDeclaration_new --o T21_domain_domain_BatchId__self
T30_domain_domain_BatchDeclaration_new --> T30_domain_domain_BatchDeclaration__self
T30_domain_domain_BatchDeclaration_new --> T38_domain_domain_BatchPlanValidationError__self
T30_domain_domain_BatchDeclaration_id --> T21_domain_domain_BatchId__self
T21_domain_domain_BatchId_try_new --> T21_domain_domain_BatchId__self
T21_domain_domain_BatchId_try_new --> T38_domain_domain_BatchPlanValidationError__self
T31_domain_domain_BatchPlanDocument_new --o T26_domain_domain_TaskEstimate__self
T31_domain_domain_BatchPlanDocument_new --o T30_domain_domain_BatchDeclaration__self
T31_domain_domain_BatchPlanDocument_new --> T31_domain_domain_BatchPlanDocument__self
T31_domain_domain_BatchPlanDocument_new --> T38_domain_domain_BatchPlanValidationError__self
T31_domain_domain_BatchPlanDocument_task_estimates --> T26_domain_domain_TaskEstimate__self
T31_domain_domain_BatchPlanDocument_batches --> T30_domain_domain_BatchDeclaration__self
T31_domain_domain_BatchPlanDocument_estimate_for --> T26_domain_domain_TaskEstimate__self
T31_domain_domain_BatchPlanDocument_batch_of --> T30_domain_domain_BatchDeclaration__self
T31_domain_domain_BatchPlanDocument_scope_total --o T30_domain_domain_BatchDeclaration__self
T31_domain_domain_BatchPlanDocument_scope_total --> T23_domain_domain_LineCount__self
T31_domain_domain_BatchPlanDocument_current_batch --> T30_domain_domain_BatchDeclaration__self
T34_domain_domain_BatchPlanGateOutcome_Blocked --o|violations| T36_domain_domain_NonEmptyGateViolations__self
T34_domain_domain_BatchPlanGateOutcome_from_violations --o T36_domain_domain_BatchPlanGateViolation__self
T34_domain_domain_BatchPlanGateOutcome_from_violations --> T34_domain_domain_BatchPlanGateOutcome__self
T34_domain_domain_BatchPlanGateOutcome_violations --> T36_domain_domain_NonEmptyGateViolations__self
T36_domain_domain_BatchPlanGateViolation_CeilingExceeded --o|batch_id| T21_domain_domain_BatchId__self
T36_domain_domain_BatchPlanGateViolation_CeilingExceeded --o|total| T23_domain_domain_LineCount__self
T36_domain_domain_BatchPlanGateViolation_CeilingExceeded --o|ceiling| T23_domain_domain_LineCount__self
T36_domain_domain_BatchPlanGateViolation_OversizeScopeHasMultipleContributors --o|batch_id| T21_domain_domain_BatchId__self
T36_domain_domain_BatchPlanGateViolation_DependencyInLaterBatch --o|task_batch| T21_domain_domain_BatchId__self
T36_domain_domain_BatchPlanGateViolation_DependencyInLaterBatch --o|dependency_batch| T21_domain_domain_BatchId__self
T38_domain_domain_BatchPlanValidationError_EmptyBatch --o|batch_id| T21_domain_domain_BatchId__self
T38_domain_domain_BatchPlanValidationError_DuplicateBatchId --o|batch_id| T21_domain_domain_BatchId__self
T38_domain_domain_BatchPlanValidationError_DuplicateBatchMembership --o|batch_ids| T21_domain_domain_BatchId__self
T41_domain_domain_IndivisibilityJustification_try_new --> T38_domain_domain_BatchPlanValidationError__self
T41_domain_domain_IndivisibilityJustification_try_new --> T41_domain_domain_IndivisibilityJustification__self
T23_domain_domain_LineCount_new --> T23_domain_domain_LineCount__self
T23_domain_domain_LineCount_saturating_add --o T23_domain_domain_LineCount__self
T23_domain_domain_LineCount_saturating_add --> T23_domain_domain_LineCount__self
T31_domain_domain_MeasuredScopeDiff_new --o T23_domain_domain_LineCount__self
T31_domain_domain_MeasuredScopeDiff_new --> T31_domain_domain_MeasuredScopeDiff__self
T31_domain_domain_MeasuredScopeDiff_lines --> T23_domain_domain_LineCount__self
T36_domain_domain_NonEmptyGateViolations_try_new --o T36_domain_domain_BatchPlanGateViolation__self
T36_domain_domain_NonEmptyGateViolations_try_new --> T36_domain_domain_NonEmptyGateViolations__self
T36_domain_domain_NonEmptyGateViolations_as_slice --> T36_domain_domain_BatchPlanGateViolation__self
T36_domain_domain_NonEmptyGateViolations_into_vec --> T36_domain_domain_BatchPlanGateViolation__self
T30_domain_domain_NonZeroLineCount_try_new --o T23_domain_domain_LineCount__self
T30_domain_domain_NonZeroLineCount_try_new --> T30_domain_domain_NonZeroLineCount__self
T30_domain_domain_NonZeroLineCount_get --> T23_domain_domain_LineCount__self
T26_domain_domain_ScopeCeiling_Limited --o T23_domain_domain_LineCount__self
T26_domain_domain_ScopeCeiling_resolve --> T26_domain_domain_ScopeCeiling__self
T26_domain_domain_ScopeCeiling_admits --o T23_domain_domain_LineCount__self
T26_domain_domain_ScopeCeiling_limit --> T23_domain_domain_LineCount__self
T31_domain_domain_ScopeLineEstimate_new --o T23_domain_domain_LineCount__self
T31_domain_domain_ScopeLineEstimate_new --o T23_domain_domain_LineCount__self
T31_domain_domain_ScopeLineEstimate_new --> T31_domain_domain_ScopeLineEstimate__self
T31_domain_domain_ScopeLineEstimate_production_lines --> T23_domain_domain_LineCount__self
T31_domain_domain_ScopeLineEstimate_test_lines --> T23_domain_domain_LineCount__self
T31_domain_domain_ScopeLineEstimate_total --> T23_domain_domain_LineCount__self
T31_domain_domain_TaskDecomposition_Indivisible --o T41_domain_domain_IndivisibilityJustification__self
T31_domain_domain_TaskDecomposition_justification --> T41_domain_domain_IndivisibilityJustification__self
T26_domain_domain_TaskEstimate_new --o T31_domain_domain_ScopeLineEstimate__self
T26_domain_domain_TaskEstimate_new --o T31_domain_domain_TaskDecomposition__self
T26_domain_domain_TaskEstimate_new --> T38_domain_domain_BatchPlanValidationError__self
T26_domain_domain_TaskEstimate_new --> T26_domain_domain_TaskEstimate__self
T26_domain_domain_TaskEstimate_scope_estimates --> T31_domain_domain_ScopeLineEstimate__self
T26_domain_domain_TaskEstimate_decomposition --> T31_domain_domain_TaskDecomposition__self
T26_domain_domain_TaskEstimate_estimate_for --> T31_domain_domain_ScopeLineEstimate__self
F50_domain_domain_domain__batch_plan__check_batch_plan --o T31_domain_domain_BatchPlanDocument__self
F50_domain_domain_domain__batch_plan__check_batch_plan --o T31_domain_domain_ReviewScopeConfig__self
F50_domain_domain_domain__batch_plan__check_batch_plan --o T23_domain_domain_TrackTask__self
F50_domain_domain_domain__batch_plan__check_batch_plan --> T34_domain_domain_BatchPlanGateOutcome__self
F52_domain_domain_domain__batch_plan__evaluate_admission --o T31_domain_domain_BatchPlanDocument__self
F52_domain_domain_domain__batch_plan__evaluate_admission --o T31_domain_domain_ReviewScopeConfig__self
F52_domain_domain_domain__batch_plan__evaluate_admission --o T31_domain_domain_MeasuredScopeDiff__self
F52_domain_domain_domain__batch_plan__evaluate_admission --> T31_domain_domain_AdmissionDecision__self
F52_domain_domain_domain__batch_plan__evaluate_admission --> T38_domain_domain_AdmissionEvaluationError__self
T31_domain_domain_ReviewScopeConfig_new --> T31_domain_domain_ReviewScopeConfig__self
T23_domain_domain_TrackTask_new --> T23_domain_domain_TrackTask__self
T23_domain_domain_TrackTask_new --> T29_domain_domain_ValidationError__self
T23_domain_domain_TrackTask_with_status --> T23_domain_domain_TrackTask__self
T23_domain_domain_TrackTask_with_status --> T29_domain_domain_ValidationError__self
T23_domain_domain_TrackTask_with_dependencies --> T23_domain_domain_TrackTask__self
T40_usecase_usecase_BatchPlanCheckInteractor_new --o R35_usecase_usecase_BatchPlanReaderPort__self
T40_usecase_usecase_BatchPlanCheckInteractor_new --o R37_usecase_usecase_PlannedTaskReaderPort__self
T40_usecase_usecase_BatchPlanCheckInteractor_new --o R37_usecase_usecase_ScopeConfigReaderPort__self
T40_usecase_usecase_BatchPlanCheckInteractor_new --> T40_usecase_usecase_BatchPlanCheckInteractor__self
R37_usecase_usecase_BatchPlanCheckService_check --o T37_usecase_usecase_BatchPlanCheckCommand__self
R37_usecase_usecase_BatchPlanCheckService_check --> T35_usecase_usecase_BatchPlanCheckError__self
R37_usecase_usecase_BatchPlanCheckService_check --> T34_domain_domain_BatchPlanGateOutcome__self
R35_usecase_usecase_BatchPlanReaderPort_read --> T34_usecase_usecase_BatchPlanReadError__self
R35_usecase_usecase_BatchPlanReaderPort_read --> T31_domain_domain_BatchPlanDocument__self
R37_usecase_usecase_PlannedTaskReaderPort_read_planned_tasks --> T36_usecase_usecase_PlannedTaskReadError__self
R37_usecase_usecase_PlannedTaskReaderPort_read_planned_tasks --> T23_domain_domain_TrackTask__self
R37_usecase_usecase_ScopeConfigReaderPort_read --> T36_usecase_usecase_ScopeConfigReadError__self
R37_usecase_usecase_ScopeConfigReaderPort_read --> T31_domain_domain_ReviewScopeConfig__self
R36_usecase_usecase_ScopeDiffMeasurePort_measure_scope_diff --> T37_usecase_usecase_ScopeDiffMeasureError__self
R36_usecase_usecase_ScopeDiffMeasurePort_measure_scope_diff --> T31_domain_domain_MeasuredScopeDiff__self
T39_usecase_usecase_TaskOperationInteractor_new --o R35_usecase_usecase_BatchPlanReaderPort__self
T39_usecase_usecase_TaskOperationInteractor_new --o R36_usecase_usecase_ScopeDiffMeasurePort__self
T39_usecase_usecase_TaskOperationInteractor_new --o R37_usecase_usecase_ScopeConfigReaderPort__self
T39_usecase_usecase_TaskOperationInteractor_new --> T39_usecase_usecase_TaskOperationInteractor__self
T37_usecase_usecase_TaskTransitionOutcome_Rejected --o T32_domain_domain_AdmissionRejection__self
T37_usecase_usecase_TaskTransitionOutcome_rejection --> T32_domain_domain_AdmissionRejection__self
R36_usecase_usecase_TaskOperationService_transition_task --> T37_usecase_usecase_TaskTransitionOutcome__self
T39_usecase_usecase_TaskOperationInteractor__self -.impl.-> R36_usecase_usecase_TaskOperationService__self
T40_usecase_usecase_BatchPlanCheckInteractor__self -.impl.-> R37_usecase_usecase_BatchPlanCheckService__self
T49_infrastructure_infrastructure_BatchDeclarationDto__self --o|id| T21_domain_domain_BatchId__self
T49_infrastructure_infrastructure_BatchPlanCodecError_InvalidDocument --o|source| T38_domain_domain_BatchPlanValidationError__self
T50_infrastructure_infrastructure_BatchPlanDocumentDto__self --o|task_estimates| T45_infrastructure_infrastructure_TaskEstimateDto__self
T50_infrastructure_infrastructure_BatchPlanDocumentDto__self --o|batches| T49_infrastructure_infrastructure_BatchDeclarationDto__self
T50_infrastructure_infrastructure_ScopeLineEstimateDto__self --o|production_lines| T23_domain_domain_LineCount__self
T50_infrastructure_infrastructure_ScopeLineEstimateDto__self --o|test_lines| T23_domain_domain_LineCount__self
T45_infrastructure_infrastructure_TaskEstimateDto__self --o|scope_estimates| T50_infrastructure_infrastructure_ScopeLineEstimateDto__self
T45_infrastructure_infrastructure_TaskEstimateDto__self --o|oversize_justification| T41_domain_domain_IndivisibilityJustification__self
F70_infrastructure_infrastructure_infrastructure__batch_plan_codec__decode --> T49_infrastructure_infrastructure_BatchPlanCodecError__self
F70_infrastructure_infrastructure_infrastructure__batch_plan_codec__decode --> T31_domain_domain_BatchPlanDocument__self
T47_infrastructure_infrastructure_FsBatchPlanReader_new --> T47_infrastructure_infrastructure_FsBatchPlanReader__self
T49_infrastructure_infrastructure_FsPlannedTaskReader_new --> T49_infrastructure_infrastructure_FsPlannedTaskReader__self
T55_infrastructure_infrastructure_FsReviewScopeConfigReader_new --> T55_infrastructure_infrastructure_FsReviewScopeConfigReader__self
T50_infrastructure_infrastructure_GitScopeDiffMeasurer_new --> T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self
T47_infrastructure_infrastructure_FsBatchPlanReader__self -.impl.-> R35_usecase_usecase_BatchPlanReaderPort__self
T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self -.impl.-> R36_usecase_usecase_ScopeDiffMeasurePort__self
T49_infrastructure_infrastructure_FsPlannedTaskReader__self -.impl.-> R37_usecase_usecase_PlannedTaskReaderPort__self
T55_infrastructure_infrastructure_FsReviewScopeConfigReader__self -.impl.-> R37_usecase_usecase_ScopeConfigReaderPort__self
T37_cli_driver_cli_driver_BatchPlanDriver_new --o R37_usecase_usecase_BatchPlanCheckService__self
T37_cli_driver_cli_driver_BatchPlanDriver_new --> T37_cli_driver_cli_driver_BatchPlanDriver__self
T37_cli_driver_cli_driver_BatchPlanDriver_handle --o T36_cli_driver_cli_driver_BatchPlanInput__self
T56_cli_composition_cli_composition_BatchPlanCompositionRoot_new --> T56_cli_composition_cli_composition_BatchPlanCompositionRoot__self
T56_cli_composition_cli_composition_BatchPlanCompositionRoot_batch_plan_driver --> T37_cli_driver_cli_driver_BatchPlanDriver__self
T18_cli_cli_CliCommand_BatchPlan --o|cmd| T24_cli_cli_BatchPlanCommand__self
T24_cli_cli_BatchPlanCommand_Check --o T26_cli_cli_BatchPlanCheckArgs__self
F42_cli_cli_cli__commands__batch_plan__execute --o T24_cli_cli_BatchPlanCommand__self
class T31_domain_domain_AdmissionDecision_Admitted variant_node
class T31_domain_domain_AdmissionDecision_Rejected variant_node
class T31_domain_domain_AdmissionDecision_is_admitted method_node
class T31_domain_domain_AdmissionDecision_rejection method_node
class T31_domain_domain_AdmissionDecision__self value_object
class T38_domain_domain_AdmissionEvaluationError_MissingTaskEstimate variant_node
class T38_domain_domain_AdmissionEvaluationError__self error_type
class T32_domain_domain_AdmissionRejection_NotCurrentBatchMember variant_node
class T32_domain_domain_AdmissionRejection_ScopeCeilingWouldBeExceeded variant_node
class T32_domain_domain_AdmissionRejection__self value_object
class T30_domain_domain_BatchDeclaration_new method_node
class T30_domain_domain_BatchDeclaration_id method_node
class T30_domain_domain_BatchDeclaration_task_ids method_node
class T30_domain_domain_BatchDeclaration_contains method_node
class T30_domain_domain_BatchDeclaration__self value_object
class T21_domain_domain_BatchId_try_new method_node
class T21_domain_domain_BatchId_as_str method_node
class T21_domain_domain_BatchId__self value_object
class T31_domain_domain_BatchPlanDocument_new method_node
class T31_domain_domain_BatchPlanDocument_track_id method_node
class T31_domain_domain_BatchPlanDocument_task_estimates method_node
class T31_domain_domain_BatchPlanDocument_batches method_node
class T31_domain_domain_BatchPlanDocument_estimate_for method_node
class T31_domain_domain_BatchPlanDocument_batch_of method_node
class T31_domain_domain_BatchPlanDocument_scope_total method_node
class T31_domain_domain_BatchPlanDocument_current_batch method_node
class T31_domain_domain_BatchPlanDocument__self value_object
class T34_domain_domain_BatchPlanGateOutcome_Passed variant_node
class T34_domain_domain_BatchPlanGateOutcome_Blocked variant_node
class T34_domain_domain_BatchPlanGateOutcome_from_violations method_node
class T34_domain_domain_BatchPlanGateOutcome_violations method_node
class T34_domain_domain_BatchPlanGateOutcome__self value_object
class T36_domain_domain_BatchPlanGateViolation_CeilingExceeded variant_node
class T36_domain_domain_BatchPlanGateViolation_OversizeScopeHasMultipleContributors variant_node
class T36_domain_domain_BatchPlanGateViolation_UnknownTaskRef variant_node
class T36_domain_domain_BatchPlanGateViolation_UnplannedTask variant_node
class T36_domain_domain_BatchPlanGateViolation_DependencyInLaterBatch variant_node
class T36_domain_domain_BatchPlanGateViolation__self value_object
class T38_domain_domain_BatchPlanValidationError_EmptyJustification variant_node
class T38_domain_domain_BatchPlanValidationError_EmptyBatchId variant_node
class T38_domain_domain_BatchPlanValidationError_EmptyBatch variant_node
class T38_domain_domain_BatchPlanValidationError_DuplicateTaskEstimate variant_node
class T38_domain_domain_BatchPlanValidationError_DuplicateScopeEstimate variant_node
class T38_domain_domain_BatchPlanValidationError_DuplicateBatchId variant_node
class T38_domain_domain_BatchPlanValidationError_MissingTaskEstimate variant_node
class T38_domain_domain_BatchPlanValidationError_UnassignedTask variant_node
class T38_domain_domain_BatchPlanValidationError_DuplicateBatchMembership variant_node
class T38_domain_domain_BatchPlanValidationError__self error_type
class T41_domain_domain_IndivisibilityJustification_try_new method_node
class T41_domain_domain_IndivisibilityJustification_as_str method_node
class T41_domain_domain_IndivisibilityJustification__self value_object
class T23_domain_domain_LineCount_new method_node
class T23_domain_domain_LineCount_value method_node
class T23_domain_domain_LineCount_saturating_add method_node
class T23_domain_domain_LineCount__self value_object
class T31_domain_domain_MeasuredScopeDiff_new method_node
class T31_domain_domain_MeasuredScopeDiff_scope method_node
class T31_domain_domain_MeasuredScopeDiff_lines method_node
class T31_domain_domain_MeasuredScopeDiff__self value_object
class T36_domain_domain_NonEmptyGateViolations_try_new method_node
class T36_domain_domain_NonEmptyGateViolations_as_slice method_node
class T36_domain_domain_NonEmptyGateViolations_into_vec method_node
class T36_domain_domain_NonEmptyGateViolations__self value_object
class T30_domain_domain_NonZeroLineCount_try_new method_node
class T30_domain_domain_NonZeroLineCount_get method_node
class T30_domain_domain_NonZeroLineCount__self value_object
class T26_domain_domain_ScopeCeiling_Unconstrained variant_node
class T26_domain_domain_ScopeCeiling_Limited variant_node
class T26_domain_domain_ScopeCeiling_resolve method_node
class T26_domain_domain_ScopeCeiling_admits method_node
class T26_domain_domain_ScopeCeiling_limit method_node
class T26_domain_domain_ScopeCeiling__self value_object
class T31_domain_domain_ScopeLineEstimate_new method_node
class T31_domain_domain_ScopeLineEstimate_scope method_node
class T31_domain_domain_ScopeLineEstimate_production_lines method_node
class T31_domain_domain_ScopeLineEstimate_test_lines method_node
class T31_domain_domain_ScopeLineEstimate_total method_node
class T31_domain_domain_ScopeLineEstimate__self value_object
class T31_domain_domain_TaskDecomposition_Decomposable variant_node
class T31_domain_domain_TaskDecomposition_Indivisible variant_node
class T31_domain_domain_TaskDecomposition_justification method_node
class T31_domain_domain_TaskDecomposition_is_indivisible method_node
class T31_domain_domain_TaskDecomposition__self value_object
class T26_domain_domain_TaskEstimate_new method_node
class T26_domain_domain_TaskEstimate_task_id method_node
class T26_domain_domain_TaskEstimate_scope_estimates method_node
class T26_domain_domain_TaskEstimate_decomposition method_node
class T26_domain_domain_TaskEstimate_estimate_for method_node
class T26_domain_domain_TaskEstimate__self value_object
class F50_domain_domain_domain__batch_plan__check_batch_plan free_function
class F50_domain_domain_domain__batch_plan__check_batch_plan function_node
class F52_domain_domain_domain__batch_plan__evaluate_admission free_function
class F52_domain_domain_domain__batch_plan__evaluate_admission function_node
class T29_domain_domain_ValidationError_EmptyString variant_node
class T29_domain_domain_ValidationError_InvalidTrackId variant_node
class T29_domain_domain_ValidationError_InvalidTaskId variant_node
class T29_domain_domain_ValidationError_InvalidCommitHash variant_node
class T29_domain_domain_ValidationError_InvalidTimestamp variant_node
class T29_domain_domain_ValidationError_InvalidTrackBranch variant_node
class T29_domain_domain_ValidationError_BranchIdMismatch variant_node
class T29_domain_domain_ValidationError_StatusOverrideMismatch variant_node
class T29_domain_domain_ValidationError_EmptyTrackTitle variant_node
class T29_domain_domain_ValidationError_EmptyTaskDescription variant_node
class T29_domain_domain_ValidationError_EmptyPlanSectionId variant_node
class T29_domain_domain_ValidationError_EmptyPlanSectionTitle variant_node
class T29_domain_domain_ValidationError_DuplicateTaskId variant_node
class T29_domain_domain_ValidationError_DuplicatePlanSectionId variant_node
class T29_domain_domain_ValidationError_UnknownTaskReference variant_node
class T29_domain_domain_ValidationError_DuplicateTaskReference variant_node
class T29_domain_domain_ValidationError_UnreferencedTask variant_node
class T29_domain_domain_ValidationError_OverrideIncompatibleWithResolvedTasks variant_node
class T29_domain_domain_ValidationError_TrackActivationRequiresPlanningOnly variant_node
class T29_domain_domain_ValidationError_TrackActivationRequiresSchemaV3 variant_node
class T29_domain_domain_ValidationError_TrackAlreadyMaterialized variant_node
class T29_domain_domain_ValidationError_UnsupportedTargetStatus variant_node
class T29_domain_domain_ValidationError_SectionNotFound variant_node
class T29_domain_domain_ValidationError_NoSectionsAvailable variant_node
class T29_domain_domain_ValidationError_TaskDescriptionMutated variant_node
class T29_domain_domain_ValidationError_TaskRemoved variant_node
class T29_domain_domain_ValidationError_DuplicateElementId variant_node
class T29_domain_domain_ValidationError_InvalidLayerId variant_node
class T29_domain_domain_ValidationError_InvalidSpecElementId variant_node
class T29_domain_domain_ValidationError_EmptyAdrAnchor variant_node
class T29_domain_domain_ValidationError_EmptyConventionAnchor variant_node
class T29_domain_domain_ValidationError_InvalidContentHash variant_node
class T29_domain_domain_ValidationError_EmptyInformalGroundSummary variant_node
class T29_domain_domain_ValidationError_MultiLineInformalGroundSummary variant_node
class T29_domain_domain_ValidationError_EmptyDecisionGroundRef variant_node
class T29_domain_domain_ValidationError_InvalidObligationMinimum variant_node
class T29_domain_domain_ValidationError_InvalidDetectionRate variant_node
class T29_domain_domain_ValidationError_UnknownDependencyReference variant_node
class T29_domain_domain_ValidationError_DependencyCycle variant_node
class T29_domain_domain_ValidationError_PlanOrderViolatesDependency variant_node
class T29_domain_domain_ValidationError__self error_type
class T31_domain_domain_ReviewScopeConfig_new method_node
class T31_domain_domain_ReviewScopeConfig_diff_ceiling_for_scope method_node
class T31_domain_domain_ReviewScopeConfig_classify method_node
class T31_domain_domain_ReviewScopeConfig_get_scope_names method_node
class T31_domain_domain_ReviewScopeConfig_contains_scope method_node
class T31_domain_domain_ReviewScopeConfig_all_scope_names method_node
class T31_domain_domain_ReviewScopeConfig_briefing_file_for_scope method_node
class T31_domain_domain_ReviewScopeConfig__self domain_service
class T23_domain_domain_TrackTask_new method_node
class T23_domain_domain_TrackTask_with_status method_node
class T23_domain_domain_TrackTask_with_dependencies method_node
class T23_domain_domain_TrackTask_depends_on method_node
class T23_domain_domain_TrackTask_id method_node
class T23_domain_domain_TrackTask_description method_node
class T23_domain_domain_TrackTask_status method_node
class T23_domain_domain_TrackTask_transition method_node
class T23_domain_domain_TrackTask__self entity
class T37_usecase_usecase_BatchPlanCheckCommand__self command
class T35_usecase_usecase_BatchPlanCheckError_BatchPlanNotFound variant_node
class T35_usecase_usecase_BatchPlanCheckError_BatchPlanReadFailed variant_node
class T35_usecase_usecase_BatchPlanCheckError_ImplPlanNotFound variant_node
class T35_usecase_usecase_BatchPlanCheckError_ImplPlanReadFailed variant_node
class T35_usecase_usecase_BatchPlanCheckError_ScopeConfigReadFailed variant_node
class T35_usecase_usecase_BatchPlanCheckError__self error_type
class T40_usecase_usecase_BatchPlanCheckInteractor_new method_node
class T40_usecase_usecase_BatchPlanCheckInteractor__self interactor
class T34_usecase_usecase_BatchPlanReadError_NotFound variant_node
class T34_usecase_usecase_BatchPlanReadError_ReadFailed variant_node
class T34_usecase_usecase_BatchPlanReadError__self error_type
class T36_usecase_usecase_PlannedTaskReadError_NotFound variant_node
class T36_usecase_usecase_PlannedTaskReadError_ReadFailed variant_node
class T36_usecase_usecase_PlannedTaskReadError__self error_type
class T36_usecase_usecase_ScopeConfigReadError_ReadFailed variant_node
class T36_usecase_usecase_ScopeConfigReadError__self error_type
class T37_usecase_usecase_ScopeDiffMeasureError_MeasureFailed variant_node
class T37_usecase_usecase_ScopeDiffMeasureError__self error_type
class R37_usecase_usecase_BatchPlanCheckService_check method_node
class R37_usecase_usecase_BatchPlanCheckService__self app_service
class R35_usecase_usecase_BatchPlanReaderPort_read method_node
class R35_usecase_usecase_BatchPlanReaderPort__self secondary_port
class R37_usecase_usecase_PlannedTaskReaderPort_read_planned_tasks method_node
class R37_usecase_usecase_PlannedTaskReaderPort__self secondary_port
class R37_usecase_usecase_ScopeConfigReaderPort_read method_node
class R37_usecase_usecase_ScopeConfigReaderPort__self secondary_port
class R36_usecase_usecase_ScopeDiffMeasurePort_measure_scope_diff method_node
class R36_usecase_usecase_ScopeDiffMeasurePort__self secondary_port
class T39_usecase_usecase_TaskOperationInteractor_new method_node
class T39_usecase_usecase_TaskOperationInteractor__self interactor
class T37_usecase_usecase_TaskTransitionOutcome_Transitioned variant_node
class T37_usecase_usecase_TaskTransitionOutcome_Rejected variant_node
class T37_usecase_usecase_TaskTransitionOutcome_rejection method_node
class T37_usecase_usecase_TaskTransitionOutcome__self dto
class R36_usecase_usecase_TaskOperationService_transition_task method_node
class R36_usecase_usecase_TaskOperationService_add_task method_node
class R36_usecase_usecase_TaskOperationService_set_override method_node
class R36_usecase_usecase_TaskOperationService_clear_override method_node
class R36_usecase_usecase_TaskOperationService__self app_service
class T49_infrastructure_infrastructure_BatchDeclarationDto__self dto
class T49_infrastructure_infrastructure_BatchPlanCodecError_InvalidJson variant_node
class T49_infrastructure_infrastructure_BatchPlanCodecError_UnsupportedSchemaVersion variant_node
class T49_infrastructure_infrastructure_BatchPlanCodecError_InvalidDocument variant_node
class T49_infrastructure_infrastructure_BatchPlanCodecError__self error_type
class T50_infrastructure_infrastructure_BatchPlanDocumentDto__self dto
class T50_infrastructure_infrastructure_ScopeLineEstimateDto__self dto
class T45_infrastructure_infrastructure_TaskEstimateDto__self dto
class F70_infrastructure_infrastructure_infrastructure__batch_plan_codec__decode free_function
class F70_infrastructure_infrastructure_infrastructure__batch_plan_codec__decode function_node
class T47_infrastructure_infrastructure_FsBatchPlanReader_new method_node
class T47_infrastructure_infrastructure_FsBatchPlanReader__self secondary_adapter
class T45_infrastructure_infrastructure_ImplPlanTaskDto__self dto
class T49_infrastructure_infrastructure_FsPlannedTaskReader_new method_node
class T49_infrastructure_infrastructure_FsPlannedTaskReader__self secondary_adapter
class T55_infrastructure_infrastructure_FsReviewScopeConfigReader_new method_node
class T55_infrastructure_infrastructure_FsReviewScopeConfigReader__self secondary_adapter
class T50_infrastructure_infrastructure_GitScopeDiffMeasurer_new method_node
class T50_infrastructure_infrastructure_GitScopeDiffMeasurer__self secondary_adapter
class T37_cli_driver_cli_driver_BatchPlanDriver_new method_node
class T37_cli_driver_cli_driver_BatchPlanDriver_handle method_node
class T37_cli_driver_cli_driver_BatchPlanDriver__self primary_adapter
class T36_cli_driver_cli_driver_BatchPlanInput_Check variant_node
class T36_cli_driver_cli_driver_BatchPlanInput__self dto
class T56_cli_composition_cli_composition_BatchPlanCompositionRoot_new method_node
class T56_cli_composition_cli_composition_BatchPlanCompositionRoot_batch_plan_driver method_node
class T56_cli_composition_cli_composition_BatchPlanCompositionRoot__self composition_root
class T18_cli_cli_CliCommand_Arch variant_node
class T18_cli_cli_CliCommand_AdrBaseline variant_node
class T18_cli_cli_CliCommand_Conventions variant_node
class T18_cli_cli_CliCommand_Domain variant_node
class T18_cli_cli_CliCommand_Guard variant_node
class T18_cli_cli_CliCommand_Hook variant_node
class T18_cli_cli_CliCommand_Maintenance variant_node
class T18_cli_cli_CliCommand_Track variant_node
class T18_cli_cli_CliCommand_Git variant_node
class T18_cli_cli_CliCommand_Pr variant_node
class T18_cli_cli_CliCommand_Capability variant_node
class T18_cli_cli_CliCommand_Review variant_node
class T18_cli_cli_CliCommand_File variant_node
class T18_cli_cli_CliCommand_Verify variant_node
class T18_cli_cli_CliCommand_FindSimilar variant_node
class T18_cli_cli_CliCommand_DupIndex variant_node
class T18_cli_cli_CliCommand_DupCheck variant_node
class T18_cli_cli_CliCommand_Telemetry variant_node
class T18_cli_cli_CliCommand_Dry variant_node
class T18_cli_cli_CliCommand_RefVerify variant_node
class T18_cli_cli_CliCommand_TestObligation variant_node
class T18_cli_cli_CliCommand_Signal variant_node
class T18_cli_cli_CliCommand_TaskContract variant_node
class T18_cli_cli_CliCommand_Catalog variant_node
class T18_cli_cli_CliCommand_CatalogueLint variant_node
class T18_cli_cli_CliCommand_Template variant_node
class T18_cli_cli_CliCommand_CodexRuntime variant_node
class T18_cli_cli_CliCommand_BatchPlan variant_node
class T18_cli_cli_CliCommand__self dto
class T26_cli_cli_BatchPlanCheckArgs__self dto
class T24_cli_cli_BatchPlanCommand_Check variant_node
class T24_cli_cli_BatchPlanCommand__self dto
class F42_cli_cli_cli__commands__batch_plan__execute free_function
class F42_cli_cli_cli__commands__batch_plan__execute function_node
```
