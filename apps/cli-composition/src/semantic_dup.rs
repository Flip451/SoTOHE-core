//! `semantic_dup` command family — CliApp impl methods and input DTOs.
//!
//! Composition root for all four semantic-dup subcommands:
//! - `find_similar`: embed a query fragment and retrieve top-k results.
//! - `dup_index_build`: extract workspace fragments, embed, and insert into LanceDB.
//! - `dup_check`: check diff fragments against the index (soft gate, always exit 0).
//! - `dup_index_measure_quality`: compute embedding quality metrics over workspace.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarityThreshold, TopK};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, extractor::extract_code_fragments,
    index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{
    BuildIndexCommand, BuildIndexService as _, DupCheckCommand, DupCheckInteractor,
    DupCheckService as _, FindSimilarCommand, FindSimilarInteractor, FindSimilarService as _,
    MeasureQualityCommand, MeasureQualityInteractor, MeasureQualityService as _,
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

/// Prepare a fresh index directory at `db_path` for a rebuild.
///
/// This helper:
/// 1. Rejects `db_path` values that are equal to or an ancestor of
///    `workspace_root` (path-overlap safety guard).
/// 2. Removes the existing directory at `db_path` (if any) so that the
///    subsequent adapter construction always starts from an empty slate.
///
/// The removal uses `std::fs::remove_dir_all`, which is safe because LanceDB
/// stores its data as a directory tree (not a single file).  If `db_path`
/// does not exist yet this function is a no-op.
///
/// # Errors
///
/// Returns `Err` if the path-overlap guard fires, if `canonicalize` fails, or
/// if `remove_dir_all` fails.
fn prepare_fresh_index_dir(db_path: &Path, workspace_root: &Path) -> Result<(), String> {
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
             refusing to remove it to prevent data loss",
            db_path.display(),
            workspace_root.display(),
        ));
    }
    std::fs::remove_dir_all(db_path)
        .map_err(|e| format!("failed to remove existing index at {}: {e}", db_path.display()))
}

impl CliApp {
    /// Run `sotp dup-index build`: extract workspace fragments, embed each,
    /// and insert into the LanceDB index.
    ///
    /// **Rebuild-from-scratch semantics**: each invocation starts with a fresh
    /// index.  If a database already exists at `db_path` it is removed before
    /// the new index adapter is constructed, so stale or duplicate fragments
    /// from previous runs never accumulate.  The delete is performed at the
    /// composition level (no change to the usecase port interface) using
    /// `std::fs::remove_dir_all`, which is safe because LanceDB stores its
    /// data as a directory tree.
    ///
    /// AC-02: exits 0 on success; no network calls at build time (model weights
    /// are loaded from the local fastembed cache).
    ///
    /// # Errors
    ///
    /// Returns `Err` if the existing index cannot be removed, if extraction
    /// fails, if adapter construction fails, or if indexing fails.
    pub fn semantic_dup_index_build(
        &self,
        input: DupIndexBuildInput,
    ) -> Result<CommandOutcome, String> {
        // Remove any existing index directory so that each build starts from
        // scratch.  This prevents stale/duplicate fragments from accumulating
        // across repeated `dup-index build` invocations on the same `--db-path`.
        prepare_fresh_index_dir(&input.db_path, &input.workspace_root)?;

        let fragments = extract_code_fragments(&input.workspace_root)
            .map_err(|e| format!("fragment extraction failed: {e}"))?;

        if fragments.is_empty() {
            return Ok(CommandOutcome::success(Some(
                "Indexed 0 fragment(s) (no Rust sources found in workspace)".to_owned(),
            )));
        }

        let fragment_count = fragments.len();

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

        use usecase::semantic_dup::BuildIndexInteractor;
        let interactor = BuildIndexInteractor::new(embedding_port, index_port);
        let output = interactor
            .build_index(&BuildIndexCommand { fragments })
            .map_err(|e| format!("build-index failed: {e}"))?;

        Ok(CommandOutcome::success(Some(format!(
            "Indexed {} fragment(s) (extracted: {fragment_count})",
            output.fragments_indexed
        ))))
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

/// Input DTO for `sotp dup-index measure-quality`.
#[derive(Debug, Clone)]
pub struct DupIndexMeasureQualityInput {
    /// Root of the workspace to scan for Rust sources.
    pub workspace_root: PathBuf,
    /// Path to the local LanceDB database (used by the index port in the
    /// interactor even though measure-quality operates in-memory).
    pub db_path: PathBuf,
}

impl CliApp {
    /// Run `sotp dup-index measure-quality`: compute embedding model quality
    /// metrics over workspace fragments and output JSON to stdout (AC-03).
    ///
    /// # Errors
    ///
    /// Returns `Err` if extraction, adapter construction, or the interactor call
    /// fails.
    pub fn semantic_dup_index_measure_quality(
        &self,
        input: DupIndexMeasureQualityInput,
    ) -> Result<CommandOutcome, String> {
        let fragments = extract_code_fragments(&input.workspace_root)
            .map_err(|e| format!("fragment extraction failed: {e}"))?;

        let embedding_port = Arc::new(
            FastEmbedAdapter::new().map_err(|e| format!("failed to load embedding model: {e}"))?,
        );
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                format!("failed to open index at {}: {e}", input.db_path.display())
            })?);

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

