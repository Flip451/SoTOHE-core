//! `dup-index build` subcommand — input DTO, index helpers, and [`crate::CliApp`] impl.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, extractor::extract_code_fragments,
    index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{BuildIndexCommand, BuildIndexService as _};

use crate::{CliApp, CommandOutcome};

/// Input DTO for `sotp dup-index build`.
#[derive(Debug, Clone)]
pub struct DupIndexBuildInput {
    /// Root of the workspace to scan for Rust sources.
    pub workspace_root: PathBuf,
    /// Path to the local LanceDB database.
    pub db_path: PathBuf,
}

/// The subdirectory name LanceDB creates for the `fragments` table.
///
/// LanceDB stores each table as a `{table_name}.lance/` directory inside the
/// database root.  Checking for this marker lets us distinguish a genuine
/// LanceDB index from an arbitrary directory that the user accidentally
/// pointed `--db-path` at.
const LANCEDB_TABLE_MARKER: &str = "fragments.lance";

/// Hidden file written at the root of every index directory this tool creates.
///
/// This is a tool-ownership marker: it proves that a given directory (the
/// deterministic `.{name}.tmp-build` temp dir or `.{name}.old` backup) was
/// created by `sotp dup-index build`, not by an unrelated LanceDB database that
/// happens to reside at the same deterministic sibling path.  Any recognizable
/// LanceDB index (i.e. one that passes `is_recognizable_lancedb_index`) that
/// lacks this file is treated as foreign and is never auto-restored or
/// auto-deleted by this tool.
const OWNERSHIP_MARKER: &str = ".sotp-semantic-dup-index";

/// Return `true` when `dir` was created by this tool.
///
/// Two conditions must both hold:
///
/// 1. `dir` itself is a real directory (not a symlink).  `std::fs::symlink_metadata` is
///    used so that a symlink at `dir` — even one pointing at a real directory that
///    contains the ownership marker — does NOT count as tool-owned.  The tool never
///    creates these deterministic sibling paths as symlinks, so a symlink there is a
///    foreign artefact that must not be touched.
///
/// 2. `dir/OWNERSHIP_MARKER` exists AND is a regular file.  A symlink at the marker
///    path — even one pointing to a file — does NOT count as owned.
///    `std::fs::symlink_metadata` is used so that a broken symlink at the marker path
///    also returns `false` (no bypass).
fn is_tool_owned_index(dir: &Path) -> bool {
    // Condition 1: `dir` itself must be a real directory (not a symlink).
    match std::fs::symlink_metadata(dir) {
        Ok(m) if m.file_type().is_dir() => {}
        _ => return false,
    }
    // Condition 2: `dir/OWNERSHIP_MARKER` must be a regular file.
    std::fs::symlink_metadata(dir.join(OWNERSHIP_MARKER))
        .map(|m| m.file_type().is_file())
        .unwrap_or(false)
}

/// Return `true` when `db_path` looks like a LanceDB index previously
/// created by this tool.
///
/// The check is intentionally conservative: a directory qualifies as a
/// recognizable index only when it contains the `fragments.lance/`
/// subdirectory that LanceDB creates for the `fragments` table.  The marker
/// must be a real directory (not a file or symlink) to avoid treating an
/// unrelated directory that happens to contain a same-named file or symlink
/// as a valid index.
///
/// `std::fs::symlink_metadata` is used deliberately (it does NOT follow
/// symlinks), so a `fragments.lance` symlink — even one pointing at a
/// directory — does NOT satisfy this check.  This prevents a data-loss bypass
/// where an attacker or accidental user creates a `fragments.lance` symlink
/// inside an unrelated directory to trick the guard into accepting it as a
/// recognizable index and subsequently deleting that directory.
fn is_recognizable_lancedb_index(db_path: &Path) -> bool {
    match std::fs::symlink_metadata(db_path.join(LANCEDB_TABLE_MARKER)) {
        Ok(meta) => meta.file_type().is_dir(),
        Err(_) => false,
    }
}

/// Validate that `db_path` is safe to overwrite during an index rebuild.
///
/// This helper checks both safety guards WITHOUT deleting anything:
///
/// 1. Rejects `db_path` values that are equal to or an ancestor of
///    `workspace_root` (path-overlap data-loss guard).
/// 2. If `db_path` exists but is NOT a recognizable LanceDB index (missing
///    the `fragments.lance/` marker), returns an error — a typo'd path
///    pointing at arbitrary user data must never be clobbered.
/// 3. If `db_path` does not exist → fine; fresh-build path.
///
/// # Errors
///
/// Returns `Err` if the path-overlap guard fires, if `canonicalize` fails,
/// or if the directory exists but is not a recognizable LanceDB index.
fn validate_db_path_for_rebuild(db_path: &Path, workspace_root: &Path) -> Result<(), String> {
    if !db_path.exists() {
        return Ok(());
    }
    let canonical_db = db_path
        .canonicalize()
        .map_err(|e| format!("failed to resolve index path {}: {e}", db_path.display()))?;
    let canonical_workspace = workspace_root.canonicalize().map_err(|e| {
        format!("failed to resolve workspace root {}: {e}", workspace_root.display())
    })?;
    // Reject if db_path is an ancestor of (or equal to) the workspace root.
    if canonical_workspace.starts_with(&canonical_db) {
        return Err(format!(
            "--db-path '{}' overlaps with the workspace root '{}'; \
             refusing to overwrite it to prevent data loss",
            db_path.display(),
            workspace_root.display(),
        ));
    }
    // Reject if the directory exists but is not a recognizable LanceDB index.
    // A typo'd --db-path pointing at an unrelated directory must never be
    // silently and recursively deleted or replaced.
    if !is_recognizable_lancedb_index(db_path) {
        return Err(format!(
            "--db-path '{}' exists but does not appear to be a LanceDB index \
             (missing '{}' marker); refusing to overwrite it to prevent data loss. \
             If this is a new path, remove the directory manually first or \
             choose a path that does not exist yet.",
            db_path.display(),
            LANCEDB_TABLE_MARKER,
        ));
    }
    Ok(())
}

