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
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T33_domain_domain_CatalogueLinterRule["tddd::catalogue_linter::CatalogueLinterRule"]
    direction TB
    T33_domain_domain_CatalogueLinterRule__self[CatalogueLinterRule]
    T33_domain_domain_CatalogueLinterRule_new([new])
    T33_domain_domain_CatalogueLinterRule_target([target])
    T33_domain_domain_CatalogueLinterRule_kind([kind])
  end
  subgraph T37_domain_domain_CatalogueLinterRuleKind["tddd::catalogue_linter::CatalogueLinterRuleKind"]
    direction TB
    T37_domain_domain_CatalogueLinterRuleKind__self[CatalogueLinterRuleKind]
    T37_domain_domain_CatalogueLinterRuleKind_FieldEmpty[FieldEmpty]
    T37_domain_domain_CatalogueLinterRuleKind_FieldNonEmpty[FieldNonEmpty]
    T37_domain_domain_CatalogueLinterRuleKind_KindLayerConstraint[KindLayerConstraint]
    T37_domain_domain_CatalogueLinterRuleKind_ReferencedRoleConstraint[ReferencedRoleConstraint]
    T37_domain_domain_CatalogueLinterRuleKind_TraitImplRequired[TraitImplRequired]
    T37_domain_domain_CatalogueLinterRuleKind_NoRoleInMethodSignature[NoRoleInMethodSignature]
    T37_domain_domain_CatalogueLinterRuleKind_MethodReferenceSignature[MethodReferenceSignature]
    T37_domain_domain_CatalogueLinterRuleKind_AccessorSignatureRequired[AccessorSignatureRequired]
    T37_domain_domain_CatalogueLinterRuleKind_FieldElementUniqueAcrossEntries[FieldElementUniqueAcrossEntries]
    T37_domain_domain_CatalogueLinterRuleKind_NoExternalReferenceInMethods[NoExternalReferenceInMethods]
    T37_domain_domain_CatalogueLinterRuleKind_NoPublicField[NoPublicField]
    T37_domain_domain_CatalogueLinterRuleKind_ForbiddenMethodReceiver[ForbiddenMethodReceiver]
    T37_domain_domain_CatalogueLinterRuleKind_ForbidPrimitiveInTypes[ForbidPrimitiveInTypes]
    T37_domain_domain_CatalogueLinterRuleKind_CompositionRootPureDi[CompositionRootPureDi]
    T37_domain_domain_CatalogueLinterRuleKind_discriminant_name([discriminant_name])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_adr_baseline["usecase::adr_baseline"]
    direction TB
  subgraph T34_usecase_usecase_AdrBaselineCommand["adr_baseline::AdrBaselineCommand"]
    direction TB
    T34_usecase_usecase_AdrBaselineCommand__self[AdrBaselineCommand]
    T34_usecase_usecase_AdrBaselineCommand_Snapshot[Snapshot]
    T34_usecase_usecase_AdrBaselineCommand_Restore[Restore]
  end
  subgraph T32_usecase_usecase_AdrBaselineError["adr_baseline::AdrBaselineError"]
    direction TB
    T32_usecase_usecase_AdrBaselineError__self[AdrBaselineError]
    T32_usecase_usecase_AdrBaselineError_Store[Store]
    T32_usecase_usecase_AdrBaselineError_Source[Source]
    T32_usecase_usecase_AdrBaselineError_Clock[Clock]
  end
  subgraph T37_usecase_usecase_AdrBaselineInteractor["adr_baseline::AdrBaselineInteractor"]
    direction TB
    T37_usecase_usecase_AdrBaselineInteractor__self[AdrBaselineInteractor]
    T37_usecase_usecase_AdrBaselineInteractor_new([new])
  end
  subgraph T41_usecase_usecase_AdrBaselineTimestampError["adr_baseline::AdrBaselineTimestampError"]
    direction TB
    T41_usecase_usecase_AdrBaselineTimestampError__self[AdrBaselineTimestampError]
    T41_usecase_usecase_AdrBaselineTimestampError_InvalidTimestamp[InvalidTimestamp]
  end
  subgraph R34_usecase_usecase_AdrBaselineService["adr_baseline::AdrBaselineService"]
    direction TB
    R34_usecase_usecase_AdrBaselineService__self[AdrBaselineService]
    R34_usecase_usecase_AdrBaselineService_execute([execute])
  end
  subgraph R36_usecase_usecase_AdrBaselineStorePort["adr_baseline::AdrBaselineStorePort"]
    direction TB
    R36_usecase_usecase_AdrBaselineStorePort__self[AdrBaselineStorePort]
    R36_usecase_usecase_AdrBaselineStorePort_snapshot([snapshot])
    R36_usecase_usecase_AdrBaselineStorePort_restore([restore])
  end
  subgraph R25_usecase_usecase_ClockPort["adr_baseline::ClockPort"]
    direction TB
    R25_usecase_usecase_ClockPort__self[ClockPort]
    R25_usecase_usecase_ClockPort_now([now])
  end
  end
  subgraph usecase_usecase_module_catalogue_lint_workflow["usecase::catalogue_lint_workflow"]
    direction TB
  subgraph T28_usecase_usecase_LintRuleKind["catalogue_lint_workflow::LintRuleKind"]
    direction TB
    T28_usecase_usecase_LintRuleKind__self[LintRuleKind]
    T28_usecase_usecase_LintRuleKind_FieldEmpty[FieldEmpty]
    T28_usecase_usecase_LintRuleKind_FieldNonEmpty[FieldNonEmpty]
    T28_usecase_usecase_LintRuleKind_KindLayerConstraint[KindLayerConstraint]
    T28_usecase_usecase_LintRuleKind_ReferencedRoleConstraint[ReferencedRoleConstraint]
    T28_usecase_usecase_LintRuleKind_TraitImplRequired[TraitImplRequired]
    T28_usecase_usecase_LintRuleKind_NoRoleInMethodSignature[NoRoleInMethodSignature]
    T28_usecase_usecase_LintRuleKind_MethodReferenceSignature[MethodReferenceSignature]
    T28_usecase_usecase_LintRuleKind_AccessorSignatureRequired[AccessorSignatureRequired]
    T28_usecase_usecase_LintRuleKind_FieldElementUniqueAcrossEntries[FieldElementUniqueAcrossEntries]
    T28_usecase_usecase_LintRuleKind_NoExternalReferenceInMethods[NoExternalReferenceInMethods]
    T28_usecase_usecase_LintRuleKind_NoPublicField[NoPublicField]
    T28_usecase_usecase_LintRuleKind_ForbiddenMethodReceiver[ForbiddenMethodReceiver]
    T28_usecase_usecase_LintRuleKind_ForbidPrimitiveInTypes[ForbidPrimitiveInTypes]
    T28_usecase_usecase_LintRuleKind_CompositionRootPureDi[CompositionRootPureDi]
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_adr_baseline["infrastructure::adr_baseline"]
    direction TB
  subgraph T48_infrastructure_infrastructure_SystemClockAdapter["adr_baseline::SystemClockAdapter"]
    direction TB
    T48_infrastructure_infrastructure_SystemClockAdapter__self[SystemClockAdapter]
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_adr_baseline["cli_driver::adr_baseline"]
    direction TB
  subgraph T39_cli_driver_cli_driver_AdrBaselineDriver["adr_baseline::AdrBaselineDriver"]
    direction TB
    T39_cli_driver_cli_driver_AdrBaselineDriver__self[AdrBaselineDriver]
    T39_cli_driver_cli_driver_AdrBaselineDriver_new([new])
    T39_cli_driver_cli_driver_AdrBaselineDriver_handle([handle])
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_adr_baseline["cli_composition::adr_baseline"]
    direction TB
  subgraph T58_cli_composition_cli_composition_AdrBaselineCompositionRoot["adr_baseline::AdrBaselineCompositionRoot"]
    direction TB
    T58_cli_composition_cli_composition_AdrBaselineCompositionRoot__self[AdrBaselineCompositionRoot]
    T58_cli_composition_cli_composition_AdrBaselineCompositionRoot_new([new])
    T58_cli_composition_cli_composition_AdrBaselineCompositionRoot_adr_baseline_driver([adr_baseline_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
end
T33_domain_domain_CatalogueLinterRule_new --o T37_domain_domain_CatalogueLinterRuleKind__self
T33_domain_domain_CatalogueLinterRule_new --> T33_domain_domain_CatalogueLinterRule__self
T33_domain_domain_CatalogueLinterRule_kind --> T37_domain_domain_CatalogueLinterRuleKind__self
T32_usecase_usecase_AdrBaselineError_Clock --o T41_usecase_usecase_AdrBaselineTimestampError__self
T37_usecase_usecase_AdrBaselineInteractor_new --o R36_usecase_usecase_AdrBaselineStorePort__self
T37_usecase_usecase_AdrBaselineInteractor_new --o R25_usecase_usecase_ClockPort__self
T37_usecase_usecase_AdrBaselineInteractor_new --> T37_usecase_usecase_AdrBaselineInteractor__self
R34_usecase_usecase_AdrBaselineService_execute --o T34_usecase_usecase_AdrBaselineCommand__self
R34_usecase_usecase_AdrBaselineService_execute --> T32_usecase_usecase_AdrBaselineError__self
R25_usecase_usecase_ClockPort_now --> T41_usecase_usecase_AdrBaselineTimestampError__self
T37_usecase_usecase_AdrBaselineInteractor__self -.impl.-> R34_usecase_usecase_AdrBaselineService__self
T48_infrastructure_infrastructure_SystemClockAdapter__self -.impl.-> R25_usecase_usecase_ClockPort__self
T39_cli_driver_cli_driver_AdrBaselineDriver_new --o R34_usecase_usecase_AdrBaselineService__self
T39_cli_driver_cli_driver_AdrBaselineDriver_new --> T39_cli_driver_cli_driver_AdrBaselineDriver__self
T58_cli_composition_cli_composition_AdrBaselineCompositionRoot_new --> T58_cli_composition_cli_composition_AdrBaselineCompositionRoot__self
T58_cli_composition_cli_composition_AdrBaselineCompositionRoot_adr_baseline_driver --> T39_cli_driver_cli_driver_AdrBaselineDriver__self
class T33_domain_domain_CatalogueLinterRule_new method_node
class T33_domain_domain_CatalogueLinterRule_target method_node
class T33_domain_domain_CatalogueLinterRule_kind method_node
class T33_domain_domain_CatalogueLinterRule__self value_object
class T37_domain_domain_CatalogueLinterRuleKind_FieldEmpty variant_node
class T37_domain_domain_CatalogueLinterRuleKind_FieldNonEmpty variant_node
class T37_domain_domain_CatalogueLinterRuleKind_KindLayerConstraint variant_node
class T37_domain_domain_CatalogueLinterRuleKind_ReferencedRoleConstraint variant_node
class T37_domain_domain_CatalogueLinterRuleKind_TraitImplRequired variant_node
class T37_domain_domain_CatalogueLinterRuleKind_NoRoleInMethodSignature variant_node
class T37_domain_domain_CatalogueLinterRuleKind_MethodReferenceSignature variant_node
class T37_domain_domain_CatalogueLinterRuleKind_AccessorSignatureRequired variant_node
class T37_domain_domain_CatalogueLinterRuleKind_FieldElementUniqueAcrossEntries variant_node
class T37_domain_domain_CatalogueLinterRuleKind_NoExternalReferenceInMethods variant_node
class T37_domain_domain_CatalogueLinterRuleKind_NoPublicField variant_node
class T37_domain_domain_CatalogueLinterRuleKind_ForbiddenMethodReceiver variant_node
class T37_domain_domain_CatalogueLinterRuleKind_ForbidPrimitiveInTypes variant_node
class T37_domain_domain_CatalogueLinterRuleKind_CompositionRootPureDi variant_node
class T37_domain_domain_CatalogueLinterRuleKind_discriminant_name method_node
class T37_domain_domain_CatalogueLinterRuleKind__self value_object
class T34_usecase_usecase_AdrBaselineCommand_Snapshot variant_node
class T34_usecase_usecase_AdrBaselineCommand_Restore variant_node
class T34_usecase_usecase_AdrBaselineCommand__self command
class T32_usecase_usecase_AdrBaselineError_Store variant_node
class T32_usecase_usecase_AdrBaselineError_Source variant_node
class T32_usecase_usecase_AdrBaselineError_Clock variant_node
class T32_usecase_usecase_AdrBaselineError__self error_type
class T37_usecase_usecase_AdrBaselineInteractor_new method_node
class T37_usecase_usecase_AdrBaselineInteractor__self interactor
class T41_usecase_usecase_AdrBaselineTimestampError_InvalidTimestamp variant_node
class T41_usecase_usecase_AdrBaselineTimestampError__self error_type
class R34_usecase_usecase_AdrBaselineService_execute method_node
class R34_usecase_usecase_AdrBaselineService__self app_service
class R36_usecase_usecase_AdrBaselineStorePort_snapshot method_node
class R36_usecase_usecase_AdrBaselineStorePort_restore method_node
class R36_usecase_usecase_AdrBaselineStorePort__self secondary_port
class R25_usecase_usecase_ClockPort_now method_node
class R25_usecase_usecase_ClockPort__self secondary_port
class T28_usecase_usecase_LintRuleKind_FieldEmpty variant_node
class T28_usecase_usecase_LintRuleKind_FieldNonEmpty variant_node
class T28_usecase_usecase_LintRuleKind_KindLayerConstraint variant_node
class T28_usecase_usecase_LintRuleKind_ReferencedRoleConstraint variant_node
class T28_usecase_usecase_LintRuleKind_TraitImplRequired variant_node
class T28_usecase_usecase_LintRuleKind_NoRoleInMethodSignature variant_node
class T28_usecase_usecase_LintRuleKind_MethodReferenceSignature variant_node
class T28_usecase_usecase_LintRuleKind_AccessorSignatureRequired variant_node
class T28_usecase_usecase_LintRuleKind_FieldElementUniqueAcrossEntries variant_node
class T28_usecase_usecase_LintRuleKind_NoExternalReferenceInMethods variant_node
class T28_usecase_usecase_LintRuleKind_NoPublicField variant_node
class T28_usecase_usecase_LintRuleKind_ForbiddenMethodReceiver variant_node
class T28_usecase_usecase_LintRuleKind_ForbidPrimitiveInTypes variant_node
class T28_usecase_usecase_LintRuleKind_CompositionRootPureDi variant_node
class T28_usecase_usecase_LintRuleKind__self dto
class T48_infrastructure_infrastructure_SystemClockAdapter__self secondary_adapter
class T39_cli_driver_cli_driver_AdrBaselineDriver_new method_node
class T39_cli_driver_cli_driver_AdrBaselineDriver_handle method_node
class T39_cli_driver_cli_driver_AdrBaselineDriver__self primary_adapter
class T58_cli_composition_cli_composition_AdrBaselineCompositionRoot_new method_node
class T58_cli_composition_cli_composition_AdrBaselineCompositionRoot_adr_baseline_driver method_node
class T58_cli_composition_cli_composition_AdrBaselineCompositionRoot__self composition_root
```