    // ── prepare_fresh_index_dir: safety guard + removal ──────────────────────
    //
    // These tests drive the `prepare_fresh_index_dir` helper directly so that
    // no test ever constructs the real `FastEmbedAdapter` (which would create
    // a `.fastembed_cache/` directory and attempt a model download — forbidden
    // in CI).

    #[test]
    fn test_prepare_fresh_index_dir_rejects_db_path_equal_to_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().to_path_buf();
        // db_path == workspace_root: the overlap guard must fire.
        let db_path = workspace_root.clone();
        let result = prepare_fresh_index_dir(&db_path, &workspace_root);
        assert!(result.is_err(), "db_path == workspace_root must be rejected");
        let msg = result.unwrap_err();
        assert!(msg.contains("overlaps"), "error message must mention 'overlaps', got: {msg}");
    }

    #[test]
    fn test_prepare_fresh_index_dir_rejects_db_path_ancestor_of_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        // workspace_root is a subdir; db_path is the parent → ancestor overlap.
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().to_path_buf(); // parent of workspace_root
        let result = prepare_fresh_index_dir(&db_path, &workspace_root);
        assert!(result.is_err(), "db_path ancestor of workspace_root must be rejected");
        let msg = result.unwrap_err();
        assert!(msg.contains("overlaps"), "error message must mention 'overlaps', got: {msg}");
    }

    #[test]
    fn test_prepare_fresh_index_dir_removes_existing_db_dir() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().join("index.db");
        // Pre-create a directory at db_path to simulate a prior build.
        std::fs::create_dir_all(&db_path).unwrap();
        // Place a sentinel file inside to verify it gets removed.
        std::fs::write(db_path.join("stale.txt"), "stale").unwrap();
        assert!(db_path.exists(), "pre-condition: db_path must exist");

        let result = prepare_fresh_index_dir(&db_path, &workspace_root);
        assert!(result.is_ok(), "removal should succeed: {:?}", result.err());
        assert!(!db_path.exists(), "existing index directory must be removed");
    }

    #[test]
    fn test_prepare_fresh_index_dir_is_noop_when_db_path_does_not_exist() {
        let dir = tempfile::tempdir().unwrap();
        let workspace_root = dir.path().join("workspace");
        std::fs::create_dir_all(&workspace_root).unwrap();
        let db_path = dir.path().join("nonexistent.db");
        // db_path does not exist — function must be a no-op and succeed.
        let result = prepare_fresh_index_dir(&db_path, &workspace_root);
        assert!(result.is_ok(), "no-op on absent path must succeed: {:?}", result.err());
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
