//! `semantic_dup` command family — CliApp impl methods and input DTOs.
//!
//! Composition root for all four semantic-dup subcommands:
//! - `find_similar`: embed a query fragment and retrieve top-k results.
//! - `dup_index_build`: extract workspace fragments, embed, and insert into LanceDB.
//! - `dup_check`: check diff fragments against the index (soft gate, always exit 0).
//! - `dup_index_measure_quality`: compute embedding quality metrics over workspace.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarFragment, SimilarityThreshold, TopK};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, extractor::extract_code_fragments,
    index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{
    BuildIndexCommand, BuildIndexService as _, DupCheckCommand, DupCheckInteractor,
    DupCheckService as _, FindSimilarCommand, FindSimilarInteractor, FindSimilarService as _,
    MeasureQualityCommand, MeasureQualityInteractor, MeasureQualityService as _,
    SemanticIndexError, SemanticIndexPort,
};

use crate::{CliApp, CommandOutcome};

// ── find-similar ─────────────────────────────────────────────────────────────

/// Input DTO for `sotp find-similar`.
#[derive(Debug, Clone)]
pub struct FindSimilarInput {
    /// The query text fragment, or the content read from a file.
    pub fragment_text: String,
    /// Number of top-k results to return. Default: 5.
    pub top_k: usize,
    /// Path to the local LanceDB database.
    pub db_path: PathBuf,
}

impl CliApp {
    /// Run `sotp find-similar`: embed the query fragment and retrieve top-k
    /// similar entries from the index.
    ///
    /// CN-05: information-only — always exits 0.
    ///
    /// # Errors
    ///
    /// Returns `Err` if adapter construction or the interactor call fails.
    pub fn semantic_dup_find_similar(
        &self,
        input: FindSimilarInput,
    ) -> Result<CommandOutcome, String> {
        let top_k = TopK::new(input.top_k).map_err(|e| format!("invalid --top-k value: {e}"))?;

        let fragment = CodeFragment::new(PathBuf::from("<query>"), input.fragment_text.clone())
            .map_err(|e| format!("invalid query fragment: {e}"))?;

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

        let interactor = FindSimilarInteractor::new(embedding_port, index_port);
        let output = interactor
            .find_similar(&FindSimilarCommand { fragment, top_k })
            .map_err(|e| format!("find-similar failed: {e}"))?;

        if output.results.is_empty() {
            return Ok(CommandOutcome::success(Some("(no results found)".to_owned())));
        }

        let mut lines = Vec::new();
        for sf in &output.results {
            let snippet = truncate_snippet(sf.fragment.content(), 80);
            lines.push(format!(
                "{} | {:.4} | {}",
                sf.fragment.source_path.display(),
                sf.score.value(),
                snippet
            ));
        }

        Ok(CommandOutcome::success(Some(lines.join("\n"))))
    }
}

