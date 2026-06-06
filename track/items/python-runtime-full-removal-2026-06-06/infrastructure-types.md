<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchRulesError | error_type | — | Io, Parse, InvalidRules | 🔵 | 🔵 |
| ConventionDocsError | error_type | — | Io, MissingReadme, MissingMarkers, AlreadyExists, InvalidSlug | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::arch::render_direct_checks | free_function | — | fn(root: &std::path::Path) -> Result<String, ArchRulesError> | 🔵 | 🔵 |
| infrastructure::arch::render_workspace_members | free_function | — | fn(root: &std::path::Path) -> Result<String, ArchRulesError> | 🔵 | 🔵 |
| infrastructure::arch::render_workspace_tree | free_function | — | fn(root: &std::path::Path) -> Result<String, ArchRulesError> | 🔵 | 🔵 |
| infrastructure::arch::render_workspace_tree_full | free_function | — | fn(root: &std::path::Path) -> Result<String, ArchRulesError> | 🔵 | 🔵 |
| infrastructure::conventions::add_convention_doc | free_function | — | fn(root: &std::path::Path, name: &str, slug: Option<&str>, title: Option<&str>, summary: Option<&str>) -> Result<(), ConventionDocsError> | 🟡 | 🔵 |
| infrastructure::conventions::update_convention_index | free_function | — | fn(root: &std::path::Path) -> Result<(), ConventionDocsError> | 🟡 | 🔵 |
| infrastructure::conventions::verify_convention_index | free_function | — | fn(root: &std::path::Path) -> domain::verify::VerifyOutcome | 🟡 | 🔵 |
| infrastructure::verify::convention_docs::verify | free_function | delete | fn(root: &std::path::Path) -> domain::verify::VerifyOutcome | 🟡 | 🔵 |

