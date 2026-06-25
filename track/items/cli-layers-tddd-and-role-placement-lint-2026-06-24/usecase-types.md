<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PlannerPortError | error_type | add | MissingPromptSource, PlannerUnavailable, PlannerTimeout, PlannerFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PlannerPort | secondary_port | add | fn run(&self, model: &str, prompt: &str, timeout_seconds: u64) -> Result<PlanRunOutput, PlannerPortError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchService | application_service | add | fn render_tree(&self, project_root: std::path::PathBuf) -> Result<String, ArchPortError>, fn render_tree_full(&self, project_root: std::path::PathBuf) -> Result<String, ArchPortError>, fn render_members(&self, project_root: std::path::PathBuf) -> Result<String, ArchPortError>, fn render_direct_checks(&self, project_root: std::path::PathBuf) -> Result<String, ArchPortError> | 🔵 | 🔵 |
| ConventionsService | application_service | add | fn add_convention(&self, root: std::path::PathBuf, name: String, slug: Option<String>, title: Option<String>, summary: Option<String>) -> Result<String, ConventionsPortError>, fn update_index(&self, root: std::path::PathBuf) -> Result<String, ConventionsPortError>, fn verify_index(&self, root: std::path::PathBuf) -> Result<VerifyIndexResult, ConventionsPortError> | 🔵 | 🔵 |
| FileService | application_service | add | fn write_atomic(&self, path: std::path::PathBuf, content: Vec<u8>) -> Result<(), FilePortError> | 🔵 | 🔵 |
| PlannerService | application_service | add | fn run_codex_local(&self, model: String, briefing_file: Option<std::path::PathBuf>, prompt: Option<String>, timeout_seconds: u64) -> Result<PlanRunOutput, PlannerPortError> | 🔵 | 🔵 |
| VerifyService | application_service | add | fn verify_tech_stack(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_latest_track(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_arch_docs(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_layers(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_hooks_path(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_spec_attribution(&self, spec_path: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_spec_frontmatter(&self, spec_path: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_canonical_modules(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_module_size(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_domain_purity(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_domain_strings(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_usecase_purity(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_doc_links(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_view_freshness(&self, project_root: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_spec_signals(&self, spec_path: std::path::PathBuf) -> Result<VerifyOutcome, VerifyPortError>, fn verify_plan_artifact_refs(&self, track_dir: Option<std::path::PathBuf>) -> Result<VerifyOutcome, VerifyPortError>, fn verify_catalogue_spec_refs(&self, track_id: Option<String>, items_dir: std::path::PathBuf, workspace_root: std::path::PathBuf, skip_stale: bool) -> Result<VerifyOutcome, VerifyPortError> | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| ArchInteractor | interactor | add | — | 🔵 | 🔵 |
| ConventionsInteractor | interactor | add | — | 🔵 | 🔵 |
| FileInteractor | interactor | add | — | 🔵 | 🔵 |
| PlannerInteractor | interactor | add | — | 🔵 | 🔵 |
| VerifyInteractor | interactor | add | — | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| PlanRunOutput | dto | add | — | 🔵 | 🔵 |

