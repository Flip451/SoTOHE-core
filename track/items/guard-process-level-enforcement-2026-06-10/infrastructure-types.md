<!-- Generated from infrastructure-types.json — DO NOT EDIT DIRECTLY -->

## Error Types

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| GitError | error_type | reference | CurrentDir, Spawn, CommandFailed, EmptyRepoRoot | 🔵 | 🔵 |

## DTOs

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| TrackBranchRecord | dto | reference | — | 🔵 | 🔵 |

## Secondary Adapters

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| SystemGitRepo | secondary_adapter | reference | impl Debug, impl GitRepository | 🔵 | 🔵 |

## Free Functions

| Name | Kind | Action | Details | Signal | Cat-Spec |
|------|------|--------|---------|--------|----------|
| infrastructure::git_cli::collect_track_branch_claims | free_function | reference | fn(root: &std::path::Path) -> Result<Vec<TrackBranchRecord>, String> | 🔵 | 🔵 |
| infrastructure::git_cli::load_explicit_track_branch | free_function | reference | fn(root: &std::path::Path, track_dir: &std::path::Path) -> Result<TrackBranchRecord, String> | 🔵 | 🔵 |
| infrastructure::git_cli::load_explicit_track_branch_from_items_dir | free_function | reference | fn(root: &std::path::Path, items_dir: &std::path::Path, track_dir: &std::path::Path) -> Result<TrackBranchRecord, String> | 🔵 | 🔵 |
| infrastructure::git_cli::resolve_repo_path | free_function | reference | fn(root: &std::path::Path, path: &std::path::Path) -> std::path::PathBuf | 🔵 | 🔵 |
| infrastructure::verify::hooks_path::verify | free_function | — | fn(root: &std::path::Path) -> domain::verify::VerifyOutcome | 🟡 | 🔵 |

