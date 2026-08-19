<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackCommand | enum | modify | Archive, Transition, Branch, Resolve, Views, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, TypeGraph, TypeSignals, BaselineGraph, ContractMap, SpecElementHash, BaselineCapture, FixpointResolve, SetCommitHash, Lint, CatalogueImplSignals, SwitchBase, MergeBase | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::track::tddd::type_signals::execute_type_signals | free_function | add | fn(track_id: Option<String>, workspace_root: std::path::PathBuf, layer: Option<String>) -> Result<std::process::ExitCode, CliError> | 🔵 | 🔵 |

