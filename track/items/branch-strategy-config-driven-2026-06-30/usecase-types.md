<!-- Generated from usecase-types.json — DO NOT EDIT DIRECTLY -->

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| BranchStrategyPort | secondary_port | add | fn base_branch(&self) -> &str, fn merge_target(&self) -> &str, fn merge_method(&self) -> domain::branch_strategy::MergeMethod, fn track_prefix(&self) -> &str | 🟡 | 🔵 |

## Application Services

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackService | application_service | modify | fn init(&self, items_dir: std::path::PathBuf, track_id: String, description: String) -> TrackCommandOutput, fn transition(&self, items_dir: std::path::PathBuf, track_id: Option<String>, task_id: String, target_status: String, commit_hash: Option<String>) -> TrackCommandOutput, fn resolve(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn branch_create(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn branch_switch(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn views_validate(&self, project_root: std::path::PathBuf) -> TrackCommandOutput, fn views_sync(&self, project_root: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn add_task(&self, items_dir: std::path::PathBuf, track_id: Option<String>, description: String, section: Option<String>, after: Option<String>) -> TrackCommandOutput, fn set_override(&self, items_dir: std::path::PathBuf, track_id: Option<String>, status: String, reason: String) -> TrackCommandOutput, fn clear_override(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn next_task(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn task_counts(&self, items_dir: std::path::PathBuf, track_id: Option<String>) -> TrackCommandOutput, fn archive(&self, items_dir: std::path::PathBuf, track_id: String) -> TrackCommandOutput, fn detect_active(&self, project_root: std::path::PathBuf) -> TrackCommandOutput, fn switch_base(&self, project_root: std::path::PathBuf) -> TrackCommandOutput | 🟡 | 🔵 |

