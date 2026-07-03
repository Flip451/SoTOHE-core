<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Enums

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| LintRuleKind | enum | modify | FieldEmpty, FieldNonEmpty, KindLayerConstraint, ReferencedRoleConstraint, TraitImplRequired, NoRoleInMethodSignature, MethodReferenceSignature, AccessorSignatureRequired, FieldElementUniqueAcrossEntries, NoExternalReferenceInMethods, NoPublicField, ForbiddenMethodReceiver, ForbidPrimitiveInTypes | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| LintConfigLoader | secondary_port | reference | fn load(&self) -> Result<LintConfig, LintConfigLoaderError> | 🔵 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLint | application_service | reference | fn execute(&self, command: RunCatalogueLintCommand) -> Result<Vec<CatalogueLintViolation>, RunCatalogueLintError> | 🔵 | 🔵 |
| TrackService | application_service | modify | fn init(&self, items_dir: std::path::PathBuf, track_id: String, description: String) -> TrackCommandOutput, fn transition(&self, items_dir: std::path::PathBuf, track_id: Option<String>, task_id: String, target_status: String, commit_hash: Option<String>) -> TrackCommandOutput, fn resolve(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn views_sync(&self, project_root: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn branch_create(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn branch_switch(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn views_validate(&self, project_root: std::path::PathBuf) -> TrackCommandOutput, fn add_task(&self, items_dir: std::path::PathBuf, track_id: Option<String>, description: String, section: Option<String>, after: Option<String>) -> TrackCommandOutput, fn set_override(&self, items_dir: std::path::PathBuf, track_id: Option<String>, status: String, reason: String) -> TrackCommandOutput, fn clear_override(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn next_task(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn task_counts(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn archive(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn detect_active(&self, project_root: std::path::PathBuf) -> TrackCommandOutput, fn catalogue_lint_check_active_track(&self, track_id: Option<String>, workspace_root: std::path::PathBuf, rules_file: Option<std::path::PathBuf>) -> TrackCommandOutput | 🔵 | 🔵 |

## Interactors

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| RunCatalogueLintInteractor | interactor | modify | — | 🔵 | 🔵 |

