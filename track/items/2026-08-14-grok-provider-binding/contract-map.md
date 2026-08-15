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
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_capability_exec["usecase::capability_exec"]
    direction TB
  subgraph R38_usecase_usecase_CapabilityProviderPort["capability_exec::CapabilityProviderPort"]
    direction TB
    R38_usecase_usecase_CapabilityProviderPort__self[CapabilityProviderPort]
    R38_usecase_usecase_CapabilityProviderPort_provider([provider])
    R38_usecase_usecase_CapabilityProviderPort_dispatch([dispatch])
  end
  end
  subgraph usecase_usecase_module_dry_check["usecase::dry_check"]
    direction TB
  subgraph R33_usecase_usecase_DryCheckAgentPort["dry_check::ports::DryCheckAgentPort"]
    direction TB
    R33_usecase_usecase_DryCheckAgentPort__self[DryCheckAgentPort]
    R33_usecase_usecase_DryCheckAgentPort_judge([judge])
  end
  end
  subgraph usecase_usecase_module_hook_dispatch["usecase::hook_dispatch"]
    direction TB
  subgraph R35_usecase_usecase_HookDispatchService["hook_dispatch::HookDispatchService"]
    direction TB
    R35_usecase_usecase_HookDispatchService__self[HookDispatchService]
    R35_usecase_usecase_HookDispatchService_dispatch([dispatch])
    R35_usecase_usecase_HookDispatchService_check_skill_compliance([check_skill_compliance])
  end
  end
  subgraph usecase_usecase_module_review_v2["usecase::review_v2"]
    direction TB
  subgraph R31_usecase_usecase_ReviewFixRunner["review_v2::run_review_fix::ReviewFixRunner"]
    direction TB
    R31_usecase_usecase_ReviewFixRunner__self[ReviewFixRunner]
    R31_usecase_usecase_ReviewFixRunner_run_fix([run_fix])
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
  subgraph infrastructure_infrastructure_module_capability_exec["infrastructure::capability_exec"]
    direction TB
  subgraph T51_infrastructure_infrastructure_GrokCapabilityAdapter["capability_exec::grok::GrokCapabilityAdapter"]
    direction TB
    T51_infrastructure_infrastructure_GrokCapabilityAdapter__self[GrokCapabilityAdapter]
    T51_infrastructure_infrastructure_GrokCapabilityAdapter_new([new])
  end
  subgraph T54_infrastructure_infrastructure_GrokCapabilityDefinition["capability_exec::grok::GrokCapabilityDefinition"]
    direction TB
    T54_infrastructure_infrastructure_GrokCapabilityDefinition__self[GrokCapabilityDefinition]
    T54_infrastructure_infrastructure_GrokCapabilityDefinition_model([model])
    T54_infrastructure_infrastructure_GrokCapabilityDefinition_sandbox([sandbox])
    T54_infrastructure_infrastructure_GrokCapabilityDefinition_resolve([resolve])
  end
  end
  subgraph infrastructure_infrastructure_module_dry_check["infrastructure::dry_check"]
    direction TB
  subgraph T52_infrastructure_infrastructure_CodexDryFixLocalRunner["dry_check::dry_fix_local::CodexDryFixLocalRunner"]
    direction TB
    T52_infrastructure_infrastructure_CodexDryFixLocalRunner__self[CodexDryFixLocalRunner]
    T52_infrastructure_infrastructure_CodexDryFixLocalRunner_new([new])
    T52_infrastructure_infrastructure_CodexDryFixLocalRunner_dry_run_fix_local([dry_run_fix_local])
  end
  subgraph T44_infrastructure_infrastructure_GrokDryChecker["dry_check::grok_dry_checker::GrokDryChecker"]
    direction TB
    T44_infrastructure_infrastructure_GrokDryChecker__self[GrokDryChecker]
    T44_infrastructure_infrastructure_GrokDryChecker_new([new])
  end
  end
  subgraph infrastructure_infrastructure_module_grok_common["infrastructure::grok_common"]
    direction TB
  subgraph T47_infrastructure_infrastructure_GrokEnvelopeError["grok_common::GrokEnvelopeError"]
    direction TB
    T47_infrastructure_infrastructure_GrokEnvelopeError__self[GrokEnvelopeError]
    T47_infrastructure_infrastructure_GrokEnvelopeError_ProviderFailure[ProviderFailure]
  end
  subgraph T48_infrastructure_infrastructure_GrokOutputEnvelope["grok_common::GrokOutputEnvelope"]
    direction TB
    T48_infrastructure_infrastructure_GrokOutputEnvelope__self[GrokOutputEnvelope]
    T48_infrastructure_infrastructure_GrokOutputEnvelope_Succeeded[Succeeded]
    T48_infrastructure_infrastructure_GrokOutputEnvelope_Failed[Failed]
    T48_infrastructure_infrastructure_GrokOutputEnvelope_into_structured_output([into_structured_output])
  end
  subgraph T41_infrastructure_infrastructure_GrokSandbox["grok_common::GrokSandbox"]
    direction TB
    T41_infrastructure_infrastructure_GrokSandbox__self[GrokSandbox]
    T41_infrastructure_infrastructure_GrokSandbox_ReadOnly[ReadOnly]
    T41_infrastructure_infrastructure_GrokSandbox_Workspace[Workspace]
    T41_infrastructure_infrastructure_GrokSandbox_Strict[Strict]
    T41_infrastructure_infrastructure_GrokSandbox_ProjectProfile[ProjectProfile]
  end
  subgraph T52_infrastructure_infrastructure_GrokSandboxProfileName["grok_common::GrokSandboxProfileName"]
    direction TB
    T52_infrastructure_infrastructure_GrokSandboxProfileName__self[GrokSandboxProfileName]
    T52_infrastructure_infrastructure_GrokSandboxProfileName_try_new([try_new])
    T52_infrastructure_infrastructure_GrokSandboxProfileName_as_str([as_str])
  end
  subgraph T57_infrastructure_infrastructure_GrokSandboxProfileNameError["grok_common::GrokSandboxProfileNameError"]
    direction TB
    T57_infrastructure_infrastructure_GrokSandboxProfileNameError__self[GrokSandboxProfileNameError]
    T57_infrastructure_infrastructure_GrokSandboxProfileNameError_Empty[Empty]
    T57_infrastructure_infrastructure_GrokSandboxProfileNameError_Reserved[Reserved]
  end
  end
  subgraph infrastructure_infrastructure_module_review_v2["infrastructure::review_v2"]
    direction TB
  subgraph T42_infrastructure_infrastructure_GrokReviewer["review_v2::grok_reviewer::GrokReviewer"]
    direction TB
    T42_infrastructure_infrastructure_GrokReviewer__self[GrokReviewer]
    T42_infrastructure_infrastructure_GrokReviewer_new([new])
  end
  subgraph T52_infrastructure_infrastructure_ReviewFixRunnerAdapter["review_v2::review_fix_adapter::ReviewFixRunnerAdapter"]
    direction TB
    T52_infrastructure_infrastructure_ReviewFixRunnerAdapter__self[ReviewFixRunnerAdapter]
  end
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
  subgraph cli_driver_cli_driver_module_capability["cli_driver::capability"]
    direction TB
  subgraph T38_cli_driver_cli_driver_CapabilityDriver["capability::CapabilityDriver"]
    direction TB
    T38_cli_driver_cli_driver_CapabilityDriver__self[CapabilityDriver]
    T38_cli_driver_cli_driver_CapabilityDriver_new([new])
    T38_cli_driver_cli_driver_CapabilityDriver_handle([handle])
  end
  end
  subgraph cli_driver_cli_driver_module_hook["cli_driver::hook"]
    direction TB
  subgraph T32_cli_driver_cli_driver_HookDriver["hook::HookDriver"]
    direction TB
    T32_cli_driver_cli_driver_HookDriver__self[HookDriver]
    T32_cli_driver_cli_driver_HookDriver_new([new])
    T32_cli_driver_cli_driver_HookDriver_handle([handle])
  end
  subgraph T31_cli_driver_cli_driver_HookInput["hook::HookInput"]
    direction TB
    T31_cli_driver_cli_driver_HookInput__self[HookInput]
    T31_cli_driver_cli_driver_HookInput_Dispatch[Dispatch]
  end
  end
  subgraph cli_driver_cli_driver_module_render["cli_driver::render"]
    direction TB
  subgraph T36_cli_driver_cli_driver_CommandOutcome["render::CommandOutcome"]
    direction TB
    T36_cli_driver_cli_driver_CommandOutcome__self[CommandOutcome]
    T36_cli_driver_cli_driver_CommandOutcome_success([success])
    T36_cli_driver_cli_driver_CommandOutcome_failure([failure])
  end
  end
  subgraph cli_driver_cli_driver_module_review["cli_driver::review"]
    direction TB
  subgraph T34_cli_driver_cli_driver_ReviewDriver["review::ReviewDriver"]
    direction TB
    T34_cli_driver_cli_driver_ReviewDriver__self[ReviewDriver]
    T34_cli_driver_cli_driver_ReviewDriver_new([new])
    T34_cli_driver_cli_driver_ReviewDriver_handle([handle])
  end
  subgraph T37_cli_driver_cli_driver_ReviewFixDriver["review::ReviewFixDriver"]
    direction TB
    T37_cli_driver_cli_driver_ReviewFixDriver__self[ReviewFixDriver]
    T37_cli_driver_cli_driver_ReviewFixDriver_new([new])
    T37_cli_driver_cli_driver_ReviewFixDriver_handle([handle])
  end
  end
