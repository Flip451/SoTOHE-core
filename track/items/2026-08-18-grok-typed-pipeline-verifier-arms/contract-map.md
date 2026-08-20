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
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_agent_profiles["infrastructure::agent_profiles"]
    direction TB
  subgraph T47_infrastructure_infrastructure_ResolvedExecution["agent_profiles::types::ResolvedExecution"]
    direction TB
    T47_infrastructure_infrastructure_ResolvedExecution__self[ResolvedExecution]
    T47_infrastructure_infrastructure_ResolvedExecution_ProviderCli[ProviderCli]
    T47_infrastructure_infrastructure_ResolvedExecution_HostedService[HostedService]
  end
  end
  subgraph infrastructure_infrastructure_module_capability_exec["infrastructure::capability_exec"]
    direction TB
  subgraph T54_infrastructure_infrastructure_GrokCapabilityDefinition["capability_exec::grok::GrokCapabilityDefinition"]
    direction TB
    T54_infrastructure_infrastructure_GrokCapabilityDefinition__self[GrokCapabilityDefinition]
    T54_infrastructure_infrastructure_GrokCapabilityDefinition_model([model])
    T54_infrastructure_infrastructure_GrokCapabilityDefinition_resolve([resolve])
    T54_infrastructure_infrastructure_GrokCapabilityDefinition_sandbox([sandbox])
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
  subgraph infrastructure_infrastructure_module_ref_verify["infrastructure::ref_verify"]
    direction TB
  F102_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_grok_ref_verifier_args[[build_grok_ref_verifier_args]]
  F106_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__make_ref_verifier_process_runner[[make_ref_verifier_process_runner]]
  end
end
subgraph cli_driver["cli_driver"]
  direction TB
end
subgraph cli_composition["cli_composition"]
  direction TB
end
subgraph cli["cli"]
  direction TB
end
T54_infrastructure_infrastructure_GrokCapabilityDefinition_resolve --> T54_infrastructure_infrastructure_GrokCapabilityDefinition__self
T54_infrastructure_infrastructure_GrokCapabilityDefinition_sandbox --> T41_infrastructure_infrastructure_GrokSandbox__self
T54_infrastructure_infrastructure_GrokCapabilityDefinition__self --o|sandbox| T41_infrastructure_infrastructure_GrokSandbox__self
T48_infrastructure_infrastructure_GrokOutputEnvelope_into_structured_output --> T47_infrastructure_infrastructure_GrokEnvelopeError__self
T41_infrastructure_infrastructure_GrokSandbox_ProjectProfile --o T52_infrastructure_infrastructure_GrokSandboxProfileName__self
T52_infrastructure_infrastructure_GrokSandboxProfileName_try_new --> T57_infrastructure_infrastructure_GrokSandboxProfileNameError__self
T52_infrastructure_infrastructure_GrokSandboxProfileName_try_new --> T52_infrastructure_infrastructure_GrokSandboxProfileName__self
F102_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_grok_ref_verifier_args --o T41_infrastructure_infrastructure_GrokSandbox__self
class T47_infrastructure_infrastructure_ResolvedExecution_ProviderCli variant_node
class T47_infrastructure_infrastructure_ResolvedExecution_HostedService variant_node
class T47_infrastructure_infrastructure_ResolvedExecution__self dto
class T54_infrastructure_infrastructure_GrokCapabilityDefinition_model method_node
class T54_infrastructure_infrastructure_GrokCapabilityDefinition_resolve method_node
class T54_infrastructure_infrastructure_GrokCapabilityDefinition_sandbox method_node
class T54_infrastructure_infrastructure_GrokCapabilityDefinition__self dto
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
class F102_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_grok_ref_verifier_args free_function
class F102_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__build_grok_ref_verifier_args function_node
class F106_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__make_ref_verifier_process_runner free_function
class F106_infrastructure_infrastructure_infrastructure__ref_verify__process_runner__make_ref_verifier_process_runner function_node
```
