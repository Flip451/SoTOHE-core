<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| CliCommand | enum | modify | Arch, Conventions, Domain, Guard, Hook, Track, Git, Pr, Plan, Review, File, Verify, FindSimilar, DupIndex, DupCheck, Telemetry, Dry, RefVerify, Signal, TaskContract, Catalog, CatalogueLint, Template, Demo | 🔵 | 🔵 |
| TemplateCommand | enum | add | Export | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TemplateExportArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::template::dispatch | free_function | add | fn(input: cli_driver::template_export::TemplateInput) -> std::process::ExitCode | 🔵 | 🔵 |
| cli::commands::template::execute | free_function | add | fn(cmd: TemplateCommand) -> std::process::ExitCode | 🔵 | 🔵 |

