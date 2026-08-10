//! Bounded committed source-tree enumeration for implementation-input hashing.

use std::path::Path;

use crate::git_cli::isolation::isolated_bounded_git_output;
use crate::git_cli::show::{TreeEntryKind, git_ls_tree_entry_kind_isolated, git_show_blob_sha256};
use crate::tddd::type_signals_evaluator::build_inputs::TreeFileDigest;
use crate::tddd::type_signals_evaluator::layer_graph::LayerCrateRoot;

use super::{
    MAX_SOURCE_DEPTH, MAX_SOURCE_ENTRIES, MAX_SOURCE_FILE_BYTES, MAX_SOURCE_FILES,
    MAX_TOTAL_SOURCE_BYTES, MAX_TREE_OUTPUT_BYTES,
};

pub(super) fn collect_branch_tree_file_digests(
    repo_root: &Path,
    branch: &str,
    roots: &[LayerCrateRoot],
    remaining_budget: &mut usize,
) -> Result<Vec<TreeFileDigest>, String> {
    let mut files = Vec::new();
    let mut visited_entries = 0usize;
    for root in roots {
        ensure_branch_crate_directory(repo_root, branch, &root.path)?;
        append_branch_tree_file_digests(
            repo_root,
            branch,
            &root.path,
            &mut visited_entries,
            &mut files,
            remaining_budget,
        )?;
    }
    if files.is_empty() {
        return Err("architecture layer closure contains no regular files".to_owned());
    }

    // Cargo patch sources live outside the architecture graph. Include the
    // committed vendor tree whenever the branch has one, without interpreting
    // the workspace manifest or guessing which patch entries use it.
    if ensure_optional_branch_vendor_directory(repo_root, branch)? {
        append_branch_tree_file_digests(
            repo_root,
            branch,
            "vendor",
            &mut visited_entries,
            &mut files,
            remaining_budget,
        )?;
    }
    Ok(files)
}

fn append_branch_tree_file_digests(
    repo_root: &Path,
    branch: &str,
    tree_root: &str,
    visited_entries: &mut usize,
    files: &mut Vec<TreeFileDigest>,
    remaining_budget: &mut usize,
) -> Result<(), String> {
    let paths = branch_tree_paths(repo_root, branch, tree_root, visited_entries)?;
    for path in paths {
        if is_vcs_internal(&path) {
            continue;
        }
        if files.len() >= MAX_SOURCE_FILES {
            return Err(format!(
                "implementation source traversal exceeds maximum of {MAX_SOURCE_FILES} files"
            ));
        }
        let (digest, bytes) =
            git_show_blob_sha256(repo_root, branch, &path, MAX_SOURCE_FILE_BYTES)?;
        if bytes > *remaining_budget as u64 {
            return Err(format!(
                "implementation inputs exceed the {MAX_TOTAL_SOURCE_BYTES}-byte cumulative limit"
            ));
        }
        *remaining_budget -= bytes as usize;
        files.push((path.into_bytes(), digest));
    }
    Ok(())
}

fn ensure_branch_crate_directory(
    repo_root: &Path,
    branch: &str,
    crate_root: &str,
) -> Result<(), String> {
    match git_ls_tree_entry_kind_isolated(repo_root, branch, crate_root)? {
        TreeEntryKind::Other(0o040_000) => Ok(()),
        TreeEntryKind::NotFound => {
            Err(format!("layer crate directory '{crate_root}' is unavailable on origin/{branch}"))
        }
        TreeEntryKind::RegularFile => Err(format!("layer crate path '{crate_root}' is a file")),
        TreeEntryKind::Symlink => {
            Err(format!("symlink is not allowed at layer crate path '{crate_root}'"))
        }
        TreeEntryKind::Submodule => {
            Err(format!("submodule is not allowed at layer crate path '{crate_root}'"))
        }
        TreeEntryKind::Other(mode) => {
            Err(format!("unexpected tree entry mode {mode:06o} at layer crate path '{crate_root}'"))
        }
    }
}

fn ensure_optional_branch_vendor_directory(repo_root: &Path, branch: &str) -> Result<bool, String> {
    match git_ls_tree_entry_kind_isolated(repo_root, branch, "vendor")? {
        TreeEntryKind::Other(0o040_000) => Ok(true),
        TreeEntryKind::NotFound => Ok(false),
        TreeEntryKind::RegularFile => Err("vendor path 'vendor' is a file".to_owned()),
        TreeEntryKind::Symlink => Err("symlink is not allowed at vendor path 'vendor'".to_owned()),
        TreeEntryKind::Submodule => {
            Err("submodule is not allowed at vendor path 'vendor'".to_owned())
        }
        TreeEntryKind::Other(mode) => {
            Err(format!("unexpected tree entry mode {mode:06o} at vendor path 'vendor'"))
        }
    }
}

