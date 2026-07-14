<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsTemplateExportAdapter | secondary_adapter | modify | impl Debug, impl Default, impl TemplateExportPort | 🔵 | 🔵 |
| FsVerifyAdapter | secondary_adapter | modify | impl Debug, impl Default, impl VerifyPort | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::verify::machine_paths::verify | free_function | add | fn(project_root: &std::path::Path, machine_home_dir: Option<&std::path::Path>) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::sotp_version_tag::verify | free_function | add | fn(project_root: &std::path::Path) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |
| infrastructure::verify::template_refs::verify | free_function | add | fn(project_root: &std::path::Path) -> domain::verify::VerifyOutcome | 🔵 | 🔵 |

