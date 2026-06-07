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
  subgraph domain_domain_module_verify["domain::verify"]
    direction TB
  subgraph T22_domain_domain_Severity["verify::Severity"]
    direction TB
    T22_domain_domain_Severity__self[Severity]
    T22_domain_domain_Severity_Info[Info]
    T22_domain_domain_Severity_Warning[Warning]
    T22_domain_domain_Severity_Error[Error]
  end
  subgraph T27_domain_domain_VerifyFinding["verify::VerifyFinding"]
    direction TB
    T27_domain_domain_VerifyFinding__self[VerifyFinding]
    T27_domain_domain_VerifyFinding_new([new])
    T27_domain_domain_VerifyFinding_error([error])
    T27_domain_domain_VerifyFinding_warning([warning])
    T27_domain_domain_VerifyFinding_severity([severity])
    T27_domain_domain_VerifyFinding_message([message])
  end
  subgraph T27_domain_domain_VerifyOutcome["verify::VerifyOutcome"]
    direction TB
    T27_domain_domain_VerifyOutcome__self[VerifyOutcome]
    T27_domain_domain_VerifyOutcome_pass([pass])
    T27_domain_domain_VerifyOutcome_from_findings([from_findings])
    T27_domain_domain_VerifyOutcome_is_ok([is_ok])
    T27_domain_domain_VerifyOutcome_has_errors([has_errors])
    T27_domain_domain_VerifyOutcome_findings([findings])
    T27_domain_domain_VerifyOutcome_add([add])
    T27_domain_domain_VerifyOutcome_merge([merge])
    T27_domain_domain_VerifyOutcome_error_count([error_count])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
end
subgraph infrastructure["infrastructure"]
  direction TB
  subgraph infrastructure_infrastructure_module_track["infrastructure::track"]
    direction TB
  F78_infrastructure_infrastructure_infrastructure__track__render__plan__render_plan[[render_plan]]
  F86_infrastructure_infrastructure_infrastructure__track__render__registry__render_registry[[render_registry]]
  F94_infrastructure_infrastructure_infrastructure__track__render__snapshot__collect_track_snapshots[[collect_track_snapshots]]
  F86_infrastructure_infrastructure_infrastructure__track__render__sync__sync_rendered_views[[sync_rendered_views]]
  F95_infrastructure_infrastructure_infrastructure__track__render__validate__validate_track_snapshots[[validate_track_snapshots]]
  end
  subgraph infrastructure_infrastructure_module_verify["infrastructure::verify"]
    direction TB
  F73_infrastructure_infrastructure_infrastructure__verify__module_size__verify[[verify]]
  end
end
T27_domain_domain_VerifyFinding_new --o T22_domain_domain_Severity__self
T27_domain_domain_VerifyFinding_new --> T27_domain_domain_VerifyFinding__self
T27_domain_domain_VerifyFinding_error --> T27_domain_domain_VerifyFinding__self
T27_domain_domain_VerifyFinding_warning --> T27_domain_domain_VerifyFinding__self
T27_domain_domain_VerifyFinding_severity --> T22_domain_domain_Severity__self
T27_domain_domain_VerifyOutcome_pass --> T27_domain_domain_VerifyOutcome__self
T27_domain_domain_VerifyOutcome_from_findings --o T27_domain_domain_VerifyFinding__self
T27_domain_domain_VerifyOutcome_from_findings --> T27_domain_domain_VerifyOutcome__self
T27_domain_domain_VerifyOutcome_findings --> T27_domain_domain_VerifyFinding__self
T27_domain_domain_VerifyOutcome_add --o T27_domain_domain_VerifyFinding__self
T27_domain_domain_VerifyOutcome_merge --o T27_domain_domain_VerifyOutcome__self
F73_infrastructure_infrastructure_infrastructure__verify__module_size__verify --> T27_domain_domain_VerifyOutcome__self
class T22_domain_domain_Severity_Info variant_node
class T22_domain_domain_Severity_Warning variant_node
class T22_domain_domain_Severity_Error variant_node
class T22_domain_domain_Severity__self value_object
class T27_domain_domain_VerifyFinding_new method_node
class T27_domain_domain_VerifyFinding_error method_node
class T27_domain_domain_VerifyFinding_warning method_node
class T27_domain_domain_VerifyFinding_severity method_node
class T27_domain_domain_VerifyFinding_message method_node
class T27_domain_domain_VerifyFinding__self value_object
class T27_domain_domain_VerifyOutcome_pass method_node
class T27_domain_domain_VerifyOutcome_from_findings method_node
class T27_domain_domain_VerifyOutcome_is_ok method_node
class T27_domain_domain_VerifyOutcome_has_errors method_node
class T27_domain_domain_VerifyOutcome_findings method_node
class T27_domain_domain_VerifyOutcome_add method_node
class T27_domain_domain_VerifyOutcome_merge method_node
class T27_domain_domain_VerifyOutcome_error_count method_node
class T27_domain_domain_VerifyOutcome__self value_object
class F78_infrastructure_infrastructure_infrastructure__track__render__plan__render_plan free_function
class F78_infrastructure_infrastructure_infrastructure__track__render__plan__render_plan function_node
class F86_infrastructure_infrastructure_infrastructure__track__render__registry__render_registry free_function
class F86_infrastructure_infrastructure_infrastructure__track__render__registry__render_registry function_node
class F94_infrastructure_infrastructure_infrastructure__track__render__snapshot__collect_track_snapshots free_function
class F94_infrastructure_infrastructure_infrastructure__track__render__snapshot__collect_track_snapshots function_node
class F86_infrastructure_infrastructure_infrastructure__track__render__sync__sync_rendered_views free_function
class F86_infrastructure_infrastructure_infrastructure__track__render__sync__sync_rendered_views function_node
class F95_infrastructure_infrastructure_infrastructure__track__render__validate__validate_track_snapshots free_function
class F95_infrastructure_infrastructure_infrastructure__track__render__validate__validate_track_snapshots function_node
class F73_infrastructure_infrastructure_infrastructure__verify__module_size__verify free_function
class F73_infrastructure_infrastructure_infrastructure__verify__module_size__verify function_node
```
