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
  subgraph domain_domain_module_tddd["domain::tddd"]
    direction TB
  subgraph T22_domain_domain_DataRole["tddd::catalogue_v2::roles::DataRole"]
    direction TB
    T22_domain_domain_DataRole__self[DataRole]
    T22_domain_domain_DataRole_ValueObject[ValueObject]
    T22_domain_domain_DataRole_Entity[Entity]
    T22_domain_domain_DataRole_AggregateRoot[AggregateRoot]
    T22_domain_domain_DataRole_DomainService[DomainService]
    T22_domain_domain_DataRole_Specification[Specification]
    T22_domain_domain_DataRole_Factory[Factory]
    T22_domain_domain_DataRole_UseCase[UseCase]
    T22_domain_domain_DataRole_Interactor[Interactor]
    T22_domain_domain_DataRole_Command[Command]
    T22_domain_domain_DataRole_Query[Query]
    T22_domain_domain_DataRole_Dto[Dto]
    T22_domain_domain_DataRole_ErrorType[ErrorType]
    T22_domain_domain_DataRole_SecondaryAdapter[SecondaryAdapter]
    T22_domain_domain_DataRole_DomainEvent[DomainEvent]
    T22_domain_domain_DataRole_EventPolicy[EventPolicy]
    T22_domain_domain_DataRole_CompositionRoot[CompositionRoot]
    T22_domain_domain_DataRole_PrimaryAdapter[PrimaryAdapter]
    T22_domain_domain_DataRole_value_object([value_object])
    T22_domain_domain_DataRole_entity([entity])
    T22_domain_domain_DataRole_aggregate_root([aggregate_root])
    T22_domain_domain_DataRole_domain_service([domain_service])
    T22_domain_domain_DataRole_use_case([use_case])
    T22_domain_domain_DataRole_variant_name([variant_name])
  end
  subgraph T22_domain_domain_RoleKind["tddd::catalogue_linter::RoleKind"]
    direction TB
    T22_domain_domain_RoleKind__self[RoleKind]
    T22_domain_domain_RoleKind_ValueObject[ValueObject]
    T22_domain_domain_RoleKind_Entity[Entity]
    T22_domain_domain_RoleKind_AggregateRoot[AggregateRoot]
    T22_domain_domain_RoleKind_DomainService[DomainService]
    T22_domain_domain_RoleKind_Specification[Specification]
    T22_domain_domain_RoleKind_Factory[Factory]
    T22_domain_domain_RoleKind_UseCase[UseCase]
    T22_domain_domain_RoleKind_Interactor[Interactor]
    T22_domain_domain_RoleKind_Command[Command]
    T22_domain_domain_RoleKind_Query[Query]
    T22_domain_domain_RoleKind_Dto[Dto]
    T22_domain_domain_RoleKind_ErrorType[ErrorType]
    T22_domain_domain_RoleKind_SecondaryAdapter[SecondaryAdapter]
    T22_domain_domain_RoleKind_DomainEvent[DomainEvent]
    T22_domain_domain_RoleKind_EventPolicy[EventPolicy]
    T22_domain_domain_RoleKind_SpecificationPort[SpecificationPort]
    T22_domain_domain_RoleKind_ApplicationService[ApplicationService]
    T22_domain_domain_RoleKind_SecondaryPort[SecondaryPort]
    T22_domain_domain_RoleKind_Repository[Repository]
    T22_domain_domain_RoleKind_CompositionRoot[CompositionRoot]
    T22_domain_domain_RoleKind_PrimaryAdapter[PrimaryAdapter]
    T22_domain_domain_RoleKind_FreeFunction[FreeFunction]
    T22_domain_domain_RoleKind_UseCaseFunction[UseCaseFunction]
    T22_domain_domain_RoleKind_from_data_role([from_data_role])
    T22_domain_domain_RoleKind_from_contract_role([from_contract_role])
    T22_domain_domain_RoleKind_from_function_role([from_function_role])
    T22_domain_domain_RoleKind_variant_name([variant_name])
  end
  end
