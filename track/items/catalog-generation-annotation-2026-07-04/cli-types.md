<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogActionArg | enum | add | Reference, Modify, Delete | 🔵 | 🔵 |
| CatalogCommand | enum | add | Init, Add, Import, Cite, Check | 🔵 | 🔵 |
| CatalogKindArg | enum | add | Struct, Enum, TypeAlias, Trait, Function | 🔵 | 🔵 |
| CliCommand | enum | modify | Arch, Conventions, Domain, Guard, Hook, Track, Git, Pr, Plan, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, Signal, TaskContract, Catalog, CatalogueLint, Demo | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CatalogAddArgs | dto | add | — | 🔵 | 🔵 |
| CatalogCheckArgs | dto | add | — | 🔵 | 🔵 |
| CatalogCiteArgs | dto | add | — | 🔵 | 🔵 |
| CatalogImportArgs | dto | add | — | 🔵 | 🔵 |
| CatalogInitArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::catalog::action_to_select | free_function | add | fn(action: CatalogActionArg) -> cli_driver::catalog_gen::CatalogImportSelect | 🔵 | 🔵 |
| cli::commands::catalog::dispatch | free_function | add | fn(input: cli_driver::catalog_gen::CatalogInput) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalog::execute | free_function | add | fn(cmd: CatalogCommand) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalog::execute_add | free_function | add | fn(args: CatalogAddArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalog::execute_check | free_function | add | fn(args: CatalogCheckArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalog::execute_cite | free_function | add | fn(args: CatalogCiteArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalog::execute_import | free_function | add | fn(args: CatalogImportArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalog::execute_init | free_function | add | fn(args: CatalogInitArgs) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::catalog::kind_to_select | free_function | add | fn(kind: CatalogKindArg) -> cli_driver::catalog_gen::CatalogKindSelect | 🔵 | 🔵 |
| cli::commands::catalog::resolve_for_read | free_function | add | fn(explicit: Option<String>, items_dir: &std::path::Path) -> Result<String, std::process::ExitCode> | 🔵 | 🔵 |
| cli::commands::catalog::resolve_for_write | free_function | add | fn(explicit: Option<String>, items_dir: &std::path::Path) -> Result<String, std::process::ExitCode> | 🔵 | 🔵 |

