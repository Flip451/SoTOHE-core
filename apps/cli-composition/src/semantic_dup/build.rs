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
        // Only remove it when it is a recognizable LanceDB index (our own stale
        // backup from a crashed run).  If something unrelated already occupies
        // that path, refuse to clobber it and ask the user to resolve the
        // conflict manually.
        //
        // Use `symlink_metadata` (does NOT follow symlinks) so that a broken
        // symlink at `backup_path` also triggers the guard.  `Path::exists()`
        // returns `false` for broken symlinks, which would silently skip the
        // guard and let the subsequent `rename(db_path, backup_path)` overwrite
        // the symlink entry — an unrelated user artefact.
        if std::fs::symlink_metadata(&backup_path).is_ok() {
            if !is_recognizable_lancedb_index(&backup_path) {
                return Err(format!(
                    "the path '{}' already exists and does not appear to be a \
                     LanceDB index (missing '{}' marker); refusing to overwrite \
                     it to prevent data loss. \
                     Please remove or rename '{}' manually and retry.",
                    backup_path.display(),
                    LANCEDB_TABLE_MARKER,
                    backup_path.display(),
                ));
            }
            std::fs::remove_dir_all(&backup_path).map_err(|e| {
                format!("failed to remove stale backup at {}: {e}", backup_path.display())
            })?;
        }

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
        // backup sibling exists AND it is a recognizable LanceDB index, rename
        // the backup back so the old index is restored before we proceed with a
        // fresh rebuild.
        //
        // If `.old` exists but is NOT a recognizable LanceDB index, it is
        // unrelated user data that happened to occupy that path.  Leave it
        // completely untouched — `db_path` being absent is a valid fresh-build
        // state in this case.
        let crash_backup = backup_path_for(&input.db_path)?;
        if !input.db_path.exists()
            && crash_backup.exists()
            && is_recognizable_lancedb_index(&crash_backup)
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
        // Only remove the stale temp dir when it is a recognizable LanceDB
        // index (our own leftover from a prior crashed build).  If an unrelated
        // directory or file already occupies `.{name}.tmp-build`, refuse to
        // clobber it and ask the user to resolve the conflict manually.  This
        // mirrors the same guard applied to the `.old` backup in
        // `commit_rebuilt_index`.
        if std::fs::symlink_metadata(&temp_path).is_ok() {
            if !is_recognizable_lancedb_index(&temp_path) {
                return Err(format!(
                    "the path '{}' already exists and does not appear to be a \
                     LanceDB index (missing '{}' marker); refusing to delete it \
                     to prevent data loss. \
                     Please remove or rename '{}' manually, or choose a \
                     --db-path whose sibling '{}' does not collide.",
                    temp_path.display(),
                    LANCEDB_TABLE_MARKER,
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

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );

        // Build into temp_path.  The adapter is scoped here so it is dropped
        // (connection closed, data flushed) before the rename swap below.
        let build_result = {
            let index_port =
                Arc::new(LanceDbSemanticIndexAdapter::new(temp_path.to_path_buf()).map_err(
                    |e| format!("failed to open temp index at {}: {e}", temp_path.display()),
                )?);

            use usecase::semantic_dup::BuildIndexInteractor;
            let interactor = BuildIndexInteractor::new(embedding_port, index_port);
            interactor
                .build_index(&BuildIndexCommand { fragments })
                .map_err(|e| format!("build-index failed: {e}"))
        }; // `index_port` (and the Arc inside) is dropped here.

        build_result.map(|_| fragment_count)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use super::*;

    /// Create a directory that looks like a LanceDB index (contains the
    /// `fragments.lance/` marker subdirectory plus a sentinel file).
    fn create_fake_lancedb_index(db_path: &std::path::Path) {
        std::fs::create_dir_all(db_path.join(LANCEDB_TABLE_MARKER)).unwrap();
        // Sentinel file lets tests verify tree content after swaps.
        std::fs::write(db_path.join("stale.txt"), "stale").unwrap();
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

        // Place a recognizable stale temp dir (simulates a leftover from a
        // previous crash — it has the `fragments.lance/` marker because the
        // prior run built into it before crashing).
        create_fake_lancedb_index(&temp_path);
        std::fs::write(temp_path.join("stale_temp.txt"), "stale").unwrap();

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let outcome = app.semantic_dup_index_build(input).unwrap();
        // Zero-fragment build succeeds.
        assert_eq!(outcome.exit_code, 0);
        // Recognizable stale temp dir must be cleaned up.
        assert!(
            !temp_path.exists(),
            "recognizable stale temp dir must be removed before build starts"
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

        // Must return Err — refuse to delete unrecognized data.
        assert!(
            result.is_err(),
            "build must return Err when unrecognized dir occupies the temp path"
        );
        let msg = result.unwrap_err();
        assert!(
            msg.contains("does not appear to be a LanceDB index"),
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

        // Simulate crash state: db_path is absent, backup is present.
        create_fake_lancedb_index(&backup_path);
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

        // Recognizable stale backup — this is our own leftover from a prior crash.
        create_fake_lancedb_index(&stale_backup);
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
        assert!(!stale_backup.exists(), "recognizable stale backup must be removed during swap");
        // Temp path must be gone.
        assert!(!temp_path.exists(), "temp_path must be removed after successful swap");
    }
}
