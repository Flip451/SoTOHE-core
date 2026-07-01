<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackInput | enum | modify | Init, Transition, Resolve, BranchCreate, BranchSwitch, ViewsValidate, ViewsSync, AddTask, SetOverride, ClearOverride, NextTask, TaskCounts, Archive, DetectActive, SwitchBase, FixpointResolve | 🟡 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| DryDriver | primary_adapter | reference | — | 🔵 | 🔵 |
| TrackDriver | primary_adapter | modify | — | 🟡 | 🔵 |

