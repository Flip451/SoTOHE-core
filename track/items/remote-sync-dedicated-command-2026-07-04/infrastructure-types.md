<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitError | error_type | reference | CurrentDir, Spawn, CommandFailed, EmptyRepoRoot | 🔵 | 🔵 |
| SyncError | error_type | add | UpstreamNotSet, NonFastForward, WorktreeUnresolved | 🟡 | 🔵 |
| TrackBranchError | error_type | reference | LoadFailed | 🔵 | 🔵 |

## Secondary Ports

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitRepository | secondary_port | delete | fn root(&self) -> &std::path::Path, fn status(&self, args: &[&str]) -> Result<i32, GitError>, fn output(&self, args: &[&str]) -> Result<std::process::Output, GitError>, fn resolve_path(&self, path: &std::path::Path) -> std::path::PathBuf, fn current_branch(&self) -> Result<Option<String>, GitError>, fn push_branch(&self, branch: &str) -> Result<(), GitError>, fn index_tree_hash(&self) -> Result<String, GitError>, fn stage_all_excluding(&self, exclude_files: &[&str], exclude_dirs: &[&str]) -> Result<(), GitError> | 🟡 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackBranchRecord | dto | reference | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| FsGitWorkflowAdapter | secondary_adapter | modify | impl Default, impl GitWorkflowService, impl GitPrimitivePort | 🟡 | 🔵 |
| FsWorkspaceAdapter | secondary_adapter | add | impl Default, impl TrackArchiveFsPort | 🟡 | 🔵 |
| SystemGitRepo | secondary_adapter | modify | impl Debug, impl Clone, impl GitRepository, impl WorktreeReader, impl BranchReaderPort | 🟡 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::git_cli::collect_track_branch_claims | free_function | reference | fn(root: &std::path::Path) -> Result<Vec<TrackBranchRecord>, TrackBranchError> | 🔵 | 🔵 |
| infrastructure::git_cli::load_explicit_track_branch | free_function | reference | fn(root: &std::path::Path, track_dir: &std::path::Path) -> Result<TrackBranchRecord, TrackBranchError> | 🔵 | 🔵 |
| infrastructure::git_cli::load_explicit_track_branch_from_items_dir | free_function | reference | fn(root: &std::path::Path, items_dir: &std::path::Path, track_dir: &std::path::Path) -> Result<TrackBranchRecord, TrackBranchError> | 🔵 | 🔵 |
| infrastructure::git_cli::resolve_repo_path | free_function | reference | fn(root: &std::path::Path, path: &std::path::Path) -> std::path::PathBuf | 🔵 | 🔵 |

