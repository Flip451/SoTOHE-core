//! Low-level `git show` / `git ls-tree` primitives for fail-closed blob
//! retrieval from a git ref.
//!
//! All primitives in this module are `pub(crate)` — they are internal
//! building blocks consumed by the `merge_gate_adapter` which translates
//! them into the usecase-level [`BlobFetchResult<T>`](crate::merge_gate)
//! contract.
//!
//! ## Fail-closed properties
//!
//! - `LANG=C LC_ALL=C LANGUAGE=C` is forced on every git subprocess so
//!   stderr substring matching (`is_path_not_found_stderr`) is stable
//!   across locales (ADR §D4.1).
//! - `fetch_blob_safe` runs `git ls-tree` first to inspect the tree entry
//!   mode, rejecting symlinks (`120000`) and submodules (`160000`) BEFORE
//!   fetching the blob content (ADR §D4.3).
//! - `git_show_blob` returns raw `Vec<u8>` so the caller can apply a strict
//!   UTF-8 decode (not `from_utf8_lossy`) and reject malformed bytes (ADR §D4).
//!
//! Reference: ADR `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md`
//! §D4, §D4.1, §D4.3, §D5.3.

// Until the T009 adapter (`merge_gate_adapter::GitShowTrackBlobReader`)
// consumes these primitives, they have no non-test caller inside the
// infrastructure crate. The allow is lifted automatically once the adapter
// imports `fetch_blob_safe` / `BlobResult`.
#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

// Keep the allow attribute inside the module (inner attribute) so rustfmt
// accepts the ordering: `//!` doc → `#![allow(...)]` → `use` → items.

/// Low-level result of running `git show origin/<ref>:<path>`.
#[derive(Debug)]
pub(crate) enum BlobResult {
    /// Blob content retrieved as raw bytes.
    Found(Vec<u8>),
    /// Blob path does not exist on the remote ref (detected via stderr
    /// substring match, locale-stabilized by `LANG=C`).
    NotFound,
    /// git spawn error OR non-zero exit with non-path-not-found stderr.
    /// The string carries a human-readable description.
    CommandFailed(String),
}

/// Tree entry mode returned by `git ls-tree`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TreeEntryKind {
    /// `100644` or `100755` — regular file (or executable).
    RegularFile,
    /// `120000` — symbolic link. Always rejected by the gate.
    Symlink,
    /// `160000` — gitlink (submodule). Always rejected by the gate.
    Submodule,
    /// Any other mode. Always rejected by the gate (unexpected).
    Other(u32),
    /// `git ls-tree` returned an empty line — the path does not exist
    /// in the tree.
    NotFound,
}

/// Returns `true` when git's stderr indicates the blob path does not
/// exist in the ref.
///
/// Git's canonical English messages for this case are:
/// - `fatal: path '...' does not exist in '...'`
/// - `fatal: path '...' exists on disk, but not in '...'`
///
/// The `LANG=C` enforcement in [`spawn_git`] ensures git always emits
/// English stderr regardless of the operator's locale settings.
pub(crate) fn is_path_not_found_stderr(stderr: &str) -> bool {
    stderr.contains("does not exist in") || stderr.contains("exists on disk, but not in")
}

/// Spawns a `git` subprocess with the locale pinned to `C` so stderr
/// parsing is stable.
///
/// `env_clear` is NOT used — we want to preserve PATH and git-specific
/// env vars (e.g. `GIT_CONFIG`) — but `LANG`, `LC_ALL`, and `LANGUAGE`
/// are explicitly overridden to `C`.
fn spawn_git(repo_root: &Path, args: &[&str]) -> std::io::Result<std::process::Output> {
    Command::new("git")
        .env("LANG", "C")
        .env("LC_ALL", "C")
        .env("LANGUAGE", "C")
        .args(args)
        .current_dir(repo_root)
        .output()
}

/// Inspects `git ls-tree origin/<branch> -- <path>` and returns the
/// tree entry kind for the final path component.
///
/// # Errors
///
/// Returns `Err(String)` on spawn failure or non-zero git exit.
pub(crate) fn git_ls_tree_entry_kind(
    repo_root: &Path,
    branch: &str,
    path: &str,
) -> Result<TreeEntryKind, String> {
    let git_ref = format!("origin/{branch}");
    let output = spawn_git(repo_root, &["ls-tree", &git_ref, "--", path])
        .map_err(|e| format!("failed to run git ls-tree for {path}: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "git ls-tree failed for {path} (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ));
    }

    // `git ls-tree` output format for a matching entry:
    //   <mode> SP <type> SP <hash> TAB <path>\n
    // For a non-existent path it emits nothing (exit 0, empty stdout).
    let stdout = String::from_utf8_lossy(&output.stdout);
    let line = stdout.lines().next().unwrap_or("").trim();
    if line.is_empty() {
        return Ok(TreeEntryKind::NotFound);
    }

    // Parse the mode field (first whitespace-delimited token).
    let mode_str = line.split_whitespace().next().unwrap_or("");
    let mode = u32::from_str_radix(mode_str, 8)
        .map_err(|e| format!("failed to parse tree mode '{mode_str}' for {path}: {e}"))?;

    Ok(match mode {
        0o100_644 | 0o100_755 => TreeEntryKind::RegularFile,
        0o120_000 => TreeEntryKind::Symlink,
        0o160_000 => TreeEntryKind::Submodule,
        other => TreeEntryKind::Other(other),
    })
}