end
subgraph usecase["usecase"]
  direction TB
end
subgraph infrastructure["infrastructure"]
  direction TB
end
T22_domain_domain_DataRole_value_object --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_entity --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_aggregate_root --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_domain_service --> T22_domain_domain_DataRole__self
T22_domain_domain_DataRole_use_case --> T22_domain_domain_DataRole__self
T22_domain_domain_RoleKind_from_data_role --o T22_domain_domain_DataRole__self
T22_domain_domain_RoleKind_from_data_role --> T22_domain_domain_RoleKind__self
T22_domain_domain_RoleKind_from_contract_role --> T22_domain_domain_RoleKind__self
T22_domain_domain_RoleKind_from_function_role --> T22_domain_domain_RoleKind__self
class T22_domain_domain_DataRole_ValueObject variant_node
class T22_domain_domain_DataRole_Entity variant_node
class T22_domain_domain_DataRole_AggregateRoot variant_node
class T22_domain_domain_DataRole_DomainService variant_node
class T22_domain_domain_DataRole_Specification variant_node
class T22_domain_domain_DataRole_Factory variant_node
class T22_domain_domain_DataRole_UseCase variant_node
class T22_domain_domain_DataRole_Interactor variant_node
class T22_domain_domain_DataRole_Command variant_node
class T22_domain_domain_DataRole_Query variant_node
class T22_domain_domain_DataRole_Dto variant_node
class T22_domain_domain_DataRole_ErrorType variant_node
class T22_domain_domain_DataRole_SecondaryAdapter variant_node
class T22_domain_domain_DataRole_DomainEvent variant_node
class T22_domain_domain_DataRole_EventPolicy variant_node
class T22_domain_domain_DataRole_CompositionRoot variant_node
class T22_domain_domain_DataRole_PrimaryAdapter variant_node
class T22_domain_domain_DataRole_value_object method_node
class T22_domain_domain_DataRole_entity method_node
class T22_domain_domain_DataRole_aggregate_root method_node
class T22_domain_domain_DataRole_domain_service method_node
class T22_domain_domain_DataRole_use_case method_node
class T22_domain_domain_DataRole_variant_name method_node
class T22_domain_domain_DataRole__self value_object
class T22_domain_domain_RoleKind_ValueObject variant_node
class T22_domain_domain_RoleKind_Entity variant_node
class T22_domain_domain_RoleKind_AggregateRoot variant_node
class T22_domain_domain_RoleKind_DomainService variant_node
class T22_domain_domain_RoleKind_Specification variant_node
class T22_domain_domain_RoleKind_Factory variant_node
class T22_domain_domain_RoleKind_UseCase variant_node
class T22_domain_domain_RoleKind_Interactor variant_node
class T22_domain_domain_RoleKind_Command variant_node
class T22_domain_domain_RoleKind_Query variant_node
class T22_domain_domain_RoleKind_Dto variant_node
class T22_domain_domain_RoleKind_ErrorType variant_node
class T22_domain_domain_RoleKind_SecondaryAdapter variant_node
class T22_domain_domain_RoleKind_DomainEvent variant_node
class T22_domain_domain_RoleKind_EventPolicy variant_node
class T22_domain_domain_RoleKind_SpecificationPort variant_node
class T22_domain_domain_RoleKind_ApplicationService variant_node
class T22_domain_domain_RoleKind_SecondaryPort variant_node
class T22_domain_domain_RoleKind_Repository variant_node
class T22_domain_domain_RoleKind_CompositionRoot variant_node
class T22_domain_domain_RoleKind_PrimaryAdapter variant_node
class T22_domain_domain_RoleKind_FreeFunction variant_node
class T22_domain_domain_RoleKind_UseCaseFunction variant_node
class T22_domain_domain_RoleKind_from_data_role method_node
class T22_domain_domain_RoleKind_from_contract_role method_node
class T22_domain_domain_RoleKind_from_function_role method_node
class T22_domain_domain_RoleKind_variant_name method_node
class T22_domain_domain_RoleKind__self value_object
```
