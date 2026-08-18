<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| MethodDeclaration | value_object | modify | — | 🔵 | 🔵 |
| TestObligation | value_object | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::tddd::test_obligation::obligations::validate_add_modify_methods_have_spec_refs | free_function | add | fn(methods: &[MethodDeclaration]) -> Result<(), DiagnosticMessage> | 🔵 | 🔵 |
| domain::tddd::test_obligation::obligations::validate_method_anchor_coverage | free_function | add | fn(entry: &TraitEntry) -> Result<(), DiagnosticMessage> | 🔵 | 🔵 |
| domain::tddd::test_obligation::obligations::validate_parent_forbids_method_spec_refs | free_function | add | fn(parent: ItemAction, methods: &[MethodDeclaration]) -> Result<(), DiagnosticMessage> | 🔵 | 🔵 |