/// Reads the raw bytes of `origin/<branch>:<path>` via `git show`.
///
/// Unlike [`fetch_blob_safe`], this does NOT perform tree-mode inspection
/// beforehand. Callers outside this module MUST use [`fetch_blob_safe`]
/// for any code path that requires symlink/submodule rejection.
pub(crate) fn git_show_blob(repo_root: &Path, branch: &str, path: &str) -> BlobResult {
    let git_ref = format!("origin/{branch}:{path}");
    let output = match spawn_git(repo_root, &["show", &git_ref]) {
        Ok(o) => o,
        Err(e) => {
            return BlobResult::CommandFailed(format!("failed to run git show for {path}: {e}"));
        }
    };

    if output.status.success() {
        return BlobResult::Found(output.stdout);
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if is_path_not_found_stderr(&stderr) {
        BlobResult::NotFound
    } else {
        BlobResult::CommandFailed(format!(
            "git show failed for {path} (exit {}): {}",
            output.status.code().unwrap_or(-1),
            stderr.trim()
        ))
    }
}

/// Safely fetches a blob from `origin/<branch>:<path>` with fail-closed
/// symlink and submodule rejection.
///
/// Two-phase operation:
/// 1. Run `git ls-tree origin/<branch> -- <path>` and inspect the tree mode.
/// 2. If and only if the mode is `100644` / `100755` (regular file), run
///    `git show origin/<branch>:<path>` to retrieve the blob contents.
///
/// Rejected modes map to [`BlobResult::CommandFailed`] with a descriptive
/// message:
/// - `120000` → "symlink is not allowed at {path} (use a regular file)"
/// - `160000` → "submodule is not allowed at {path}"
/// - other    → "unexpected tree entry mode"
///
/// `NotFound` tree entries map to [`BlobResult::NotFound`] so the caller
/// can apply opt-in semantics (e.g. Stage 2 TDDD skip).
pub(crate) fn fetch_blob_safe(repo_root: &Path, branch: &str, path: &str) -> BlobResult {
    match git_ls_tree_entry_kind(repo_root, branch, path) {
        Ok(TreeEntryKind::RegularFile) => git_show_blob(repo_root, branch, path),
        Ok(TreeEntryKind::NotFound) => BlobResult::NotFound,
        Ok(TreeEntryKind::Symlink) => BlobResult::CommandFailed(format!(
            "symlink is not allowed at {path} (use a regular file)"
        )),
        Ok(TreeEntryKind::Submodule) => {
            BlobResult::CommandFailed(format!("submodule is not allowed at {path}"))
        }
        Ok(TreeEntryKind::Other(mode)) => {
            BlobResult::CommandFailed(format!("unexpected tree entry mode {mode:06o} at {path}"))
        }
        Err(msg) => BlobResult::CommandFailed(msg),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use super::*;

    // --- is_path_not_found_stderr ---

    #[test]
    fn test_is_path_not_found_stderr_matches_does_not_exist() {
        assert!(is_path_not_found_stderr("fatal: path 'foo.json' does not exist in 'origin/bar'"));
    }

    #[test]
    fn test_is_path_not_found_stderr_matches_exists_on_disk() {
        assert!(is_path_not_found_stderr(
            "fatal: path 'foo.json' exists on disk, but not in 'origin/bar'"
        ));
    }

    #[test]
    fn test_is_path_not_found_stderr_rejects_bad_revision() {
        assert!(!is_path_not_found_stderr("fatal: bad revision 'origin/missing'"));
    }

    #[test]
    fn test_is_path_not_found_stderr_rejects_empty() {
        assert!(!is_path_not_found_stderr(""));
    }

    // --- Integration tests using a real temp git repo ---
    //
    // Each test creates a fresh git repo in a tempdir, commits fixture
    // content, and then invokes the primitives against the local `origin`
    // (a second clone). This exercises the actual `git show` / `git ls-tree`
    // command path.

    fn git(cwd: &Path, args: &[&str]) {
        let output = Command::new("git")
            .env("LANG", "C")
            .env("LC_ALL", "C")
            .env("LANGUAGE", "C")
            // Use a local identity so commits don't fail on CI agents.
            .env("GIT_AUTHOR_NAME", "test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .args(args)
            .current_dir(cwd)
            .output()
            .expect("git command failed to spawn");
        if !output.status.success() {
            panic!(
                "git {:?} failed: stdout={} stderr={}",
                args,
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }

    fn setup_repo_with_file(filename: &str, contents: &[u8]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        std::fs::write(repo.join(filename), contents).unwrap();
        git(repo, &["add", filename]);
        git(repo, &["commit", "--quiet", "-m", "initial"]);
        // Set up a local `origin` that points to itself so `origin/main` resolves.
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);
        dir
    }

    #[test]
    fn test_git_show_blob_found() {
        let dir = setup_repo_with_file("spec.json", b"{\"schema_version\":1}");
        let result = git_show_blob(dir.path(), "main", "spec.json");
        match result {
            BlobResult::Found(bytes) => {
                assert_eq!(bytes, b"{\"schema_version\":1}");
            }
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_git_show_blob_not_found() {
        let dir = setup_repo_with_file("spec.json", b"{}");
        let result = git_show_blob(dir.path(), "main", "missing.json");
        assert!(matches!(result, BlobResult::NotFound), "got {result:?}");
    }

    #[test]
    fn test_git_show_blob_bad_branch_command_failed() {
        let dir = setup_repo_with_file("spec.json", b"{}");
        let result = git_show_blob(dir.path(), "does-not-exist", "spec.json");
        assert!(matches!(result, BlobResult::CommandFailed(_)), "got {result:?}");
    }

    #[test]
    fn test_git_show_blob_invalid_repo_spawn_error_or_failure() {
        // repo_root is a directory that exists but is not a git repo.
        let dir = tempfile::tempdir().unwrap();
        let result = git_show_blob(dir.path(), "main", "spec.json");
        assert!(matches!(result, BlobResult::CommandFailed(_)), "got {result:?}");
    }

    #[test]
    fn test_git_ls_tree_entry_kind_regular_file() {
        let dir = setup_repo_with_file("spec.json", b"{}");
        let kind = git_ls_tree_entry_kind(dir.path(), "main", "spec.json").unwrap();
        assert_eq!(kind, TreeEntryKind::RegularFile);
    }

    #[test]
    fn test_git_ls_tree_entry_kind_not_found() {
        let dir = setup_repo_with_file("spec.json", b"{}");
        let kind = git_ls_tree_entry_kind(dir.path(), "main", "missing.json").unwrap();
        assert_eq!(kind, TreeEntryKind::NotFound);
    }

    #[cfg(unix)]
    #[test]
    fn test_git_ls_tree_entry_kind_symlink() {
        // Create a repo where spec.json is committed as a symlink (mode 120000).
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        // Create the symlink in the worktree so `git add` records mode 120000.
        std::os::unix::fs::symlink("target.json", repo.join("spec.json")).unwrap();
        std::fs::write(repo.join("target.json"), b"{}").unwrap();
        git(repo, &["add", "spec.json", "target.json"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let kind = git_ls_tree_entry_kind(repo, "main", "spec.json").unwrap();
        assert_eq!(kind, TreeEntryKind::Symlink);
    }

    #[test]
    fn test_fetch_blob_safe_regular_file_passes_through() {
        let dir = setup_repo_with_file("spec.json", b"{\"ok\":true}");
        let result = fetch_blob_safe(dir.path(), "main", "spec.json");
        match result {
            BlobResult::Found(bytes) => assert_eq!(bytes, b"{\"ok\":true}"),
            other => panic!("expected Found, got {other:?}"),
        }
    }

    #[test]
    fn test_fetch_blob_safe_not_found_in_tree() {
        let dir = setup_repo_with_file("spec.json", b"{}");
        let result = fetch_blob_safe(dir.path(), "main", "missing.json");
        assert!(matches!(result, BlobResult::NotFound), "got {result:?}");
    }

    #[cfg(unix)]
    #[test]
    fn test_fetch_blob_safe_rejects_symlink() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path();
        git(repo, &["init", "--quiet", "--initial-branch=main"]);
        std::os::unix::fs::symlink("target.json", repo.join("spec.json")).unwrap();
        std::fs::write(repo.join("target.json"), b"{}").unwrap();
        git(repo, &["add", "spec.json", "target.json"]);
        git(repo, &["commit", "--quiet", "-m", "add symlink"]);
        git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
        git(repo, &["fetch", "--quiet", "origin"]);

        let result = fetch_blob_safe(repo, "main", "spec.json");
        match result {
            BlobResult::CommandFailed(msg) => {
                assert!(msg.contains("symlink"), "expected symlink message, got: {msg}");
            }
            other => panic!("expected CommandFailed(symlink), got {other:?}"),
        }
    }
}
