<!-- Generated from cli_driver-types.json — DO NOT EDIT DIRECTLY -->

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateOutputInput | dto | add | — | 🔵 | 🔵 |
| cli_driver::render::CommandOutcome | dto | reference | — | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| cli_driver::gate_output::failure_excerpts | free_function | add | fn(output: &[u8]) -> Vec<String> | 🔵 | 🔵 |
| cli_driver::gate_output::is_failure_line | free_function | add | fn(line: &str) -> bool | 🔵 | 🔵 |
| cli_driver::gate_output::render_summary | free_function | add | fn(result: &usecase::gate_output::GateRunResult) -> String | 🔵 | 🔵 |
| cli_driver::gate_output::truncate_line | free_function | add | fn(line: &str) -> String | 🔵 | 🔵 |

## Primary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GateOutputDriver | primary_adapter | add | — | 🔵 | 🔵 |

