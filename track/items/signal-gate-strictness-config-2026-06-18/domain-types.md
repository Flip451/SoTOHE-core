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
| ChainGateEntry | value_object | — | — | 🔵 | 🔵 |
| SignalGateMatrix | value_object | — | — | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ChainIdentity | secondary_port | — | — | 🟡 | 🔵 |
| LiveSoTChain | secondary_port | — | fn calc_live(input: Self) -> Result<Self, Self> | 🟡 | 🔵 |
| PersistedSoTChain | secondary_port | — | fn calc(input: Self) -> Result<Self, Self>, fn load(input: Self) -> Result<Self, Self>, fn check_freshness(input: Self, persisted: Self) -> Result<(), Self>, fn evaluate_gate(persisted: Self, strict: bool) -> VerifyOutcome, fn calc_error(error: Self) -> VerifyOutcome, fn stale_error(error: Self) -> VerifyOutcome | 🟡 | 🔵 |
| SoTChain | secondary_port | — | fn check(input: Self, strict: bool) -> VerifyOutcome | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| domain::chain::check_catalogue_spec_signals | free_function | — | fn(doc: &CatalogueSpecSignalsDocument, strict: bool) -> VerifyOutcome | 🔵 | 🔵 |

