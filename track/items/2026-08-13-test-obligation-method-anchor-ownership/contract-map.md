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
  subgraph T31_domain_domain_MethodDeclaration["tddd::catalogue_v2::methods::MethodDeclaration"]
    direction TB
    T31_domain_domain_MethodDeclaration__self[MethodDeclaration]
    T31_domain_domain_MethodDeclaration_new([new])
    T31_domain_domain_MethodDeclaration_associated_function([associated_function])
    T31_domain_domain_MethodDeclaration_name([name])
    T31_domain_domain_MethodDeclaration_receiver([receiver])
    T31_domain_domain_MethodDeclaration_params([params])
    T31_domain_domain_MethodDeclaration_returns([returns])
    T31_domain_domain_MethodDeclaration_is_async([is_async])
    T31_domain_domain_MethodDeclaration_has_default_impl([has_default_impl])
    T31_domain_domain_MethodDeclaration_generics([generics])
    T31_domain_domain_MethodDeclaration_where_predicates([where_predicates])
    T31_domain_domain_MethodDeclaration_spec_refs([spec_refs])
    T31_domain_domain_MethodDeclaration_docs([docs])
    T31_domain_domain_MethodDeclaration_signature_string([signature_string])
  end
  subgraph T28_domain_domain_TestObligation["tddd::test_obligation::obligations::TestObligation"]
    direction TB
    T28_domain_domain_TestObligation__self[TestObligation]
    T28_domain_domain_TestObligation_new([new])
    T28_domain_domain_TestObligation_id([id])
    T28_domain_domain_TestObligation_target_entry([target_entry])
    T28_domain_domain_TestObligation_target_role([target_role])
    T28_domain_domain_TestObligation_brief([brief])
    T28_domain_domain_TestObligation_declaration_hash([declaration_hash])
    T28_domain_domain_TestObligation_spec_refs([spec_refs])
    T28_domain_domain_TestObligation_owns_edge([owns_edge])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
  subgraph usecase_usecase_module_test_obligation["usecase::test_obligation"]
    direction TB
  subgraph T47_usecase_usecase_DeriveTestObligationsInteractor["test_obligation::derive::DeriveTestObligationsInteractor"]
    direction TB
    T47_usecase_usecase_DeriveTestObligationsInteractor__self[DeriveTestObligationsInteractor]
    T47_usecase_usecase_DeriveTestObligationsInteractor_new([new])
  end
  subgraph T49_usecase_usecase_EvaluateTestObligationsInteractor["test_obligation::evaluate::EvaluateTestObligationsInteractor"]
    direction TB
    T49_usecase_usecase_EvaluateTestObligationsInteractor__self[EvaluateTestObligationsInteractor]
    T49_usecase_usecase_EvaluateTestObligationsInteractor_new([new])
  end
  end
end
subgraph infrastructure["infrastructure"]
  direction TB
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
T31_domain_domain_MethodDeclaration_new --> T31_domain_domain_MethodDeclaration__self
T31_domain_domain_MethodDeclaration_associated_function --> T31_domain_domain_MethodDeclaration__self
T28_domain_domain_TestObligation_new --> T28_domain_domain_TestObligation__self
T47_usecase_usecase_DeriveTestObligationsInteractor_new --> T47_usecase_usecase_DeriveTestObligationsInteractor__self
T49_usecase_usecase_EvaluateTestObligationsInteractor_new --> T49_usecase_usecase_EvaluateTestObligationsInteractor__self
class T31_domain_domain_MethodDeclaration_new method_node
class T31_domain_domain_MethodDeclaration_associated_function method_node
class T31_domain_domain_MethodDeclaration_name method_node
class T31_domain_domain_MethodDeclaration_receiver method_node
class T31_domain_domain_MethodDeclaration_params method_node
class T31_domain_domain_MethodDeclaration_returns method_node
class T31_domain_domain_MethodDeclaration_is_async method_node
class T31_domain_domain_MethodDeclaration_has_default_impl method_node
class T31_domain_domain_MethodDeclaration_generics method_node
class T31_domain_domain_MethodDeclaration_where_predicates method_node
class T31_domain_domain_MethodDeclaration_spec_refs method_node
class T31_domain_domain_MethodDeclaration_docs method_node
class T31_domain_domain_MethodDeclaration_signature_string method_node
class T31_domain_domain_MethodDeclaration__self value_object
class T28_domain_domain_TestObligation_new method_node
class T28_domain_domain_TestObligation_id method_node
class T28_domain_domain_TestObligation_target_entry method_node
class T28_domain_domain_TestObligation_target_role method_node
class T28_domain_domain_TestObligation_brief method_node
class T28_domain_domain_TestObligation_declaration_hash method_node
class T28_domain_domain_TestObligation_spec_refs method_node
class T28_domain_domain_TestObligation_owns_edge method_node
class T28_domain_domain_TestObligation__self value_object
class T47_usecase_usecase_DeriveTestObligationsInteractor_new method_node
class T47_usecase_usecase_DeriveTestObligationsInteractor__self interactor
class T49_usecase_usecase_EvaluateTestObligationsInteractor_new method_node
class T49_usecase_usecase_EvaluateTestObligationsInteractor__self interactor
```
