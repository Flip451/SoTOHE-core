<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogActionArg | enum | add | Reference, Modify, Delete | 🟡 | 🔵 |
| CatalogCommand | enum | add | Init, Add, Import, Cite, Check | 🟡 | 🔵 |
| CatalogGateArg | enum | add | Phase2, Commit, Merge | 🟡 | 🔵 |
| CatalogKindArg | enum | add | Struct, Enum, TypeAlias, Trait, Function | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogAddArgs | dto | add | — | 🟡 | 🔵 |
| CatalogCheckArgs | dto | add | — | 🟡 | 🔵 |
| CatalogCiteArgs | dto | add | — | 🟡 | 🔵 |
| CatalogImportArgs | dto | add | — | 🟡 | 🔵 |
| CatalogInitArgs | dto | add | — | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::catalog::execute | free_function | add | fn(cmd: CatalogCommand) -> std::process::ExitCode | 🟡 | 🔵 |