fn branch_tree_paths(
    repo_root: &Path,
    branch: &str,
    crate_root: &str,
    visited_entries: &mut usize,
) -> Result<Vec<String>, String> {
    let git_ref = format!("origin/{branch}");
    let tree_path = format!("{crate_root}/");
    let output = isolated_bounded_git_output(
        repo_root,
        &["ls-tree", "-r", "-z", &git_ref, "--", &tree_path],
        MAX_TREE_OUTPUT_BYTES,
    )
    .map_err(|error| format!("failed to run git ls-tree for {crate_root}: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "git ls-tree failed for {crate_root} (exit {}): {}",
            output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let prefix = format!("{crate_root}/");
    let mut paths = Vec::new();
    for record in output.stdout.split(|byte| *byte == 0).filter(|record| !record.is_empty()) {
        *visited_entries = visited_entries
            .checked_add(1)
            .ok_or_else(|| "implementation source entry count overflowed".to_owned())?;
        if *visited_entries > MAX_SOURCE_ENTRIES {
            return Err(format!(
                "implementation source traversal exceeds maximum of {MAX_SOURCE_ENTRIES} entries"
            ));
        }
        let tab = record
            .iter()
            .position(|byte| *byte == b'\t')
            .ok_or_else(|| format!("git ls-tree returned a malformed record below {crate_root}"))?;
        let mode_bytes = record
            .get(..tab)
            .and_then(|prefix| prefix.split(|byte| *byte == b' ').next())
            .ok_or_else(|| format!("git ls-tree returned a missing mode below {crate_root}"))?;
        let mode_text = std::str::from_utf8(mode_bytes)
            .map_err(|_| format!("git ls-tree returned a non-UTF-8 mode below {crate_root}"))?;
        let mode = u32::from_str_radix(mode_text, 8)
            .map_err(|error| format!("failed to parse git tree mode '{mode_text}': {error}"))?;
        let path = String::from_utf8(
            record
                .get(tab + 1..)
                .ok_or_else(|| format!("git ls-tree returned a missing path below {crate_root}"))?
                .to_vec(),
        )
        .map_err(|_| format!("git ls-tree returned a non-UTF-8 path below {crate_root}"))?;
        if !path.starts_with(&prefix) {
            return Err(format!("git returned a tree path outside '{crate_root}': {path}"));
        }
        if is_vcs_internal(&path) {
            continue;
        }
        if !matches!(mode, 0o100_644 | 0o100_755) {
            return Err(format!(
                "unsafe tree entry mode {mode:06o} below {crate_root} in origin/{branch}"
            ));
        }
        let relative_depth = path[prefix.len()..].split('/').count();
        if relative_depth > MAX_SOURCE_DEPTH {
            return Err(format!(
                "implementation source traversal exceeds maximum depth of {MAX_SOURCE_DEPTH} at '{path}'"
            ));
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

fn is_vcs_internal(path: &str) -> bool {
    path.split('/').any(|component| component == ".git")
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::{LayerCrateRoot, collect_branch_tree_file_digests};
    use std::path::Path;

    fn git(cwd: &Path, args: &[&str]) {
        crate::verify::test_support::git_with_identity(cwd, args);
    }

    fn setup_repo() -> tempfile::TempDir {
        let directory = tempfile::tempdir().unwrap();
        let repo = directory.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        std::fs::create_dir_all(repo.join("libs/domain/src")).unwrap();
        std::fs::write(repo.join("libs/domain/Cargo.toml"), b"[package]\nname = \"domain\"\n")
            .unwrap();
        std::fs::write(repo.join("libs/domain/src/lib.rs"), b"pub struct Domain;\n").unwrap();
        std::fs::write(repo.join("libs/domain/README.md"), b"readme\n").unwrap();
        git(repo, &["add", "libs"]);
        git(repo, &["commit", "--quiet", "-m", "initial"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);
        directory
    }

    #[test]
    fn test_collect_branch_tree_file_digests_includes_non_rust_files() {
        let directory = setup_repo();
        let roots = vec![LayerCrateRoot {
            crate_name: "domain".to_owned(),
            path: "libs/domain".to_owned(),
        }];
        let mut budget = 64 * 1024 * 1024;
        let files = collect_branch_tree_file_digests(directory.path(), "main", &roots, &mut budget)
            .unwrap();
        assert!(files.iter().any(|(path, _)| path == b"libs/domain/README.md"));
        assert!(files.iter().any(|(path, _)| path == b"libs/domain/Cargo.toml"));
    }

    #[test]
    fn test_collect_branch_tree_file_digests_includes_and_updates_vendor_blobs() {
        let directory = setup_repo();
        let repo = directory.path();
        std::fs::create_dir_all(repo.join("vendor/conch-parser/src")).unwrap();
        std::fs::write(
            repo.join("vendor/conch-parser/Cargo.toml"),
            b"[package]\nname = \"conch-parser\"\nversion = \"0.1.1\"\n",
        )
        .unwrap();
        std::fs::write(repo.join("vendor/conch-parser/src/lib.rs"), b"pub struct Before;\n")
            .unwrap();
        git(repo, &["add", "vendor"]);
        git(repo, &["commit", "--quiet", "-m", "vendor"]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let roots = vec![LayerCrateRoot {
            crate_name: "domain".to_owned(),
            path: "libs/domain".to_owned(),
        }];
        let mut budget = 64 * 1024 * 1024;
        let initial = collect_branch_tree_file_digests(repo, "main", &roots, &mut budget).unwrap();
        let (_, initial_digest) =
            initial.iter().find(|(path, _)| path == b"vendor/conch-parser/src/lib.rs").unwrap();

        std::fs::write(repo.join("vendor/conch-parser/src/lib.rs"), b"pub struct After;\n")
            .unwrap();
        git(repo, &["add", "vendor/conch-parser/src/lib.rs"]);
        git(repo, &["commit", "--quiet", "-m", "vendor change"]);
        git(repo, &["fetch", "--quiet", "origin"]);
        let mut budget = 64 * 1024 * 1024;
        let changed = collect_branch_tree_file_digests(repo, "main", &roots, &mut budget).unwrap();
        let (_, changed_digest) =
            changed.iter().find(|(path, _)| path == b"vendor/conch-parser/src/lib.rs").unwrap();

        assert_ne!(initial_digest, changed_digest, "vendor blob changes must affect branch inputs");
    }

    #[test]
    fn test_collect_branch_tree_file_digests_includes_module_limit_excluded_subtree() {
        let directory = setup_repo();
        let repo = directory.path();
        std::fs::create_dir_all(repo.join("libs/domain/tmp")).unwrap();
        std::fs::write(repo.join("libs/domain/tmp/generated"), b"ignored\n").unwrap();
        git(repo, &["add", "libs/domain/tmp/generated"]);
        git(repo, &["commit", "--quiet", "-m", "ignored"]);
        git(repo, &["fetch", "--quiet", "origin"]);
        let roots = vec![LayerCrateRoot {
            crate_name: "domain".to_owned(),
            path: "libs/domain".to_owned(),
        }];
        let mut budget = 64 * 1024 * 1024;
        let files = collect_branch_tree_file_digests(repo, "main", &roots, &mut budget).unwrap();
        assert!(files.iter().any(|(path, _)| path.ends_with(b"generated")));
    }

    #[test]
    fn test_collect_branch_tree_file_digests_rejects_missing_crate_directory() {
        let directory = setup_repo();
        let roots = vec![LayerCrateRoot {
            crate_name: "missing".to_owned(),
            path: "libs/missing".to_owned(),
        }];
        let mut budget = 64 * 1024 * 1024;
        let error = collect_branch_tree_file_digests(directory.path(), "main", &roots, &mut budget)
            .unwrap_err();
        assert!(error.contains("unavailable"), "got: {error}");
    }

    #[cfg(unix)]
    #[test]
    fn test_collect_branch_tree_file_digests_accepts_executable_mode() {
        use std::os::unix::fs::PermissionsExt;

        let directory = setup_repo();
        let repo = directory.path();
        let readme = repo.join("libs/domain/README.md");
        std::fs::set_permissions(&readme, std::fs::Permissions::from_mode(0o755)).unwrap();
        git(repo, &["add", "libs/domain/README.md"]);
        git(repo, &["commit", "--quiet", "-m", "make readme executable"]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let roots = vec![LayerCrateRoot {
            crate_name: "domain".to_owned(),
            path: "libs/domain".to_owned(),
        }];
        let mut budget = 64 * 1024 * 1024;
        let files = collect_branch_tree_file_digests(repo, "main", &roots, &mut budget).unwrap();
        let (_, content_digest) =
            files.iter().find(|(path, _)| path == b"libs/domain/README.md").unwrap();
        assert_ne!(*content_digest, [0; 32]);
    }
}
