<!-- Generated from cli-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitCommand | enum | modify | AddAll, AddFromFile, CommitFromFile, NoteFromFile, Sync, Unstage, Stash | 🟡 | 🔵 |
| GitStashAction | enum | add | Push, Pop | 🟡 | 🔵 |
| TrackCommand | enum | modify | Archive, Transition, Branch, Resolve, Views, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, TypeGraph, BaselineGraph, ContractMap, SpecElementHash, BaselineCapture, FixpointResolve, SetCommitHash, Lint, CatalogueImplSignals, SwitchBase, MergeBase | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli::commands::git::execute | free_function | reference | fn(cmd: GitCommand) -> std::process::ExitCode | 🔵 | 🔵 |