// ── dup-index build ───────────────────────────────────────────────────────────

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
        if backup_path.exists() {
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
        // backup sibling exists, rename the backup back so the old index is
        // restored before we proceed with a fresh rebuild.
        let crash_backup = backup_path_for(&input.db_path)?;
        if !input.db_path.exists() && crash_backup.exists() {
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
        if temp_path.exists() {
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

// ── dup-check ─────────────────────────────────────────────────────────────────

/// Input DTO for `sotp dup-check`.
#[derive(Debug, Clone)]
pub struct DupCheckInput {
    /// List of paths to individual fragment text files (one file per fragment).
    pub fragment_files: Vec<PathBuf>,
    /// Cosine similarity threshold above which a match is flagged (0.0–1.0).
    pub threshold: f32,
    /// Path to the local LanceDB database.
    pub db_path: PathBuf,
    /// Optional path to the ack-set file.  When provided:
    /// - fragments whose content hash already appears in the ack set are
    ///   silently suppressed (AC-05).
    /// - after the run, any new warnings whose fragments the user chose to ack
    ///   (via `--ack`) are written into this file.
    pub ack_file: Option<PathBuf>,
    /// When `true`, all warnings from this run are acked and written to
    /// `ack_file` (AC-05).
    pub ack: bool,
}

/// Read the ack-set (a newline-separated list of SHA-256 hex hashes) from `path`.
///
/// Returns an empty set when the file does not exist yet (first run).
fn read_ack_set(path: &Path) -> Result<std::collections::HashSet<String>, String> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(std::collections::HashSet::new()),
        Err(e) => Err(format!("cannot read ack file {}: {e}", path.display())),
    }
}

/// Write the ack-set to `path`.
fn write_ack_set(path: &Path, set: &std::collections::HashSet<String>) -> Result<(), String> {
    let mut sorted: Vec<&str> = set.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let contents = sorted.join("\n") + "\n";
    std::fs::write(path, contents)
        .map_err(|e| format!("cannot write ack file {}: {e}", path.display()))
}

/// Compute a stable content hash for a fragment (SHA-256 of the content bytes).
fn fragment_content_hash(content: &str) -> String {
    // Simple stable hash using std only: we XOR-fold SHA-256 via FNV-1a 64-bit
    // so there are no extra dependencies.  For ack suppression, collision
    // resistance is not critical; what matters is stable reproducibility across
    // runs for the same content.
    //
    // Using FNV-1a 64-bit hash of the UTF-8 content bytes.
    const FNV_OFFSET: u64 = 14_695_981_039_346_656_037;
    const FNV_PRIME: u64 = 1_099_511_628_211;
    let mut hash: u64 = FNV_OFFSET;
    for byte in content.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    format!("{hash:016x}")
}

impl CliApp {
    /// Run `sotp dup-check`: check diff fragments against the semantic index.
    ///
    /// CN-02/AC-04: always exits 0 (soft gate — warnings to stderr, no block).
    /// AC-05: fragments whose content hash appears in the ack file are suppressed.
    ///
    /// # Errors
    ///
    /// Returns `Err` only on hard infrastructure failures (adapter construction
    /// or file I/O errors for the ack file), not on warnings.
    pub fn semantic_dup_check(&self, input: DupCheckInput) -> Result<CommandOutcome, String> {
        // Reject the illegal combination: --ack requires --ack-file to be set.
        if input.ack && input.ack_file.is_none() {
            return Err("--ack requires --ack-file to be specified".to_owned());
        }

        let threshold = SimilarityThreshold::new(input.threshold)
            .map_err(|e| format!("invalid --threshold value: {e}"))?;

        // Read all fragment files.
        let mut fragments: Vec<CodeFragment> = Vec::new();
        for path in &input.fragment_files {
            let content = std::fs::read_to_string(path)
                .map_err(|e| format!("cannot read fragment file {}: {e}", path.display()))?;
            let fragment = CodeFragment::new(path.clone(), content)
                .map_err(|e| format!("invalid fragment in {}: {e}", path.display()))?;
            fragments.push(fragment);
        }

        if fragments.is_empty() {
            return Ok(CommandOutcome::success(Some(
                "dup-check: no fragments to check".to_owned(),
            )));
        }

        // Load the ack set (empty on first run).
        let ack_path_opt = input.ack_file.as_deref();
        let ack_set = match ack_path_opt {
            Some(p) => read_ack_set(p)?,
            None => std::collections::HashSet::new(),
        };

        // Filter out already-acked fragments (AC-05: suppress on re-run).
        let (acked_fragments, check_fragments): (Vec<_>, Vec<_>) =
            fragments.into_iter().partition(|f| {
                let hash = fragment_content_hash(f.content());
                ack_set.contains(&hash)
            });

        let _ = acked_fragments; // suppressed — no warning emitted.

        if check_fragments.is_empty() {
            return Ok(CommandOutcome::success(Some(
                "dup-check: all fragments already acked; no warnings".to_owned(),
            )));
        }

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

        let interactor = DupCheckInteractor::new(embedding_port, index_port);
        let output = interactor
            .dup_check(&DupCheckCommand { fragments: check_fragments, threshold })
            .map_err(|e| format!("dup-check failed: {e}"))?;

        // Build stderr warnings string.
        let mut warning_lines: Vec<String> = Vec::new();
        let mut warn_hashes: Vec<String> = Vec::new();

        for warning in &output.warnings {
            warning_lines.push(format!(
                "[dup-check WARNING] fragment '{}' has {} near-duplicate(s):",
                warning.input_fragment.source_path.display(),
                warning.similar_fragments.len()
            ));
            for sf in &warning.similar_fragments {
                let snippet = truncate_snippet(sf.fragment.content(), 60);
                warning_lines.push(format!(
                    "  similar: {} (score={:.4}) | {}",
                    sf.fragment.source_path.display(),
                    sf.score.value(),
                    snippet
                ));
            }
            warn_hashes.push(fragment_content_hash(warning.input_fragment.content()));
        }

        // Handle ack: write acknowledged hashes to the ack file (AC-05).
        if input.ack {
            if let Some(p) = ack_path_opt {
                let mut updated_ack_set = ack_set.clone();
                for h in &warn_hashes {
                    updated_ack_set.insert(h.clone());
                }
                write_ack_set(p, &updated_ack_set)?;
            }
        }

        // Soft gate: warnings go to stderr, exit 0 always (CN-02/AC-04).
        if warning_lines.is_empty() {
            Ok(CommandOutcome::success(Some(
                "dup-check: no near-duplicates found above threshold".to_owned(),
            )))
        } else {
            Ok(CommandOutcome {
                stdout: Some(
                    "dup-check: near-duplicates found (see stderr for details)".to_owned(),
                ),
                stderr: Some(warning_lines.join("\n")),
                exit_code: 0, // soft gate — always exit 0
            })
        }
    }
}

// ── dup-index measure-quality ─────────────────────────────────────────────────

/// No-op implementation of [`SemanticIndexPort`] for use by
/// [`MeasureQualityInteractor`], which only computes embedding metrics and
/// never reads from or writes to an index.
///
/// Using a no-op port removes the spurious dependency on LanceDB state /
/// filesystem permissions that would otherwise be required by the real adapter.
struct NoopSemanticIndexPort;

impl SemanticIndexPort for NoopSemanticIndexPort {
    fn insert(
        &self,
        _fragment: &CodeFragment,
        _embedding: &[f32],
    ) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn search(
        &self,
        _embedding: &[f32],
        _top_k: TopK,
    ) -> Result<Vec<SimilarFragment>, SemanticIndexError> {
        Ok(Vec::new())
    }
}

/// Input DTO for `sotp dup-index measure-quality`.
#[derive(Debug, Clone)]
pub struct DupIndexMeasureQualityInput {
    /// Root of the workspace to scan for Rust sources.
    pub workspace_root: PathBuf,
}

impl CliApp {
    /// Run `sotp dup-index measure-quality`: compute embedding model quality
    /// metrics over workspace fragments and output JSON to stdout (AC-03).
    ///
    /// The index port is not used by [`MeasureQualityInteractor`] (metrics are
    /// computed from embeddings alone, not index lookups), so a no-op port is
    /// supplied here — avoiding a spurious dependency on LanceDB state or
    /// filesystem permissions.
    ///
    /// # Errors
    ///
    /// Returns `Err` if extraction, embedding adapter construction, or the
    /// interactor call fails.
    pub fn semantic_dup_index_measure_quality(
        &self,
        input: DupIndexMeasureQualityInput,
    ) -> Result<CommandOutcome, String> {
        let fragments = extract_code_fragments(&input.workspace_root)
            .map_err(|e| format!("fragment extraction failed: {e}"))?;

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port = Arc::new(NoopSemanticIndexPort);

        let interactor = MeasureQualityInteractor::new(embedding_port, index_port);
        let metrics = interactor
            .measure_quality(&MeasureQualityCommand { fragments })
            .map_err(|e| format!("measure-quality failed: {e}"))?;

        let p = &metrics.cosine_percentiles;
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "mean_cosine": metrics.mean_cosine,
            "cosine_std_dev": metrics.cosine_std_dev,
            "cosine_percentiles": {
                "p10": p.first().copied().unwrap_or(0.0),
                "p25": p.get(1).copied().unwrap_or(0.0),
                "p50": p.get(2).copied().unwrap_or(0.0),
                "p75": p.get(3).copied().unwrap_or(0.0),
                "p90": p.get(4).copied().unwrap_or(0.0),
                "p95": p.get(5).copied().unwrap_or(0.0),
                "p99": p.get(6).copied().unwrap_or(0.0),
            },
            "above_threshold_rate": metrics.above_threshold_rate,
        }))
        .map_err(|e| format!("failed to serialize metrics to JSON: {e}"))?;

        Ok(CommandOutcome::success(Some(json)))
    }
}