end
subgraph cli_composition["cli_composition"]
  direction TB
  subgraph cli_composition_cli_composition_module_capability["cli_composition::capability"]
    direction TB
  subgraph T57_cli_composition_cli_composition_CapabilityCompositionRoot["capability::CapabilityCompositionRoot"]
    direction TB
    T57_cli_composition_cli_composition_CapabilityCompositionRoot__self[CapabilityCompositionRoot]
    T57_cli_composition_cli_composition_CapabilityCompositionRoot_new([new])
    T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover([discover])
    T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver([capability_driver])
  end
  end
  subgraph cli_composition_cli_composition_module_review_v2["cli_composition::review_v2"]
    direction TB
  subgraph T53_cli_composition_cli_composition_ReviewCompositionRoot["review_v2::shim::ReviewCompositionRoot"]
    direction TB
    T53_cli_composition_cli_composition_ReviewCompositionRoot__self[ReviewCompositionRoot]
    T53_cli_composition_cli_composition_ReviewCompositionRoot_new([new])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver([review_driver])
    T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver([review_fix_driver])
  end
  end
end
subgraph cli["cli"]
  direction TB
end
T51_infrastructure_infrastructure_GrokCapabilityAdapter_new --> T51_infrastructure_infrastructure_GrokCapabilityAdapter__self
T54_infrastructure_infrastructure_GrokCapabilityDefinition_sandbox --> T41_infrastructure_infrastructure_GrokSandbox__self
T54_infrastructure_infrastructure_GrokCapabilityDefinition_resolve --> T54_infrastructure_infrastructure_GrokCapabilityDefinition__self
T54_infrastructure_infrastructure_GrokCapabilityDefinition__self --o|sandbox| T41_infrastructure_infrastructure_GrokSandbox__self
T52_infrastructure_infrastructure_CodexDryFixLocalRunner_new --> T52_infrastructure_infrastructure_CodexDryFixLocalRunner__self
T44_infrastructure_infrastructure_GrokDryChecker_new --o T41_infrastructure_infrastructure_GrokSandbox__self
T44_infrastructure_infrastructure_GrokDryChecker_new --> T44_infrastructure_infrastructure_GrokDryChecker__self
T48_infrastructure_infrastructure_GrokOutputEnvelope_into_structured_output --> T47_infrastructure_infrastructure_GrokEnvelopeError__self
T41_infrastructure_infrastructure_GrokSandbox_ProjectProfile --o T52_infrastructure_infrastructure_GrokSandboxProfileName__self
T52_infrastructure_infrastructure_GrokSandboxProfileName_try_new --> T57_infrastructure_infrastructure_GrokSandboxProfileNameError__self
T52_infrastructure_infrastructure_GrokSandboxProfileName_try_new --> T52_infrastructure_infrastructure_GrokSandboxProfileName__self
T42_infrastructure_infrastructure_GrokReviewer_new --o T41_infrastructure_infrastructure_GrokSandbox__self
T42_infrastructure_infrastructure_GrokReviewer_new --> T42_infrastructure_infrastructure_GrokReviewer__self
T51_infrastructure_infrastructure_GrokCapabilityAdapter__self -.impl.-> R38_usecase_usecase_CapabilityProviderPort__self
T42_infrastructure_infrastructure_GrokReviewer__self -.impl.-> R24_usecase_usecase_Reviewer__self
T44_infrastructure_infrastructure_GrokDryChecker__self -.impl.-> R33_usecase_usecase_DryCheckAgentPort__self
T52_infrastructure_infrastructure_ReviewFixRunnerAdapter__self -.impl.-> R31_usecase_usecase_ReviewFixRunner__self
T38_cli_driver_cli_driver_CapabilityDriver_new --> T38_cli_driver_cli_driver_CapabilityDriver__self
T38_cli_driver_cli_driver_CapabilityDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T32_cli_driver_cli_driver_HookDriver_new --o R35_usecase_usecase_HookDispatchService__self
T32_cli_driver_cli_driver_HookDriver_new --> T32_cli_driver_cli_driver_HookDriver__self
T32_cli_driver_cli_driver_HookDriver_handle --o T31_cli_driver_cli_driver_HookInput__self
T32_cli_driver_cli_driver_HookDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_CommandOutcome_success --> T36_cli_driver_cli_driver_CommandOutcome__self
T36_cli_driver_cli_driver_CommandOutcome_failure --> T36_cli_driver_cli_driver_CommandOutcome__self
T34_cli_driver_cli_driver_ReviewDriver_new --> T34_cli_driver_cli_driver_ReviewDriver__self
T34_cli_driver_cli_driver_ReviewDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T37_cli_driver_cli_driver_ReviewFixDriver_new --> T37_cli_driver_cli_driver_ReviewFixDriver__self
T37_cli_driver_cli_driver_ReviewFixDriver_handle --> T36_cli_driver_cli_driver_CommandOutcome__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_new --> T57_cli_composition_cli_composition_CapabilityCompositionRoot__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover --> T57_cli_composition_cli_composition_CapabilityCompositionRoot__self
T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver --> T38_cli_driver_cli_driver_CapabilityDriver__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_new --> T53_cli_composition_cli_composition_ReviewCompositionRoot__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver --> T34_cli_driver_cli_driver_ReviewDriver__self
T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver --> T37_cli_driver_cli_driver_ReviewFixDriver__self
class R38_usecase_usecase_CapabilityProviderPort_provider method_node
class R38_usecase_usecase_CapabilityProviderPort_dispatch method_node
class R38_usecase_usecase_CapabilityProviderPort__self secondary_port
class R33_usecase_usecase_DryCheckAgentPort_judge method_node
class R33_usecase_usecase_DryCheckAgentPort__self secondary_port
class R35_usecase_usecase_HookDispatchService_dispatch method_node
class R35_usecase_usecase_HookDispatchService_check_skill_compliance method_node
class R35_usecase_usecase_HookDispatchService__self app_service
class R31_usecase_usecase_ReviewFixRunner_run_fix method_node
class R31_usecase_usecase_ReviewFixRunner__self secondary_port
class R24_usecase_usecase_Reviewer_review method_node
class R24_usecase_usecase_Reviewer_fast_review method_node
class R24_usecase_usecase_Reviewer__self secondary_port
class T51_infrastructure_infrastructure_GrokCapabilityAdapter_new method_node
class T51_infrastructure_infrastructure_GrokCapabilityAdapter__self secondary_adapter
class T54_infrastructure_infrastructure_GrokCapabilityDefinition_model method_node
class T54_infrastructure_infrastructure_GrokCapabilityDefinition_sandbox method_node
class T54_infrastructure_infrastructure_GrokCapabilityDefinition_resolve method_node
class T54_infrastructure_infrastructure_GrokCapabilityDefinition__self dto
class T52_infrastructure_infrastructure_CodexDryFixLocalRunner_new method_node
class T52_infrastructure_infrastructure_CodexDryFixLocalRunner_dry_run_fix_local method_node
class T52_infrastructure_infrastructure_CodexDryFixLocalRunner__self secondary_adapter
class T44_infrastructure_infrastructure_GrokDryChecker_new method_node
class T44_infrastructure_infrastructure_GrokDryChecker__self secondary_adapter
class T47_infrastructure_infrastructure_GrokEnvelopeError_ProviderFailure variant_node
class T47_infrastructure_infrastructure_GrokEnvelopeError__self error_type
class T48_infrastructure_infrastructure_GrokOutputEnvelope_Succeeded variant_node
class T48_infrastructure_infrastructure_GrokOutputEnvelope_Failed variant_node
class T48_infrastructure_infrastructure_GrokOutputEnvelope_into_structured_output method_node
class T48_infrastructure_infrastructure_GrokOutputEnvelope__self dto
class T41_infrastructure_infrastructure_GrokSandbox_ReadOnly variant_node
class T41_infrastructure_infrastructure_GrokSandbox_Workspace variant_node
class T41_infrastructure_infrastructure_GrokSandbox_Strict variant_node
class T41_infrastructure_infrastructure_GrokSandbox_ProjectProfile variant_node
class T41_infrastructure_infrastructure_GrokSandbox__self value_object
class T52_infrastructure_infrastructure_GrokSandboxProfileName_try_new method_node
class T52_infrastructure_infrastructure_GrokSandboxProfileName_as_str method_node
class T52_infrastructure_infrastructure_GrokSandboxProfileName__self value_object
class T57_infrastructure_infrastructure_GrokSandboxProfileNameError_Empty variant_node
class T57_infrastructure_infrastructure_GrokSandboxProfileNameError_Reserved variant_node
class T57_infrastructure_infrastructure_GrokSandboxProfileNameError__self error_type
class T42_infrastructure_infrastructure_GrokReviewer_new method_node
class T42_infrastructure_infrastructure_GrokReviewer__self secondary_adapter
class T52_infrastructure_infrastructure_ReviewFixRunnerAdapter__self secondary_adapter
class T38_cli_driver_cli_driver_CapabilityDriver_new method_node
class T38_cli_driver_cli_driver_CapabilityDriver_handle method_node
class T38_cli_driver_cli_driver_CapabilityDriver__self primary_adapter
class T32_cli_driver_cli_driver_HookDriver_new method_node
class T32_cli_driver_cli_driver_HookDriver_handle method_node
class T32_cli_driver_cli_driver_HookDriver__self primary_adapter
class T31_cli_driver_cli_driver_HookInput_Dispatch variant_node
class T31_cli_driver_cli_driver_HookInput__self dto
class T36_cli_driver_cli_driver_CommandOutcome_success method_node
class T36_cli_driver_cli_driver_CommandOutcome_failure method_node
class T36_cli_driver_cli_driver_CommandOutcome__self dto
class T34_cli_driver_cli_driver_ReviewDriver_new method_node
class T34_cli_driver_cli_driver_ReviewDriver_handle method_node
class T34_cli_driver_cli_driver_ReviewDriver__self primary_adapter
class T37_cli_driver_cli_driver_ReviewFixDriver_new method_node
class T37_cli_driver_cli_driver_ReviewFixDriver_handle method_node
class T37_cli_driver_cli_driver_ReviewFixDriver__self primary_adapter
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_new method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_discover method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot_capability_driver method_node
class T57_cli_composition_cli_composition_CapabilityCompositionRoot__self composition_root
class T53_cli_composition_cli_composition_ReviewCompositionRoot_new method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_driver method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot_review_fix_driver method_node
class T53_cli_composition_cli_composition_ReviewCompositionRoot__self composition_root
```