/// Derive the path of the sibling temporary build directory for `db_path`.
///
/// The temp dir is a hidden sibling of `db_path` (same parent directory, so a
/// rename is same-filesystem and therefore atomic).  Using a deterministic name
/// means we can detect and clean up leftovers from a previously crashed run.
fn temp_build_path(db_path: &Path) -> Result<PathBuf, String> {
    let file_name = db_path
        .file_name()
        .ok_or_else(|| format!("--db-path '{}' has no file name component", db_path.display()))?
        .to_string_lossy();
    let parent = db_path
        .parent()
        .ok_or_else(|| format!("--db-path '{}' has no parent directory", db_path.display()))?;
    Ok(parent.join(format!(".{file_name}.tmp-build")))
}

/// Derive the path of the sibling backup directory for `db_path`.
///
/// The backup dir is a hidden sibling of `db_path` (same parent, same
/// filesystem).  Using a deterministic name means we can detect and recover
/// from a crash that happened between moving the original index aside and
/// completing the rename of the new build into place.
fn backup_path_for(db_path: &Path) -> Result<PathBuf, String> {
    let file_name = db_path
        .file_name()
        .ok_or_else(|| format!("--db-path '{}' has no file name component", db_path.display()))?
        .to_string_lossy();
    let parent = db_path
        .parent()
        .ok_or_else(|| format!("--db-path '{}' has no parent directory", db_path.display()))?;
    Ok(parent.join(format!(".{file_name}.old")))
}

/// Atomically promote `temp_path` to `db_path` after a successful rebuild.
///
/// Strategy:
/// - If `db_path` already exists: rename it aside to a backup sibling, rename
///   `temp_path` → `db_path`, then remove the backup (best-effort; failure to
///   remove the backup is non-fatal because the new index is already live).
/// - If `db_path` does not exist: just rename `temp_path` → `db_path`.
///
/// # Errors
///
/// Returns `Err` if a critical rename fails.  If the `temp_path → db_path`
/// rename fails after the existing index has already been moved aside, a
/// best-effort restore of the backup is attempted so `db_path` is left intact
/// (non-destructive rebuild guarantee).  A best-effort backup-removal failure
/// after a successful swap is silently ignored (the new index is already live).
fn commit_rebuilt_index(temp_path: &Path, db_path: &Path) -> Result<(), String> {
    if db_path.exists() {
        let backup_path = backup_path_for(db_path)?;

        // Remove any stale backup from a prior run before renaming the current
        // index aside — avoids a failure if the old backup still exists.
        // Only remove it when it is TOOL-OWNED (has the OWNERSHIP_MARKER file we
        // wrote during the build), confirming we created it.  A recognizable
        // LanceDB index at the backup path that lacks the marker is a foreign
        // database that happens to sit at our deterministic sibling path; refuse
        // to clobber it and ask the user to resolve the conflict manually.
        //
        // Use `symlink_metadata` (does NOT follow symlinks) so that a broken
        // symlink at `backup_path` also triggers the guard.  `Path::exists()`
        // returns `false` for broken symlinks, which would silently skip the
        // guard and let the subsequent `rename(db_path, backup_path)` overwrite
        // the symlink entry — an unrelated user artefact.
        if std::fs::symlink_metadata(&backup_path).is_ok() {
            if !is_tool_owned_index(&backup_path) {
                return Err(format!(
                    "the path '{}' already exists but is not owned by this tool \
                     (missing '{}' ownership marker); refusing to overwrite it to \
                     prevent data loss. \
                     Please remove or rename '{}' manually and retry.",
                    backup_path.display(),
                    OWNERSHIP_MARKER,
                    backup_path.display(),
                ));
            }
            std::fs::remove_dir_all(&backup_path).map_err(|e| {
                format!("failed to remove stale backup at {}: {e}", backup_path.display())
            })?;
        }

        // Write the ownership marker into the existing index before renaming it
        // to the backup path.  A tool-owned backup is required for crash recovery
        // (Step 1b checks `is_tool_owned_index(&crash_backup)`).  If `db_path`
        // was already built by this tool it already has the marker; writing it
        // again is idempotent.  For a legacy index (built before this marker was
        // introduced) the marker is stamped now so the backup is recoverable.
        //
        // This is a hard error, not best-effort: if the write fails (e.g. the
        // index directory is read-only), we must NOT proceed with the rename —
        // an unowned backup left on disk after a crash would not be restored by
        // Step 1b, breaking the non-destructive rebuild guarantee.
        std::fs::write(db_path.join(OWNERSHIP_MARKER), b"sotp-semantic-dup\n").map_err(|e| {
            format!("failed to write ownership marker in existing index {}: {e}", db_path.display())
        })?;

        // Rename current index → backup.
        std::fs::rename(db_path, &backup_path).map_err(|e| {
            format!(
                "failed to rename existing index {} to backup {}: {e}",
                db_path.display(),
                backup_path.display()
            )
        })?;

        // Rename temp build → final location (atomic on same filesystem).
        // If this rename fails, attempt to restore the backup so db_path is
        // left intact (non-destructive rebuild guarantee).
        if let Err(e) = std::fs::rename(temp_path, db_path) {
            // Attempt to restore the backup.  If the restore also fails, report
            // both failures so the user knows the backup location.
            match std::fs::rename(&backup_path, db_path) {
                Ok(()) => {
                    return Err(format!(
                        "failed to rename temp build {} to {} (original index restored from {}): {e}",
                        temp_path.display(),
                        db_path.display(),
                        backup_path.display(),
                    ));
                }
                Err(restore_err) => {
                    return Err(format!(
                        "failed to rename temp build {} to {}: {e}; \
                         additionally, failed to restore backup {} to {}: {restore_err}; \
                         original index is at backup path {}",
                        temp_path.display(),
                        db_path.display(),
                        backup_path.display(),
                        db_path.display(),
                        backup_path.display(),
                    ));
                }
            }
        }

        // Remove the backup — best-effort; failure is non-fatal because the new
        // index is already live at db_path.
        let _ = std::fs::remove_dir_all(&backup_path);
    } else {
        // No existing index — just move temp into place.
        std::fs::rename(temp_path, db_path).map_err(|e| {
            format!(
                "failed to rename temp build {} to {}: {e}",
                temp_path.display(),
                db_path.display()
            )
        })?;
    }
    Ok(())
}

