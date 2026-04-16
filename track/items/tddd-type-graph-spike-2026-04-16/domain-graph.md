<!-- Generated from domain TypeGraph — DO NOT EDIT DIRECTLY -->
# domain Type Graph

Types: 110 total, 50 connected, 63 edges (truncated to 50 nodes)

```mermaid
flowchart LR
    classDef structNode fill:#f3e5f5,stroke:#7b1fa2
    classDef enumNode fill:#e1f5fe,stroke:#0288d1

    ActionContradiction[ActionContradiction]:::structNode
    ActionContradictionKind{{ActionContradictionKind}}:::enumNode
    ConfidenceSignal{{ConfidenceSignal}}:::enumNode
    ConsistencyReport[ConsistencyReport]:::structNode
    CoverageResult[CoverageResult]:::structNode
    DomainError{{DomainError}}:::enumNode
    FilePath[FilePath]:::structNode
    FunctionInfo[FunctionInfo]:::structNode
    HearingMode{{HearingMode}}:::enumNode
    HearingRecord[HearingRecord]:::structNode
    HearingSignalDelta[HearingSignalDelta]:::structNode
    HearingSignalSnapshot[HearingSignalSnapshot]:::structNode
    ImplInfo[ImplInfo]:::structNode
    MethodDeclaration[MethodDeclaration]:::structNode
    NonEmptyReviewerFindings[NonEmptyReviewerFindings]:::structNode
    ParamDeclaration[ParamDeclaration]:::structNode
    PlanSection[PlanSection]:::structNode
    PlanView[PlanView]:::structNode
    ReviewScopeConfig[ReviewScopeConfig]:::structNode
    ReviewTarget[ReviewTarget]:::structNode
    ReviewerFinding[ReviewerFinding]:::structNode
    SchemaExport[SchemaExport]:::structNode
    ScopeName{{ScopeName}}:::enumNode
    SignalBasis{{SignalBasis}}:::enumNode
    SignalCounts[SignalCounts]:::structNode
    SpecDocument[SpecDocument]:::structNode
    SpecRequirement[SpecRequirement]:::structNode
    SpecScope[SpecScope]:::structNode
    SpecSection[SpecSection]:::structNode
    SpecStatus{{SpecStatus}}:::enumNode
    SpecValidationError{{SpecValidationError}}:::enumNode
    StatusOverride[StatusOverride]:::structNode
    StatusOverrideKind{{StatusOverrideKind}}:::enumNode
    TaskStatus{{TaskStatus}}:::enumNode
    TaskStatusKind{{TaskStatusKind}}:::enumNode
    TaskTransition{{TaskTransition}}:::enumNode
    Timestamp[Timestamp]:::structNode
    TrackMetadata[TrackMetadata]:::structNode
    TrackStatus{{TrackStatus}}:::enumNode
    TrackTask[TrackTask]:::structNode
    TraitBaselineEntry[TraitBaselineEntry]:::structNode
    TraitImplDecl[TraitImplDecl]:::structNode
    TraitImplEntry[TraitImplEntry]:::structNode
    TraitInfo[TraitInfo]:::structNode
    TraitNode[TraitNode]:::structNode
    TransitionError{{TransitionError}}:::enumNode
    TypeAction{{TypeAction}}:::enumNode
    TypeInfo[TypeInfo]:::structNode
    TypeSignal[TypeSignal]:::structNode
    ValidationError{{ValidationError}}:::enumNode

    ActionContradiction -->|action| TypeAction
    ActionContradiction -->|kind| ActionContradictionKind
    ConsistencyReport -->|contradictions| ActionContradiction
    ConsistencyReport -->|forward_signals| TypeSignal
    FunctionInfo -->|params| ParamDeclaration
    HearingRecord -->|date| Timestamp
    HearingRecord -->|mode| HearingMode
    HearingRecord -->|signal_delta| HearingSignalDelta
    HearingSignalDelta -->|after| HearingSignalSnapshot
    HearingSignalDelta -->|before| HearingSignalSnapshot
    ImplInfo -->|methods| FunctionInfo
    MethodDeclaration -->|params| ParamDeclaration
    NonEmptyReviewerFindings -->|as_slice| ReviewerFinding
    NonEmptyReviewerFindings -->|into_vec| ReviewerFinding
    PlanView -->|sections| PlanSection
    ReviewScopeConfig -->|all_scope_names| ScopeName
    ReviewScopeConfig -->|classify| FilePath
    ReviewScopeConfig -->|classify| ScopeName
    ReviewScopeConfig -->|get_scope_names| ScopeName
    ReviewTarget -->|files| FilePath
    SchemaExport -->|functions| FunctionInfo
    SchemaExport -->|impls| ImplInfo
    SchemaExport -->|traits| TraitInfo
    SchemaExport -->|types| TypeInfo
    SignalBasis -->|signal| ConfidenceSignal
    SpecDocument -->|acceptance_criteria| SpecRequirement
    SpecDocument -->|additional_sections| SpecSection
    SpecDocument -->|approve| SpecValidationError
    SpecDocument -->|approved_at| Timestamp
    SpecDocument -->|constraints| SpecRequirement
    SpecDocument -->|effective_status| SpecStatus
    SpecDocument -->|evaluate_coverage| CoverageResult
    SpecDocument -->|evaluate_signals| SignalCounts
    SpecDocument -->|hearing_history| HearingRecord
    SpecDocument -->|scope| SpecScope
    SpecDocument -->|signals| SignalCounts
    SpecDocument -->|status| SpecStatus
    SpecRequirement -->|signal| ConfidenceSignal
    SpecScope -->|in_scope| SpecRequirement
    SpecScope -->|out_of_scope| SpecRequirement
    StatusOverride -->|kind| StatusOverrideKind
    StatusOverride -->|track_status| TrackStatus
    TaskStatus -->|kind| TaskStatusKind
    TaskTransition -->|target_kind| TaskStatusKind
    TrackMetadata -->|add_task| DomainError
    TrackMetadata -->|next_open_task| TrackTask
    TrackMetadata -->|next_task_id| ValidationError
    TrackMetadata -->|plan| PlanView
    TrackMetadata -->|set_status_override| DomainError
    TrackMetadata -->|status| TrackStatus
    TrackMetadata -->|status_override| StatusOverride
    TrackMetadata -->|tasks| TrackTask
    TrackMetadata -->|transition_task| DomainError
    TrackMetadata -->|validate_descriptions_unchanged| ValidationError
    TrackMetadata -->|validate_no_tasks_removed| ValidationError
    TrackTask -->|status| TaskStatus
    TrackTask -->|transition| TransitionError
    TraitBaselineEntry -->|methods| MethodDeclaration
    TraitImplDecl -->|expected_methods| MethodDeclaration
    TraitImplEntry -->|methods| MethodDeclaration
    TraitInfo -->|methods| FunctionInfo
    TraitNode -->|methods| MethodDeclaration
    TypeSignal -->|signal| ConfidenceSignal
```
