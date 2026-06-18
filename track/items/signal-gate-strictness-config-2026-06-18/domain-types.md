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
| SignalGateMatrix | value_object | — | — | 🔵 | 🔵 |
| TraitEntry | value_object | modify | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ChainIdentity | secondary_port | — | — | 🔵 | 🔵 |
| LiveSoTChain | secondary_port | — | fn calc_live(input: &<Self>::Input<'_>) -> Result<<Self>::LiveCalc, <Self>::CalcError> | 🔵 | 🔵 |
| PersistedSoTChain | secondary_port | — | fn calc(input: &<Self>::Input<'_>) -> Result<<Self>::Persisted, <Self>::CalcError>, fn load(input: &<Self>::Input<'_>) -> Result<<Self>::Persisted, <Self>::CalcError>, fn check_freshness(input: &<Self>::Input<'_>, persisted: &<Self>::Persisted) -> Result<(), <Self>::StaleError>, fn evaluate_gate(persisted: &<Self>::Persisted, strict: bool) -> VerifyOutcome, fn calc_error(error: <Self>::CalcError) -> VerifyOutcome, fn stale_error(error: <Self>::StaleError) -> VerifyOutcome | 🔵 | 🔵 |
| SoTChain | secondary_port | — | fn check(input: &<Self>::Input<'_>, strict: bool) -> VerifyOutcome | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::chain::check_catalogue_spec_signals | free_function | — | fn(doc: &CatalogueSpecSignalsDocument, strict: bool) -> VerifyOutcome | 🔵 | 🔵 |