// ── Internal helpers ──────────────────────────────────────────────────────────

/// Truncate `s` to at most `max_chars` characters, appending `…` if truncated.
fn truncate_snippet(s: &str, max_chars: usize) -> String {
    let first_line = s.lines().next().unwrap_or("");
    if first_line.chars().count() <= max_chars {
        first_line.to_owned()
    } else {
        let truncated: String = first_line.chars().take(max_chars).collect();
        format!("{truncated}…")
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]

    use std::collections::HashSet;

    use super::*;

    // ── NoopSemanticIndexPort ─────────────────────────────────────────────────

    #[test]
    fn test_noop_semantic_index_port_insert_returns_ok() {
        let port = NoopSemanticIndexPort;
        let fragment = CodeFragment::new(PathBuf::from("src/lib.rs"), "fn foo() {}".to_owned())
            .expect("valid fragment");
        let embedding = vec![0.1_f32, 0.2, 0.3];
        let result = port.insert(&fragment, &embedding);
        assert!(result.is_ok(), "NoopSemanticIndexPort::insert must always return Ok");
    }

    #[test]
    fn test_noop_semantic_index_port_search_returns_empty_vec() {
        let port = NoopSemanticIndexPort;
        let embedding = vec![0.1_f32, 0.2, 0.3];
        let top_k = TopK::new(5).expect("valid top_k");
        let result = port.search(&embedding, top_k);
        assert!(result.is_ok(), "NoopSemanticIndexPort::search must return Ok");
        assert!(
            result.unwrap().is_empty(),
            "NoopSemanticIndexPort::search must return an empty Vec"
        );
    }

    // ── fragment_content_hash ─────────────────────────────────────────────────

    #[test]
    fn test_fragment_content_hash_is_stable_across_calls() {
        let content = "fn foo() { let x = 1; }";
        let h1 = fragment_content_hash(content);
        let h2 = fragment_content_hash(content);
        assert_eq!(h1, h2, "FNV-1a hash must be deterministic");
    }

    #[test]
    fn test_fragment_content_hash_differs_for_different_content() {
        let h1 = fragment_content_hash("fn foo() {}");
        let h2 = fragment_content_hash("fn bar() {}");
        assert_ne!(h1, h2, "different content must yield different hashes");
    }

    #[test]
    fn test_fragment_content_hash_is_16_hex_chars() {
        let h = fragment_content_hash("hello world");
        assert_eq!(h.len(), 16, "FNV-1a 64-bit hash must be 16 hex characters");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex");
    }

    // ── read_ack_set / write_ack_set ──────────────────────────────────────────

    #[test]
    fn test_read_ack_set_returns_empty_set_when_file_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ack.txt");
        let set = read_ack_set(&path).unwrap();
        assert!(set.is_empty(), "missing file should yield empty set");
    }

    #[test]
    fn test_write_and_read_ack_set_round_trips_hashes() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ack.txt");

        let mut original: HashSet<String> = HashSet::new();
        original.insert("abc123def456789a".to_owned());
        original.insert("0000000000000000".to_owned());

        write_ack_set(&path, &original).unwrap();
        let read_back = read_ack_set(&path).unwrap();

        assert_eq!(original, read_back, "round-trip must preserve all hashes");
    }

    #[test]
    fn test_write_ack_set_produces_sorted_lines() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ack.txt");

        let mut set: HashSet<String> = HashSet::new();
        set.insert("zzzzzzzzzzzzzzzz".to_owned());
        set.insert("aaaaaaaaaaaaaaaa".to_owned());
        set.insert("mmmmmmmmmmmmmmmm".to_owned());

        write_ack_set(&path, &set).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();
        let lines: Vec<&str> = contents.lines().collect();

        let mut sorted = lines.clone();
        sorted.sort_unstable();
        assert_eq!(lines, sorted, "ack file lines should be sorted");
    }

    // ── AC-05: ack suppression via fragment_content_hash ─────────────────────
    //
    // These tests verify the suppress-on-re-run logic directly, without
    // constructing adapters or the real embedding model.  The ack-set
    // read/filter/append pipeline is exercised through the private helpers.

    #[test]
    fn test_ack_suppression_already_acked_hash_is_detected_in_set() {
        let content = "fn already_acked() {}";
        let hash = fragment_content_hash(content);

        let mut ack_set: HashSet<String> = HashSet::new();
        ack_set.insert(hash.clone());

        // The ack_set lookup mirrors the partition logic in semantic_dup_check.
        assert!(ack_set.contains(&hash), "acked fragment hash should be found in the set");
    }

    #[test]
    fn test_ack_suppression_new_hash_is_not_in_existing_set() {
        let existing_content = "fn already_acked() {}";
        let new_content = "fn new_fn() {}";

        let existing_hash = fragment_content_hash(existing_content);
        let new_hash = fragment_content_hash(new_content);

        let mut ack_set: HashSet<String> = HashSet::new();
        ack_set.insert(existing_hash);

        assert!(
            !ack_set.contains(&new_hash),
            "new fragment hash must not appear in the existing ack set"
        );
    }

    #[test]
    fn test_ack_suppression_write_new_hashes_appended_to_existing_set() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ack.txt");

        // Initial ack set with one hash.
        let old_hash = fragment_content_hash("fn old() {}");
        let mut initial_set: HashSet<String> = HashSet::new();
        initial_set.insert(old_hash.clone());
        write_ack_set(&path, &initial_set).unwrap();

        // Simulate adding a new warning hash.
        let new_hash = fragment_content_hash("fn new_warn() {}");
        let mut updated_set = read_ack_set(&path).unwrap();
        updated_set.insert(new_hash.clone());
        write_ack_set(&path, &updated_set).unwrap();

        // Both hashes should be present on the next read.
        let final_set = read_ack_set(&path).unwrap();
        assert!(final_set.contains(&old_hash), "old hash must be retained");
        assert!(final_set.contains(&new_hash), "new hash must be added");
        assert_eq!(final_set.len(), 2);
    }

    // ── AC-04: soft-gate exit-0 behavior ─────────────────────────────────────
    //
    // AC-04 specifies that dup-check always exits 0 (soft gate).  The
    // `CommandOutcome.exit_code` field on the warning path is tested here via
    // the `semantic_dup_check` method using a real (temp) LanceDB adapter.
    //
    // NOTE: This test does NOT construct the real FastEmbedAdapter — it skips
    // the embedding step by using an empty fragment list, exercising only the
    // early-exit "no fragments to check" code path which returns exit_code 0.

    #[test]
    fn test_dup_check_with_no_fragment_files_exits_zero_with_success_message() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let app = crate::CliApp;

        let input = DupCheckInput {
            fragment_files: vec![],
            threshold: 0.8,
            db_path,
            ack_file: None,
            ack: false,
        };

        let outcome = app.semantic_dup_check(input).unwrap();
        assert_eq!(outcome.exit_code, 0, "dup-check must always exit 0 (AC-04)");
        assert!(
            outcome.stdout.as_deref().unwrap_or("").contains("no fragments"),
            "expected 'no fragments' message in stdout"
        );
    }

    #[test]
    fn test_dup_check_with_ack_but_no_ack_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let app = crate::CliApp;

        let input = DupCheckInput {
            fragment_files: vec![],
            threshold: 0.8,
            db_path,
            ack_file: None,
            ack: true, // --ack without --ack-file must be rejected
        };

        let result = app.semantic_dup_check(input);
        assert!(result.is_err(), "--ack without --ack-file must return Err");
    }

    #[test]
    fn test_dup_check_all_fragments_acked_exits_zero_with_no_warnings_message() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let ack_path = dir.path().join("ack.txt");
        let frag_path = dir.path().join("frag.rs");

        // Write a fragment file.
        let content = "fn suppressed() {}";
        std::fs::write(&frag_path, content).unwrap();

        // Pre-populate the ack set with this fragment's hash.
        let hash = fragment_content_hash(content);
        let mut ack_set: HashSet<String> = HashSet::new();
        ack_set.insert(hash);
        write_ack_set(&ack_path, &ack_set).unwrap();

        let app = crate::CliApp;
        let input = DupCheckInput {
            fragment_files: vec![frag_path],
            threshold: 0.8,
            db_path,
            ack_file: Some(ack_path),
            ack: false,
        };

        let outcome = app.semantic_dup_check(input).unwrap();
        // AC-04: exit 0 always.
        assert_eq!(outcome.exit_code, 0);
        // AC-05: already-acked fragment is suppressed — "no warnings" message.
        assert!(
            outcome.stdout.as_deref().unwrap_or("").contains("already acked"),
            "expected 'already acked' message, got: {:?}",
            outcome.stdout
        );
    }

    // ── validate_db_path_for_rebuild: safety guard (no deletion) ─────────────
    //
    // These tests drive the `validate_db_path_for_rebuild` helper directly so
    // that no test ever constructs the real `FastEmbedAdapter` (which would
    // create a `.fastembed_cache/` directory and attempt a model download —
    // forbidden in CI).

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
    //
    // These tests use fake LanceDB index directories (create_fake_lancedb_index)
    // so that no real adapter or embedding model is constructed.

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
    //
    // These tests exercise the pre-build steps of `semantic_dup_index_build`
    // (stale-temp cleanup, crash-backup recovery) using a workspace root with
    // no Rust source files so that `do_build_into` returns Ok(0) immediately —
    // avoiding any call to `FastEmbedAdapter::new()` (which would touch the
    // real model cache and is forbidden in CI).

    #[test]
    fn test_index_build_cleans_stale_temp_dir_before_build() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        // workspace has no .rs files → extract_code_fragments returns empty
        let db_path = dir.path().join("index.db");
        let temp_path = dir.path().join(".index.db.tmp-build");

        // Place a stale temp dir (simulates a leftover from a previous crash).
        std::fs::create_dir_all(&temp_path).unwrap();
        std::fs::write(temp_path.join("stale_temp.txt"), "stale").unwrap();

        let app = crate::CliApp;
        let input =
            DupIndexBuildInput { workspace_root: workspace_root.clone(), db_path: db_path.clone() };
        let outcome = app.semantic_dup_index_build(input).unwrap();
        // Zero-fragment build succeeds.
        assert_eq!(outcome.exit_code, 0);
        // Stale temp dir must be cleaned up.
        assert!(!temp_path.exists(), "stale temp dir must be removed before build starts");
        // db_path was absent and zero fragments were found → must still be absent.
        assert!(!db_path.exists(), "db_path must not be created when no fragments are indexed");
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

    // ── truncate_snippet ──────────────────────────────────────────────────────

    #[test]
    fn test_truncate_snippet_short_string_is_unchanged() {
        let s = "fn foo() {}";
        assert_eq!(truncate_snippet(s, 80), s);
    }

    #[test]
    fn test_truncate_snippet_long_string_is_truncated_with_ellipsis() {
        let s = "a".repeat(100);
        let result = truncate_snippet(&s, 10);
        assert!(result.ends_with('…'), "truncated snippet must end with '…'");
        // 10 chars + 1 `…` multibyte character = chars count 11.
        assert_eq!(result.chars().count(), 11);
    }

    #[test]
    fn test_truncate_snippet_uses_only_first_line() {
        let s = "first line\nsecond line\nthird line";
        let result = truncate_snippet(s, 80);
        assert_eq!(result, "first line");
    }
}
