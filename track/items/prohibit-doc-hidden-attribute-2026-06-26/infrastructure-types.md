<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SynScanContext | dto | add | — | 🟡 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsVerifyAdapter | secondary_adapter | modify | impl VerifyPort, impl Default, impl Debug | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::verify::doc_hidden::verify | free_function | add | fn(root: &std::path::Path) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::syn_scan::scan_workspace_rust_sources | free_function | add | fn(root: &std::path::Path, inspect: impl FnMut(SynScanContext) -> Vec<domain::verify::VerifyFinding>) -> domain::verify::VerifyOutcome | 🟡 | 🔵 |

