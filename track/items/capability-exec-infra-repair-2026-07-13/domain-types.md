<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DiagnosticMessage | value_object | reference | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| Phase1Error | error_type | modify | ActionContradiction, UnresolvedTypeRef, DanglingId, RustdocRootResolution | 🟡 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalEvaluatorPort | secondary_port | reference | fn evaluate(&self, a: ExtendedCrate, b: rustdoc_types::Crate, c: rustdoc_types::Crate) -> Result<ThreeWayEvaluationReport, Phase1Error> | 🔵 | 🔵 |

