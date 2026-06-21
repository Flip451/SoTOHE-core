<!-- Generated from domain-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ChainId | enum | — | AdrUser, SpecAdr, CatalogSpec, ImplCatalog | 🔵 | 🔵 |
| GateKind | enum | — | Commit, Merge | 🔵 | 🔵 |
| Strictness | enum | — | Strict, Interim | 🔵 | 🔵 |

## Value Objects

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| AssocConstDecl | value_object | — | — | 🔵 | 🔵 |
| AssocConstName | value_object | — | — | 🔵 | 🔵 |
| AssocTypeDecl | value_object | — | — | 🔵 | 🔵 |
| ChainGateEntry | value_object | — | — | 🔵 | 🔵 |
| RustExpression | value_object | — | — | 🔵 | 🔵 |
| SignalGateMatrix | value_object | — | — | 🔵 | 🔵 |
| TraitEntry | value_object | modify | — | 🔵 | 🔵 |

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RustExpressionError | error_type | — | Empty, WhitespaceBoundary | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ChainIdentity | secondary_port | — | — | 🔵 | 🔵 |
| PersistedSoTChainGate | secondary_port | — | fn evaluate_gate(persisted: &<Self>::Persisted, strictness: Strictness) -> VerifyOutcome, fn calc_error(error: <Self>::CalcError) -> VerifyOutcome, fn stale_error(error: <Self>::StaleError) -> VerifyOutcome | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::chain::check_catalogue_spec_signals | free_function | — | fn(doc: &CatalogueSpecSignalsDocument, strictness: Strictness) -> VerifyOutcome | 🔵 | 🔵 |
| domain::spec::check_spec_doc_signals | free_function | modify | fn(doc: &SpecDocument, strictness: Strictness) -> VerifyOutcome | 🔵 | 🔵 |
| domain::tddd::consistency::check_type_signals | free_function | modify | fn(signals_doc: &TypeSignalsDocument, strictness: Strictness) -> VerifyOutcome | 🔵 | 🔵 |

