//! `dup-check` subcommand — input DTO, ack helpers, and [`crate::CliApp`] impl.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarityThreshold};
use infrastructure::semantic_dup::{
    embedding::FastEmbedAdapter, index::LanceDbSemanticIndexAdapter,
};
use usecase::semantic_dup::{DupCheckCommand, DupCheckInteractor, DupCheckService as _};

use super::SemanticDupCompositionRoot;
use crate::CommandOutcome;
use crate::error::CompositionError;

use super::common::{LANCEDB_TABLE_MARKER, is_recognizable_lancedb_index, truncate_snippet};

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
fn read_ack_set(path: &Path) -> Result<std::collections::HashSet<String>, CompositionError> {
    match std::fs::read_to_string(path) {
        Ok(contents) => Ok(contents
            .lines()
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(std::collections::HashSet::new()),
        Err(e) => Err(CompositionError::Infrastructure(format!(
            "cannot read ack file {}: {e}",
            path.display()
        ))),
    }
}

/// Write the ack-set to `path`.
fn write_ack_set(
    path: &Path,
    set: &std::collections::HashSet<String>,
) -> Result<(), CompositionError> {
    let mut sorted: Vec<&str> = set.iter().map(String::as_str).collect();
    sorted.sort_unstable();
    let contents = sorted.join("\n") + "\n";
    std::fs::write(path, contents).map_err(|e| {
        CompositionError::Infrastructure(format!("cannot write ack file {}: {e}", path.display()))
    })
}

/// Compute a stable, collision-resistant content hash for a fragment.
///
/// Returns the lowercase SHA-256 hex digest (64 characters) of the UTF-8
/// content bytes.  SHA-256 is used so that a crafted or accidental hash
/// collision cannot cause a different fragment to be silently treated as
/// already-acknowledged (AC-05 ack-suppression).
fn fragment_content_hash(content: &str) -> String {
    use sha2::{Digest as _, Sha256};
    let digest = Sha256::digest(content.as_bytes());
    format!("{digest:x}")
}

impl SemanticDupCompositionRoot {
    /// Run `sotp dup-check`: check diff fragments against the semantic index.
    ///
    /// CN-02/AC-04: always exits 0 (soft gate — warnings to stderr, no block).
    /// AC-05: fragments whose content hash appears in the ack file are suppressed.
    ///
    /// # Errors
    ///
    /// Returns `Err` on hard infrastructure failures (adapter construction or
    /// file I/O errors for the ack file), not on warnings.
    ///
    /// Also returns `Err` when `input.db_path` is absent or does not contain the
    /// `fragments.lance/` marker, i.e. is not a recognizable LanceDB index.  A
    /// missing or typo'd index silently disables the duplicate gate, so the
    /// command fails loudly instead.  Run `sotp dup-index build` first to create
    /// a valid index.
    pub fn semantic_dup_check(
        &self,
        input: DupCheckInput,
    ) -> Result<CommandOutcome, CompositionError> {
        // Reject the illegal combination: --ack requires --ack-file to be set.
        if input.ack && input.ack_file.is_none() {
            return Err(CompositionError::WiringFailed(
                "--ack requires --ack-file to be specified".to_owned(),
            ));
        }

        let threshold = SimilarityThreshold::new(input.threshold).map_err(|e| {
            CompositionError::WiringFailed(format!("invalid --threshold value: {e}"))
        })?;

        // Read all fragment files.
        let mut fragments: Vec<CodeFragment> = Vec::new();
        for path in &input.fragment_files {
            let content = std::fs::read_to_string(path).map_err(|e| {
                CompositionError::Infrastructure(format!(
                    "cannot read fragment file {}: {e}",
                    path.display()
                ))
            })?;
            // Fragments loaded from files for dup-check do not have line-span
            // information (dup-check operates on whole-file fragments from CLI
            // arguments). Use start_line=1 / end_line=u32::MAX as a sentinel
            // so these fragments always overlap any hunk if needed.
            let fragment = CodeFragment::new(path.clone(), content, 1, u32::MAX).map_err(|e| {
                CompositionError::WiringFailed(format!(
                    "invalid fragment in {}: {e}",
                    path.display()
                ))
            })?;
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

        // Validate that the semantic index exists and is recognizable BEFORE
        // constructing the embedding or index adapters.  An absent or typo'd
        // --db-path would otherwise be treated as an empty index (no
        // `fragments` table → no search results → exit 0 "no near-duplicates
        // found"), silently disabling the duplicate gate.
        if !is_recognizable_lancedb_index(&input.db_path) {
            return Err(CompositionError::WiringFailed(format!(
                "dup-check: no semantic index found at '{}' (missing '{}' marker); \
                 run `sotp dup-index build` first — refusing to run the duplicate \
                 gate against a missing index",
                input.db_path.display(),
                LANCEDB_TABLE_MARKER,
            )));
        }

        let embedding_port = Arc::new(FastEmbedAdapter::new().map_err(|e| {
            CompositionError::AdapterInit(format!("failed to load embedding model: {e}"))
        })?);
        let index_port =
            Arc::new(LanceDbSemanticIndexAdapter::new(input.db_path.clone()).map_err(|e| {
                CompositionError::AdapterInit(format!(
                    "failed to open index at {}: {e}",
                    input.db_path.display()
                ))
            })?);

        let interactor = DupCheckInteractor::new(embedding_port, index_port);
        let output = interactor
            .dup_check(&DupCheckCommand { fragments: check_fragments, threshold })
            .map_err(|e| CompositionError::Usecase(format!("dup-check failed: {e}")))?;

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
        assert_eq!(h1, h2, "SHA-256 hash must be deterministic");
    }

    #[test]
    fn test_fragment_content_hash_differs_for_different_content() {
        let h1 = fragment_content_hash("fn foo() {}");
        let h2 = fragment_content_hash("fn bar() {}");
        assert_ne!(h1, h2, "different content must yield different SHA-256 hashes");
    }

    #[test]
    fn test_fragment_content_hash_is_64_hex_chars() {
        let h = fragment_content_hash("hello world");
        assert_eq!(h.len(), 64, "SHA-256 hash must be 64 lowercase hex characters");
        assert!(h.chars().all(|c| c.is_ascii_hexdigit()), "hash must be hex");
        assert!(h.chars().all(|c| !c.is_uppercase()), "hash must be lowercase");
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

    #[test]
    fn test_dup_check_with_no_fragment_files_exits_zero_with_success_message() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("test.db");
        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();

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
        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();

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

        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();
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

    // ── Missing-index guard (P1 fail-open fix) ────────────────────────────────

    /// Create a minimal fragment file in `dir` and return its path.
    fn write_fragment_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
        let path = dir.join(name);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_dup_check_with_fragment_and_nonexistent_db_path_returns_err_missing_index() {
        // P1 fix: a fragment that reaches the gate but whose db_path does not
        // exist at all must produce Err, not a silent "no near-duplicates found".
        let dir = tempfile::tempdir().unwrap();
        let frag_path = write_fragment_file(dir.path(), "frag.rs", "fn guard_test() {}");
        // db_path points to a path that does not exist.
        let db_path = dir.path().join("nonexistent_index.db");
        assert!(!db_path.exists(), "pre-condition: db_path must not exist");

        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();
        let input = DupCheckInput {
            fragment_files: vec![frag_path],
            threshold: 0.8,
            db_path: db_path.clone(),
            ack_file: None,
            ack: false,
        };

        let result = app.semantic_dup_check(input);
        assert!(
            result.is_err(),
            "dup-check must return Err when db_path does not exist, got: {:?}",
            result.ok()
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("dup-index build"),
            "error message must reference 'dup-index build', got: {msg}"
        );
        assert!(msg.contains("missing"), "error message must mention 'missing', got: {msg}");
    }

    #[test]
    fn test_dup_check_with_fragment_and_dir_without_marker_returns_err_missing_index() {
        // P1 fix: a db_path that exists as a directory but does NOT contain the
        // `fragments.lance/` marker must produce Err, not a silent pass.
        let dir = tempfile::tempdir().unwrap();
        let frag_path = write_fragment_file(dir.path(), "frag.rs", "fn guard_test_dir() {}");
        // Create a directory at db_path WITHOUT the LanceDB marker.
        let db_path = dir.path().join("not_an_index");
        std::fs::create_dir_all(&db_path).unwrap();
        std::fs::write(db_path.join("some_other_file.txt"), "unrelated").unwrap();
        assert!(db_path.exists(), "pre-condition: db_path dir must exist");
        assert!(
            !db_path.join(super::super::common::LANCEDB_TABLE_MARKER).exists(),
            "pre-condition: marker must be absent"
        );

        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();
        let input = DupCheckInput {
            fragment_files: vec![frag_path],
            threshold: 0.8,
            db_path: db_path.clone(),
            ack_file: None,
            ack: false,
        };

        let result = app.semantic_dup_check(input);
        assert!(
            result.is_err(),
            "dup-check must return Err when db_path exists but has no LanceDB marker, got: {:?}",
            result.ok()
        );
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("dup-index build"),
            "error message must reference 'dup-index build', got: {msg}"
        );
        // The directory must be completely untouched.
        assert!(db_path.exists(), "db_path directory must not be deleted on Err");
        assert!(
            db_path.join("some_other_file.txt").exists(),
            "unrelated file inside db_path must be preserved"
        );
    }

    #[test]
    fn test_dup_check_with_no_fragments_returns_ok_regardless_of_db_path() {
        // Regression: the missing-index guard must NOT fire when there are zero
        // fragments to check — that early-return path exits before the guard.
        let dir = tempfile::tempdir().unwrap();
        // db_path does not exist — would trigger the guard if fragments were present.
        let db_path = dir.path().join("nonexistent_index.db");
        assert!(!db_path.exists(), "pre-condition: db_path must not exist");

        let app = crate::semantic_dup::SemanticDupCompositionRoot::new();
        let input = DupCheckInput {
            fragment_files: vec![], // zero fragments → early return before guard
            threshold: 0.8,
            db_path,
            ack_file: None,
            ack: false,
        };

        let outcome = app.semantic_dup_check(input).unwrap();
        assert_eq!(outcome.exit_code, 0, "zero-fragment check must exit 0 (AC-04)");
        assert!(
            outcome.stdout.as_deref().unwrap_or("").contains("no fragments"),
            "expected 'no fragments' message, got: {:?}",
            outcome.stdout
        );
    }
}
