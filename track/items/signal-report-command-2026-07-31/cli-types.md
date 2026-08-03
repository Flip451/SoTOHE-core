<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalCommand | enum | modify | CalcAdrUser, CheckAdrUser, CalcSpecAdr, CheckSpecAdr, CalcCatalogSpec, CheckCatalogSpec, CalcImplCatalog, CheckImplCatalog, Check, Report | 🔵 | 🔵 |
| SignalReportChainArg | enum | add | All, AdrUser, SpecAdr, CatalogSpec, ImplCatalog | 🔵 | 🔵 |
| SignalReportOnlyArg | enum | add | YellowOnly, RedOnly, YellowAndRed | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SignalReportArgs | dto | add | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::signal::execute | free_function | modify | fn(cmd: SignalCommand) -> std::process::ExitCode | 🔵 | 🔵 |