impl CliApp {
    /// Run `sotp dup-index build`: extract workspace fragments, embed each,
    /// and insert into the LanceDB index.
    ///
    /// **Non-destructive rebuild semantics**: the new index is built into a
    /// sibling temporary directory and swapped into place only after the rebuild
    /// fully succeeds.  If any step fails (extraction, model load, insert error),
    /// the temporary directory is removed (best-effort) and the existing index at
    /// `db_path` — if any — is left completely untouched.  This ensures a
    /// previously stale-but-working index never regresses to empty/missing due to
    /// a partial rebuild failure.
    ///
    /// **Crash recovery**: if a previous run was killed between the two renames
    /// in the atomic-swap step (leaving `db_path` absent but the hidden
    /// `.{filename}.old` backup present), this function transparently restores
    /// the backup before proceeding with the new rebuild.
    ///
    /// AC-02: exits 0 on success; no network calls at build time (model weights
    /// are loaded from the local fastembed cache).
    ///
    /// # Errors
    ///
    /// Returns `Err` if validation fails, if extraction fails, if adapter
    /// construction fails, if indexing fails, or if the atomic swap fails.
    pub fn semantic_dup_index_build(
        &self,
        input: DupIndexBuildInput,
    ) -> Result<CommandOutcome, String> {
        // Step 1: Validate that db_path is safe to overwrite — no deletions yet.
        validate_db_path_for_rebuild(&input.db_path, &input.workspace_root)?;

        // Step 1b: Recover from a crash that occurred between the two renames in
        // `commit_rebuilt_index`.  If `db_path` is absent but the deterministic
        // backup sibling exists AND it is TOOL-OWNED (has the OWNERSHIP_MARKER
        // file we wrote), rename the backup back so the old index is restored
        // before we proceed with a fresh rebuild.
        //
        // Ownership (OWNERSHIP_MARKER) is required — not just recognizability —
        // because ANY LanceDB database has a `fragments.lance/` dir.  An
        // unrelated LanceDB index that happens to sit at the deterministic
        // `.{name}.old` path must NOT be restored into `db_path`.  If `.old`
        // exists but is NOT tool-owned, leave it completely untouched — an
        // absent `db_path` is a valid fresh-build state in that case.
        let crash_backup = backup_path_for(&input.db_path)?;
        if !input.db_path.exists()
            && std::fs::symlink_metadata(&crash_backup).is_ok()
            && is_tool_owned_index(&crash_backup)
        {
            std::fs::rename(&crash_backup, &input.db_path).map_err(|e| {
                format!(
                    "found orphaned backup {} from a previous crash but failed to \
                     restore it to {}: {e}",
                    crash_backup.display(),
                    input.db_path.display(),
                )
            })?;
        }

        // Step 2: Derive the temp build path (sibling of db_path).
        let temp_path = temp_build_path(&input.db_path)?;

        // Step 3: Remove any leftover temp dir from a prior crashed run.
        //
        // Use `symlink_metadata` (does NOT follow symlinks) so that a broken
        // symlink at `temp_path` also triggers the guard — `Path::exists()`
        // returns `false` for broken symlinks and would silently bypass it.
        //
        // Only remove the stale temp dir when it is TOOL-OWNED (has the
        // OWNERSHIP_MARKER file we wrote during the prior build).  Ownership
        // is required — not just recognizability — because ANY LanceDB database
        // has a `fragments.lance/` dir, so a foreign database sitting at the
        // deterministic `.{name}.tmp-build` path would otherwise be deleted.
        // If something unrelated occupies that path (foreign LanceDB index or
        // arbitrary file/dir), refuse to clobber it and ask the user to resolve
        // the conflict manually.
        if std::fs::symlink_metadata(&temp_path).is_ok() {
            if !is_tool_owned_index(&temp_path) {
                return Err(format!(
                    "the path '{}' already exists but is not owned by this tool \
                     (missing '{}' ownership marker); refusing to delete it to \
                     prevent data loss. \
                     Please remove or rename '{}' manually, or choose a \
                     --db-path whose sibling '{}' does not collide.",
                    temp_path.display(),
                    OWNERSHIP_MARKER,
                    temp_path.display(),
                    temp_path.display(),
                ));
            }
            std::fs::remove_dir_all(&temp_path).map_err(|e| {
                format!("failed to remove stale temp build dir {}: {e}", temp_path.display())
            })?;
        }

        // Ensure the parent directory of temp_path exists.
        if let Some(parent) = temp_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                format!(
                    "failed to create parent directory for temp build path {}: {e}",
                    temp_path.display()
                )
            })?;
        }

        // Step 4: Run extraction and build into temp_path.
        //         On any failure, clean up temp_path and return the original error.
        let result = self.do_build_into(&input.workspace_root, &temp_path);
        let fragment_count = match result {
            Ok(n) => n,
            Err(e) => {
                // Best-effort cleanup of the temp build dir; ignore cleanup errors.
                let _ = std::fs::remove_dir_all(&temp_path);
                return Err(e);
            }
        };

        if fragment_count == 0 {
            // No Rust sources found — the existing index (if any) is left intact.
            // Deleting it would be destructive if --workspace-root was wrong; the
            // non-destructive rebuild guarantee takes precedence over reflecting
            // an empty workspace state.  Clean up the (empty) temp dir and return.
            let _ = std::fs::remove_dir_all(&temp_path);
            return Ok(CommandOutcome::success(Some(
                "Indexed 0 fragment(s) (no Rust sources found in workspace)".to_owned(),
            )));
        }

        // Step 5: Atomic swap — promote temp_path to db_path.
        commit_rebuilt_index(&temp_path, &input.db_path)?;

        Ok(CommandOutcome::success(Some(format!(
            "Indexed {fragment_count} fragment(s) (extracted: {fragment_count})"
        ))))
    }

    /// Build the index into `temp_path` from `workspace_root`.
    ///
    /// Returns the number of fragments indexed on success.
    ///
    /// The LanceDB adapter is explicitly dropped before this function returns
    /// so that all data is committed and no open handles remain when the caller
    /// performs the atomic rename.
    fn do_build_into(&self, workspace_root: &Path, temp_path: &Path) -> Result<usize, String> {
        let fragments = extract_code_fragments(workspace_root)
            .map_err(|e| format!("fragment extraction failed: {e}"))?;

        if fragments.is_empty() {
            return Ok(0);
        }

        let fragment_count = fragments.len();

        // Create the temp directory and write the ownership marker BEFORE invoking
        // LanceDB.  This closes the crash window that existed when the marker was
        // written only after a successful build: if the process was killed after
        // LanceDB had created `fragments.lance/` inside `temp_path` but before the
        // marker write completed, the next run would see a recognizable (has
        // `fragments.lance/`) but unowned temp dir and Step 3 would refuse to clean
        // it up, blocking all subsequent rebuilds until manual intervention.
        //
        // With the marker written first, any crash state left in `temp_path` is
        // tool-owned and Step 3 will remove it on the next run.  The zero-fragment
        // early-return above exits before this point, so no marker is written (and
        // no `temp_path` is created) when there is nothing to build.
        std::fs::create_dir_all(temp_path).map_err(|e| {
            format!("failed to create temp build directory {}: {e}", temp_path.display())
        })?;
        std::fs::write(temp_path.join(OWNERSHIP_MARKER), b"sotp-semantic-dup\n").map_err(|e| {
            format!("failed to write ownership marker in {}: {e}", temp_path.display())
        })?;

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );

        // Build into temp_path.  The adapter is scoped here so it is dropped
        // (connection closed, data flushed) before the rename swap below.
        {
            let index_port =
                Arc::new(LanceDbSemanticIndexAdapter::new(temp_path.to_path_buf()).map_err(
                    |e| format!("failed to open temp index at {}: {e}", temp_path.display()),
                )?);

            use usecase::semantic_dup::BuildIndexInteractor;
            let interactor = BuildIndexInteractor::new(embedding_port, index_port);
            interactor
                .build_index(&BuildIndexCommand { fragments })
                .map_err(|e| format!("build-index failed: {e}"))
        }?; // `index_port` (and the Arc inside) is dropped here.

        Ok(fragment_count)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Create a directory that looks like a LanceDB index (contains the
    /// `fragments.lance/` marker subdirectory plus a sentinel file).
    ///
    /// NOTE: does NOT write the ownership marker.  Use `create_tool_owned_index`
    /// when the index should also carry the `OWNERSHIP_MARKER` file.
    fn create_fake_lancedb_index(db_path: &std::path::Path) {
        std::fs::create_dir_all(db_path.join(LANCEDB_TABLE_MARKER)).unwrap();
        // Sentinel file lets tests verify tree content after swaps.
        std::fs::write(db_path.join("stale.txt"), "stale").unwrap();
    }

    /// Create a directory that looks like a tool-owned LanceDB index: the
    /// `fragments.lance/` marker plus the `OWNERSHIP_MARKER` regular file.
    fn create_tool_owned_index(db_path: &std::path::Path) {
        create_fake_lancedb_index(db_path);
        std::fs::write(db_path.join(OWNERSHIP_MARKER), b"sotp-semantic-dup\n").unwrap();
    }

    #[test]
    fn test_validate_db_path_for_rebuild_rejects_db_path_equal_to_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().to_path_buf();
        // db_path == workspace_root: the overlap guard must fire.
        let db_path = workspace_root.clone();
        let result = validate_db_path_for_rebuild(&db_path, &workspace_root);
        assert!(result.is_err(), "db_path == workspace_root must be rejected");
        let msg = result.unwrap_err();
        assert!(msg.contains("overlaps"), "error message must mention 'overlaps', got: {msg}");
        // workspace_root must not have been deleted.
        assert!(workspace_root.exists(), "workspace_root must not be deleted by validate");
    }

    #[test]
    fn test_validate_db_path_for_rebuild_rejects_db_path_ancestor_of_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        // workspace_root is a subdir; db_path is the parent → ancestor overlap.
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().to_path_buf(); // parent of workspace_root
        let result = validate_db_path_for_rebuild(&db_path, &workspace_root);
        assert!(result.is_err(), "db_path ancestor of workspace_root must be rejected");
        let msg = result.unwrap_err();
        assert!(msg.contains("overlaps"), "error message must mention 'overlaps', got: {msg}");
        // db_path (the parent dir) must not have been deleted.
        assert!(db_path.exists(), "db_path must not be deleted by validate");
    }

    #[test]
    fn test_validate_db_path_for_rebuild_accepts_valid_lancedb_index() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().join("index.db");
        create_fake_lancedb_index(&db_path);

        let result = validate_db_path_for_rebuild(&db_path, &workspace_root);
        assert!(result.is_ok(), "valid lancedb index should pass validation: {:?}", result.err());
        // Validate must NOT have removed the existing index.
        assert!(db_path.exists(), "validate must not delete the existing index");
        assert!(
            db_path.join(LANCEDB_TABLE_MARKER).exists(),
            "validate must not delete the LanceDB marker"
        );
    }

    #[test]
    fn test_validate_db_path_for_rebuild_rejects_non_index_directory() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().join("some_other_dir");
        // Create a directory WITHOUT the LanceDB marker — simulates a typo'd path.
        std::fs::create_dir_all(&db_path).unwrap();
        std::fs::write(db_path.join("important_data.txt"), "user data").unwrap();
        assert!(db_path.exists(), "pre-condition: db_path must exist");

        let result = validate_db_path_for_rebuild(&db_path, &workspace_root);
        assert!(result.is_err(), "non-index directory must be rejected");
        let msg = result.unwrap_err();
        assert!(
            msg.contains("does not appear to be a LanceDB index"),
            "error message must explain the rejection, got: {msg}"
        );
        // The directory must NOT have been deleted.
        assert!(db_path.exists(), "non-index directory must not be deleted");
        assert!(
            db_path.join("important_data.txt").exists(),
            "user data inside non-index directory must be preserved"
        );
    }

    #[test]
    fn test_validate_db_path_for_rebuild_is_noop_when_db_path_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().join("nonexistent.db");
        // db_path does not exist — function must be a no-op and succeed.
        let result = validate_db_path_for_rebuild(&db_path, &workspace_root);
        assert!(result.is_ok(), "no-op on absent path must succeed: {:?}", result.err());
        assert!(!db_path.exists(), "absent db_path must still not exist after validate");
    }

    // ── commit_rebuilt_index: atomic swap ────────────────────────────────────

    #[test]
    fn test_commit_rebuilt_index_replaces_existing_index_with_temp_content() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");

        // Pre-populate the existing index with "old" content.
        create_fake_lancedb_index(&db_path);
        std::fs::write(db_path.join("old_marker.txt"), "old").unwrap();

        // Build the temp index with "new" content.
        create_fake_lancedb_index(&temp_path);
        std::fs::write(temp_path.join("new_marker.txt"), "new").unwrap();

        let result = commit_rebuilt_index(&temp_path, &db_path);
        assert!(result.is_ok(), "commit_rebuilt_index should succeed: {:?}", result.err());

        // db_path must now contain the temp content (new_marker.txt).
        assert!(db_path.exists(), "db_path must exist after swap");
        assert!(
            db_path.join("new_marker.txt").exists(),
            "db_path must contain the newly built content"
        );
        // Old content must be gone.
        assert!(
            !db_path.join("old_marker.txt").exists(),
            "db_path must no longer contain old content"
        );
        // Temp path must be gone.
        assert!(!temp_path.exists(), "temp_path must be removed after successful swap");
    }

    #[test]
    fn test_commit_rebuilt_index_moves_temp_into_place_when_db_path_absent() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");

        // No existing db_path — fresh build scenario.
        create_fake_lancedb_index(&temp_path);
        std::fs::write(temp_path.join("new_marker.txt"), "new").unwrap();

        let result = commit_rebuilt_index(&temp_path, &db_path);
        assert!(result.is_ok(), "commit_rebuilt_index should succeed: {:?}", result.err());

        assert!(db_path.exists(), "db_path must exist after rename");
        assert!(db_path.join("new_marker.txt").exists(), "db_path must contain the temp content");
        assert!(!temp_path.exists(), "temp_path must be gone after rename");
    }

    #[test]
    fn test_existing_index_survives_simulated_build_failure() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");

        // Existing working index.
        create_fake_lancedb_index(&db_path);
        std::fs::write(db_path.join("working_sentinel.txt"), "working").unwrap();

        // Simulate build failure: temp dir was partially created then abandoned.
        create_fake_lancedb_index(&temp_path);

        // Caller (semantic_dup_index_build) removes temp on error and does NOT
        // call commit_rebuilt_index.  Simulate that cleanup here.
        std::fs::remove_dir_all(&temp_path).unwrap();

        // db_path must be completely intact.
        assert!(db_path.exists(), "existing index must survive a build failure");
        assert!(
            db_path.join("working_sentinel.txt").exists(),
            "existing index content must be untouched"
        );
        assert!(
            db_path.join(LANCEDB_TABLE_MARKER).exists(),
            "existing index marker must be untouched"
        );
        assert!(!temp_path.exists(), "temp_path must have been cleaned up");
    }

    // ── semantic_dup_index_build: public-API critical paths ──────────────────

    #[test]
    fn test_index_build_cleans_recognizable_stale_temp_dir_before_build() {
        // When `.{name}.tmp-build` exists AND is a recognizable LanceDB index
        // (our own leftover from a prior crashed run), it must be cleaned up
        // before the build starts.
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        // workspace has no .rs files → extract_code_fragments returns empty
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");

        // Place a tool-owned stale temp dir (simulates a leftover from a
        // previous crash — it has the `fragments.lance/` marker AND the
        // ownership marker because the prior run built into it before crashing).
        create_tool_owned_index(&temp_path);
        std::fs::write(temp_path.join("stale_temp.txt"), "stale").unwrap();

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let outcome = app.semantic_dup_index_build(input).unwrap();
        // Zero-fragment build succeeds.
        assert_eq!(outcome.exit_code, 0);
        // Tool-owned stale temp dir must be cleaned up.
        assert!(
            !temp_path.exists(),
            "tool-owned stale temp dir must be removed before build starts"
        );
        // db_path was absent and zero fragments were found → must still be absent.
        assert!(!db_path.exists(), "db_path must not be created when no fragments are indexed");
    }

    #[test]
    fn test_index_build_refuses_to_delete_unrecognized_stale_temp_dir() {
        // Step 3 guard: when `.{name}.tmp-build` exists but is NOT a
        // recognizable LanceDB index, the build must return Err and leave the
        // directory completely untouched.
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        // workspace has no .rs files — Step 3 errors before do_build_into anyway.
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");

        // Create an unrelated dir at the temp path — no `fragments.lance/`
        // marker, with a sentinel file to verify it is untouched after the call.
        std::fs::create_dir_all(&temp_path).unwrap();
        std::fs::write(temp_path.join("unrelated_sentinel.txt"), "precious_data").unwrap();
        assert!(temp_path.exists(), "pre-condition: temp_path must exist");
        assert!(!temp_path.join(LANCEDB_TABLE_MARKER).exists(), "pre-condition: no marker present");

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let result = app.semantic_dup_index_build(input);

        // Must return Err — refuse to delete data not owned by this tool.
        assert!(
            result.is_err(),
            "build must return Err when unrecognized dir occupies the temp path"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("not owned by this tool"),
            "error message must explain the rejection, got: {msg}"
        );

        // The unrelated temp dir and its sentinel must be completely intact.
        assert!(temp_path.exists(), "unrelated temp dir must NOT be deleted on Err");
        assert!(
            temp_path.join("unrelated_sentinel.txt").exists(),
            "sentinel file inside unrelated temp dir must be preserved"
        );
    }

    #[test]
    fn test_index_build_restores_crash_backup_before_rebuild() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        // workspace has no .rs files → extract_code_fragments returns empty
        let db_path = dir.path().join("index.db");
        let backup_path = dir.path().join(".index.db.old");

        // Simulate crash state: db_path is absent, backup is present and tool-owned.
        create_tool_owned_index(&backup_path);
        std::fs::write(backup_path.join("crash_sentinel.txt"), "crash").unwrap();
        assert!(!db_path.exists(), "pre-condition: db_path must be absent");
        assert!(backup_path.exists(), "pre-condition: backup must be present");

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let outcome = app.semantic_dup_index_build(input).unwrap();
        assert_eq!(outcome.exit_code, 0);

        // The crash-backup recovery step must have restored the backup to db_path
        // before the build ran (zero fragments → db_path left intact after
        // restoration).
        assert!(db_path.exists(), "crash backup must be restored to db_path");
        assert!(
            db_path.join("crash_sentinel.txt").exists(),
            "restored db_path must contain original content"
        );
        assert!(!backup_path.exists(), "backup must be gone after restore");
    }

    #[test]
    fn test_index_build_does_not_restore_unrecognized_old_sibling() {
        // Step 1b crash-recovery guard: when `.old` exists but is NOT a
        // recognizable LanceDB index, it must be left completely untouched.
        // `db_path` being absent is treated as a valid fresh-build state.
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        // workspace has no .rs files → extract_code_fragments returns empty
        let db_path = dir.path().join("index.db");
        let unrelated_old = dir.path().join(".index.db.old");

        // Set up an unrelated dir at the `.old` path — no `fragments.lance/` marker.
        std::fs::create_dir_all(&unrelated_old).unwrap();
        std::fs::write(unrelated_old.join("user_data.txt"), "precious").unwrap();

        assert!(!db_path.exists(), "pre-condition: db_path must be absent");
        assert!(unrelated_old.exists(), "pre-condition: unrelated .old dir must exist");

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let outcome = app.semantic_dup_index_build(input).unwrap();
        assert_eq!(outcome.exit_code, 0);

        // The unrelated `.old` dir must be completely untouched.
        assert!(unrelated_old.exists(), "unrelated .old dir must not be touched by crash-recovery");
        assert!(
            unrelated_old.join("user_data.txt").exists(),
            "sentinel file inside unrelated .old dir must be preserved"
        );
        // db_path must remain absent (zero fragments → fresh-build, no restore).
        assert!(!db_path.exists(), "db_path must not be created when unrelated .old was skipped");
    }

    #[test]
    fn test_commit_rebuilt_index_refuses_to_remove_unrecognized_backup_path() {
        // commit_rebuilt_index must return Err (not delete) when `backup_path`
        // exists but is not a recognizable LanceDB index.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");
        let unrelated_old = dir.path().join(".index.db.old");

        // Existing db_path = recognizable fake index.
        create_fake_lancedb_index(&db_path);
        std::fs::write(db_path.join("current_sentinel.txt"), "current").unwrap();

        // Unrelated dir at the backup path — no `fragments.lance/` marker.
        std::fs::create_dir_all(&unrelated_old).unwrap();
        std::fs::write(unrelated_old.join("user_data.txt"), "precious").unwrap();

        // temp = recognizable fake index.
        create_fake_lancedb_index(&temp_path);
        std::fs::write(temp_path.join("new_marker.txt"), "new").unwrap();

        let result = commit_rebuilt_index(&temp_path, &db_path);
        assert!(
            result.is_err(),
            "commit_rebuilt_index must return Err when backup_path is not a LanceDB index"
        );

        // The unrelated `.old` dir must be preserved.
        assert!(unrelated_old.exists(), "unrelated backup_path must not be deleted");
        assert!(
            unrelated_old.join("user_data.txt").exists(),
            "sentinel file in unrelated backup_path must be preserved"
        );
        // db_path must still be intact (rename aside should not have happened yet).
        assert!(
            db_path.exists(),
            "db_path must be preserved when commit_rebuilt_index returns Err"
        );
    }

    /// Verify that a *broken* symlink at `backup_path` (i.e. one whose target
    /// does not exist) is treated as "backup path occupied by something
    /// unrecognized" and causes `commit_rebuilt_index` to return `Err` rather
    /// than silently overwriting it with the `rename(db_path, backup_path)` call.
    ///
    /// `Path::exists()` returns `false` for broken symlinks, so the old guard
    /// `if backup_path.exists()` would skip this block entirely and let the
    /// subsequent rename clobber the symlink entry — an unrelated user artefact.
    /// The fix uses `symlink_metadata` (does NOT follow symlinks) instead.
    #[cfg(unix)]
    #[test]
    fn test_commit_rebuilt_index_refuses_to_clobber_broken_symlink_at_backup_path() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");
        let backup_path = dir.path().join(".index.db.old");

        // Existing db_path = recognizable fake index.
        create_fake_lancedb_index(&db_path);
        std::fs::write(db_path.join("current_sentinel.txt"), "current").unwrap();

        // Create a broken symlink at the backup path — its target does not exist.
        // `Path::exists()` returns false for broken symlinks; `symlink_metadata`
        // returns Ok so the guard fires correctly.
        let nonexistent_target = dir.path().join("does_not_exist");
        std::os::unix::fs::symlink(&nonexistent_target, &backup_path).unwrap();
        // Verify the test precondition: broken symlink is invisible to `exists()`.
        assert!(
            !backup_path.exists(),
            "pre-condition: broken symlink must NOT satisfy Path::exists()"
        );
        assert!(
            backup_path.symlink_metadata().is_ok(),
            "pre-condition: broken symlink MUST be visible to symlink_metadata"
        );

        // temp = recognizable fake index.
        create_fake_lancedb_index(&temp_path);
        std::fs::write(temp_path.join("new_marker.txt"), "new").unwrap();

        let result = commit_rebuilt_index(&temp_path, &db_path);
        assert!(
            result.is_err(),
            "commit_rebuilt_index must return Err when a broken symlink occupies backup_path"
        );

        // The broken symlink entry itself must not have been removed or replaced.
        assert!(
            backup_path.symlink_metadata().is_ok(),
            "broken symlink at backup_path must not be removed"
        );
        // db_path must be completely intact (rename aside must not have happened).
        assert!(db_path.exists(), "db_path must be preserved when Err is returned");
        assert!(db_path.join("current_sentinel.txt").exists(), "db_path content must be untouched");
    }

    #[test]
    fn test_commit_rebuilt_index_removes_recognizable_stale_backup_on_swap() {
        // When `backup_path` IS a recognizable LanceDB index (stale from a prior
        // crashed run), commit_rebuilt_index must remove it and complete the swap.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");
        let stale_backup = dir.path().join(".index.db.old");

        // Existing db_path = recognizable fake index.
        create_fake_lancedb_index(&db_path);
        std::fs::write(db_path.join("current_sentinel.txt"), "current").unwrap();

        // Tool-owned stale backup — this is our own leftover from a prior crash.
        create_tool_owned_index(&stale_backup);
        std::fs::write(stale_backup.join("stale_sentinel.txt"), "stale_backup").unwrap();

        // temp = recognizable fake index with new content.
        create_fake_lancedb_index(&temp_path);
        std::fs::write(temp_path.join("new_marker.txt"), "new").unwrap();

        let result = commit_rebuilt_index(&temp_path, &db_path);
        assert!(
            result.is_ok(),
            "commit_rebuilt_index must succeed when backup_path is a recognizable index: {:?}",
            result.err()
        );

        // db_path must now contain the new content.
        assert!(db_path.exists(), "db_path must exist after swap");
        assert!(
            db_path.join("new_marker.txt").exists(),
            "db_path must contain the newly built content"
        );
        // Stale backup must be gone (removed during swap).
        assert!(!stale_backup.exists(), "tool-owned stale backup must be removed during swap");
        // Temp path must be gone.
        assert!(!temp_path.exists(), "temp_path must be removed after successful swap");
    }

    // ── is_tool_owned_index: unit tests ─────────────────────────────────────

    #[test]
    fn test_is_tool_owned_index_returns_true_when_marker_is_regular_file() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("my.db");
        create_tool_owned_index(&index_path);
        assert!(
            is_tool_owned_index(&index_path),
            "is_tool_owned_index must return true when OWNERSHIP_MARKER is a regular file"
        );
    }

    #[test]
    fn test_is_tool_owned_index_returns_false_when_marker_absent() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("my.db");
        // Recognizable index without ownership marker.
        create_fake_lancedb_index(&index_path);
        assert!(
            !is_tool_owned_index(&index_path),
            "is_tool_owned_index must return false when OWNERSHIP_MARKER is absent"
        );
    }

    #[test]
    fn test_is_tool_owned_index_returns_false_when_marker_is_a_directory() {
        let dir = tempfile::tempdir().unwrap();
        let index_path = dir.path().join("my.db");
        create_fake_lancedb_index(&index_path);
        // Create the ownership marker path as a directory instead of a file.
        std::fs::create_dir_all(index_path.join(OWNERSHIP_MARKER)).unwrap();
        assert!(
            !is_tool_owned_index(&index_path),
            "is_tool_owned_index must return false when OWNERSHIP_MARKER is a directory"
        );
    }

    /// A symlink at `dir` that points to a real tool-owned directory must NOT count
    /// as tool-owned.  The tool never creates the deterministic sibling paths as
    /// symlinks, so a symlink there is a foreign artefact that must not be touched.
    #[cfg(unix)]
    #[test]
    fn test_is_tool_owned_index_returns_false_when_dir_itself_is_a_symlink() {
        let dir = tempfile::tempdir().unwrap();
        // Create a real tool-owned index at `real_index`.
        let real_index = dir.path().join("real.db");
        create_tool_owned_index(&real_index);
        // Create a symlink at `symlink_index` pointing to `real_index`.
        let symlink_index = dir.path().join("symlink.db");
        std::os::unix::fs::symlink(&real_index, &symlink_index).unwrap();
        // Verify the test precondition.
        assert!(
            real_index.join(OWNERSHIP_MARKER).exists(),
            "pre-condition: real index must have ownership marker"
        );
        assert!(
            symlink_index.symlink_metadata().unwrap().file_type().is_symlink(),
            "pre-condition: symlink_index must be a symlink"
        );
        // Even though the symlink points to a tool-owned directory, the symlink
        // itself is not a real directory — it must not be treated as tool-owned.
        assert!(
            !is_tool_owned_index(&symlink_index),
            "is_tool_owned_index must return false when dir itself is a symlink"
        );
    }

    // ── Ownership semantics: crash-recovery guards ───────────────────────────

    #[test]
    fn test_crash_recovery_does_not_restore_recognizable_but_unowned_old() {
        // Step 1b: a recognizable `.old` that lacks OWNERSHIP_MARKER must NOT be
        // restored into `db_path`.  `db_path` absent + unowned `.old` →
        // `.old` and its sentinel survive untouched; build proceeds normally.
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        // workspace has no .rs files → zero-fragment build
        let db_path = dir.path().join("index.db");
        let old_path = dir.path().join(".index.db.old");

        // Place a recognizable (has fragments.lance/) but NOT tool-owned `.old`.
        create_fake_lancedb_index(&old_path);
        std::fs::write(old_path.join("foreign_sentinel.txt"), "foreign_data").unwrap();
        assert!(!db_path.exists(), "pre-condition: db_path must be absent");
        assert!(old_path.exists(), "pre-condition: .old must be present");
        assert!(!is_tool_owned_index(&old_path), "pre-condition: .old must NOT be tool-owned");

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let outcome = app.semantic_dup_index_build(input).unwrap();
        assert_eq!(outcome.exit_code, 0, "zero-fragment build must succeed");

        // `.old` and its sentinel must survive completely untouched.
        assert!(old_path.exists(), "unowned recognizable .old must NOT be restored/deleted");
        assert!(
            old_path.join("foreign_sentinel.txt").exists(),
            "sentinel file inside unowned .old must be preserved"
        );
        assert!(
            old_path.join(LANCEDB_TABLE_MARKER).exists(),
            "LanceDB marker inside unowned .old must be preserved"
        );
        // db_path must remain absent (no restore occurred).
        assert!(!db_path.exists(), "db_path must remain absent when unowned .old was skipped");
    }

    #[test]
    fn test_crash_recovery_restores_tool_owned_old_to_db_path() {
        // Step 1b: a tool-owned `.old` MUST be restored into `db_path`.
        // `db_path` absent + tool-owned `.old` →
        // `.old` content is renamed back to `db_path`.
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        // workspace has no .rs files → zero-fragment build (db_path preserved after restore)
        let db_path = dir.path().join("index.db");
        let old_path = dir.path().join(".index.db.old");

        // Place a tool-owned `.old` simulating a mid-swap crash.
        create_tool_owned_index(&old_path);
        std::fs::write(old_path.join("restored_sentinel.txt"), "restored_data").unwrap();
        assert!(!db_path.exists(), "pre-condition: db_path must be absent");
        assert!(old_path.exists(), "pre-condition: .old must be present");
        assert!(is_tool_owned_index(&old_path), "pre-condition: .old must be tool-owned");

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let outcome = app.semantic_dup_index_build(input).unwrap();
        assert_eq!(outcome.exit_code, 0, "build must succeed after crash recovery");

        // The `.old` content must have been moved to `db_path`.
        assert!(db_path.exists(), "db_path must exist after crash-recovery restore");
        assert!(
            db_path.join("restored_sentinel.txt").exists(),
            "restored db_path must contain the original content"
        );
        assert!(!old_path.exists(), "tool-owned .old must be gone after restore");
    }

    // ── Ownership semantics: commit_rebuilt_index guards ─────────────────────

    #[test]
    fn test_commit_rebuilt_index_returns_err_and_preserves_recognizable_unowned_old() {
        // `backup_path` exists + is recognizable (has fragments.lance/) but NOT
        // tool-owned → commit_rebuilt_index must return Err and leave it intact.
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");
        let backup_path = dir.path().join(".index.db.old");

        // Existing db_path.
        create_fake_lancedb_index(&db_path);
        std::fs::write(db_path.join("current_sentinel.txt"), "current").unwrap();

        // Recognizable but NOT tool-owned backup at the deterministic sibling path.
        create_fake_lancedb_index(&backup_path);
        std::fs::write(backup_path.join("foreign_sentinel.txt"), "foreign_data").unwrap();

        // temp = tool-owned fake index.
        create_tool_owned_index(&temp_path);
        std::fs::write(temp_path.join("new_marker.txt"), "new").unwrap();

        let result = commit_rebuilt_index(&temp_path, &db_path);
        assert!(
            result.is_err(),
            "commit_rebuilt_index must return Err for recognizable-but-unowned backup_path"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("not owned by this tool"),
            "error message must mention ownership, got: {msg}"
        );

        // The unowned backup must be completely preserved.
        assert!(backup_path.exists(), "unowned recognizable backup_path must not be deleted");
        assert!(
            backup_path.join("foreign_sentinel.txt").exists(),
            "sentinel inside unowned backup_path must be preserved"
        );
        // db_path must remain intact (rename aside must not have occurred).
        assert!(db_path.exists(), "db_path must be preserved when Err is returned");
        assert!(
            db_path.join("current_sentinel.txt").exists(),
            "db_path content must be untouched when Err is returned"
        );
    }

    // ── Ownership semantics: Step 3 tmp-build guard ──────────────────────────

    #[test]
    fn test_step3_returns_err_and_preserves_recognizable_unowned_tmp_build() {
        // Step 3: `temp_path` exists + is recognizable (has fragments.lance/) but
        // NOT tool-owned → build must return Err and leave it intact.
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");

        // Recognizable (has fragments.lance/) but NOT tool-owned at temp path.
        create_fake_lancedb_index(&temp_path);
        std::fs::write(temp_path.join("foreign_sentinel.txt"), "foreign_data").unwrap();
        assert!(!is_tool_owned_index(&temp_path), "pre-condition: temp_path must NOT be owned");

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let result = app.semantic_dup_index_build(input);
        assert!(
            result.is_err(),
            "build must return Err when recognizable-but-unowned dir occupies temp_path"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("not owned by this tool"),
            "error message must mention ownership, got: {msg}"
        );

        // temp_path and its sentinel must survive untouched.
        assert!(temp_path.exists(), "unowned recognizable temp_path must NOT be deleted");
        assert!(
            temp_path.join("foreign_sentinel.txt").exists(),
            "sentinel inside unowned temp_path must be preserved"
        );
        assert!(
            temp_path.join(LANCEDB_TABLE_MARKER).exists(),
            "LanceDB marker inside unowned temp_path must be preserved"
        );
    }
}
