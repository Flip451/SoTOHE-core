//! Shared test helpers for the `adr_decision` module suite.
//!
//! Centralises the `git` command wrapper and the `setup_repo_with_adr_blobs`
//! fixture builder so that `git_blob_adapter` tests and
//! `verify::merge_gate_adapter` tests can reuse the same helpers without
//! duplicating the implementation.

#![cfg(test)]
#![allow(dead_code)]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::Path;

/// Run a git command in `cwd` with a deterministic locale and author identity.
///
/// Delegates to [`crate::verify::test_support::git_with_identity`] — the
/// single canonical implementation of the env-var-setting git wrapper shared
/// across infrastructure test modules.
pub(crate) fn git(cwd: &Path, args: &[&str]) {
    crate::verify::test_support::git_with_identity(cwd, args);
}

/// Creates a git repo with the given ADR files committed inside
/// `knowledge/adr/`, then sets `origin` to itself so
/// `origin/main:knowledge/adr/...` resolves.
///
/// When `adrs` is empty, a placeholder `.gitkeep` is committed at the repo
/// root so the initial commit can succeed; `knowledge/adr/` will not exist
/// on the branch, which causes `git_ls_tree_dir` to return an empty list.
pub(crate) fn setup_repo_with_adr_blobs(adrs: &[(&str, &str)]) -> tempfile::TempDir {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path();
    git(repo, &["init", "--quiet", "--initial-branch=main"]);
    if adrs.is_empty() {
        std::fs::write(repo.join(".gitkeep"), b"").unwrap();
        git(repo, &["add", ".gitkeep"]);
    } else {
        let adr_dir = repo.join("knowledge/adr");
        std::fs::create_dir_all(&adr_dir).unwrap();
        for (name, content) in adrs {
            std::fs::write(adr_dir.join(name), content).unwrap();
        }
        git(repo, &["add", "knowledge"]);
    }
    git(repo, &["commit", "--quiet", "-m", "initial"]);
    git(repo, &["remote", "add", "origin", repo.to_str().unwrap()]);
    git(repo, &["fetch", "--quiet", "origin"]);
    tmp
}
