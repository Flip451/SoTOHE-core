//! File-level content-hash manifest for the persistent semantic index cache.
//!
//! Relocated from `apps/cli-composition/src/dry/manifest.rs` (T028) — the
//! logic is fully owned by [`super::DryCheckServiceFactoryAdapter`] now, so it
//! no longer needs to live in `cli_composition`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::semantic_dup::CodeFragment;

use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

/// The stable embedding model identity string for manifest keying.
///
/// This must change whenever the embedding model changes so that a model
/// upgrade marks all files as dirty and triggers a full rebuild.
/// Matches `JINA_V2_CODE_MODEL_CODE` in `crate::semantic_dup::embedding`.
pub(super) const EMBEDDING_MODEL_ID: &str = "jinaai/jina-embeddings-v2-base-code";
pub(super) const SEMANTIC_INDEX_CACHE_MARKER_SUFFIX: &str = ".sotp-cache";

/// File-level content-hash manifest persisted alongside the semantic index.
///
/// The `embedding_model_id` is stored so that a model upgrade marks all files
/// dirty and forces a full rebuild.
///
/// Serialized as JSON: `{"embedding_model_id":"...","files":{"path":"sha256hex",...}}`
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub(super) struct IndexManifest {
    /// Embedding model identity (must match [`EMBEDDING_MODEL_ID`]).
    pub(super) embedding_model_id: String,
    /// Map from repo-relative source-file path (string) to its SHA-256 hex
    /// content hash as of the last successful index update.
    pub(super) files: HashMap<String, String>,
}

impl IndexManifest {
    /// Create an empty manifest for the given model.
    pub(super) fn empty(embedding_model_id: impl Into<String>) -> Self {
        Self { embedding_model_id: embedding_model_id.into(), files: Default::default() }
    }
}

/// Append `suffix` to the OS-string representation of `db_path` and return the
/// resulting `PathBuf`.
///
/// This is the single canonical implementation for all sidecar-path helpers
/// that live alongside the semantic index directory:
/// - `{db_path}.manifest`  - see [`manifest_sidecar_path`]
/// - `{db_path}.lock`      - see `crate::semantic_dup::null_insert_proxy::persistent_index_lock_path`
/// - `{db_path}.sotp-cache` - see [`super::persistent_index::persistent_index_marker_path`]
pub(super) fn persistent_index_suffixed_path(
    db_path: &Path,
    suffix: impl AsRef<std::ffi::OsStr>,
) -> PathBuf {
    let mut p = db_path.as_os_str().to_os_string();
    p.push(suffix);
    PathBuf::from(p)
}

/// Return the manifest sidecar path for a given `db_path`.
///
/// Stored at `{db_path}.manifest` - next to the DB directory, outside it.
pub(super) fn manifest_sidecar_path(db_path: &Path) -> PathBuf {
    persistent_index_suffixed_path(db_path, ".manifest")
}

/// Read the persisted manifest from the sidecar file.
///
/// Returns `Ok(Some(manifest))` when the file exists and is valid JSON.
/// Returns `Ok(None)` when the file is absent.
/// Returns `Err` on I/O errors other than `NotFound` or JSON parse failure.
pub(super) fn read_manifest(sidecar: &Path) -> Result<Option<IndexManifest>, String> {
    reject_manifest_path_symlinks(sidecar, "manifest")?;
    match std::fs::read_to_string(sidecar) {
        Ok(s) => {
            let m: IndexManifest = serde_json::from_str(&s)
                .map_err(|e| format!("failed to parse manifest {}: {e}", sidecar.display()))?;
            Ok(Some(m))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("failed to read manifest {}: {e}", sidecar.display())),
    }
}

fn manifest_symlink_guard_trusted_root(path: &Path) -> &Path {
    if path.is_absolute() {
        path.ancestors().last().unwrap_or_else(|| Path::new("/"))
    } else {
        Path::new("")
    }
}

fn reject_manifest_path_symlinks(path: &Path, label: &str) -> Result<bool, String> {
    reject_symlinks_below(path, manifest_symlink_guard_trusted_root(path))
        .map_err(|e| format!("symlink guard {label} {}: {e}", path.display()))
}

/// Write the manifest to the sidecar file atomically (temp -> rename).
pub(super) fn write_manifest(sidecar: &Path, manifest: &IndexManifest) -> Result<(), String> {
    if let Some(parent) = sidecar.parent() {
        if !parent.as_os_str().is_empty() {
            reject_manifest_path_symlinks(parent, "manifest parent")?;
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create manifest parent dir: {e}"))?;
        }
    }
    let value =
        serde_json::to_value(manifest).map_err(|e| format!("failed to serialize manifest: {e}"))?;
    let json =
        serde_json::to_string(&value).map_err(|e| format!("failed to serialize manifest: {e}"))?;
    reject_manifest_path_symlinks(sidecar, "manifest")?;
    atomic_write_file(sidecar, json.as_bytes())
        .map_err(|e| format!("failed to write manifest {}: {e}", sidecar.display()))
}

/// Remove the manifest sidecar if it exists (idempotent).
pub(super) fn remove_manifest(sidecar: &Path) -> Result<(), String> {
    match std::fs::remove_file(sidecar) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("failed to remove manifest {}: {e}", sidecar.display())),
    }
}

/// Compute the file-level content hash for a set of corpus fragments sharing
/// the same `source_path`.
///
/// The hash covers every field persisted in the semantic-index payload for that
/// file, sorted by (start_line, end_line, content) to be stable across fragment
/// ordering.
pub(super) fn file_content_hash(fragments_for_file: &[&CodeFragment]) -> String {
    use sha2::Digest as _;
    let mut sorted: Vec<(u32, u32, &str)> =
        fragments_for_file.iter().map(|f| (f.start_line(), f.end_line(), f.content())).collect();
    sorted.sort_unstable_by(|a, b| {
        a.0.cmp(&b.0).then_with(|| a.1.cmp(&b.1)).then_with(|| a.2.cmp(b.2))
    });
    let mut hasher = sha2::Sha256::new();
    for (start_line, end_line, content) in &sorted {
        hasher.update(start_line.to_le_bytes());
        hasher.update(b"\x00");
        hasher.update(end_line.to_le_bytes());
        hasher.update(b"\x00");
        hasher.update(content.as_bytes());
        hasher.update(b"\x00");
    }
    format!("{:x}", hasher.finalize())
}

/// Return the manifest key used for a fragment's source file path.
pub(super) fn manifest_source_path_key(fragment: &CodeFragment) -> String {
    fragment.source_path.to_string_lossy().replace('\\', "/")
}

/// Outcome of comparing the current working-tree corpus to the stored manifest.
pub(super) struct ManifestDiff {
    /// Files with changed or new content (need delete-then-reinsert).
    pub(super) dirty: Vec<String>,
    /// Files that existed in the manifest but are absent from the working tree.
    pub(super) deleted: Vec<String>,
    /// Files with identical content (nothing to do).
    ///
    /// Not consumed by production code paths; stored for observability and tests.
    #[allow(dead_code)]
    pub(super) unchanged: Vec<String>,
}

/// Compute the diff between the current corpus fragments and the stored manifest.
///
/// A `None` manifest means "no prior state" - all files are dirty.
/// Model mismatch also marks all files dirty (full rebuild path).
pub(super) fn compute_manifest_diff(
    corpus_fragments: &[CodeFragment],
    manifest: Option<&IndexManifest>,
    embedding_model_id: &str,
) -> ManifestDiff {
    let mut by_path: HashMap<String, Vec<&CodeFragment>> = Default::default();
    for frag in corpus_fragments {
        by_path.entry(manifest_source_path_key(frag)).or_default().push(frag);
    }

    let model_matches =
        manifest.map(|m| m.embedding_model_id == embedding_model_id).unwrap_or(false);

    let stored = match manifest {
        Some(m) if model_matches => m,
        _ => {
            let dirty = by_path.into_keys().collect();
            return ManifestDiff { dirty, deleted: Vec::new(), unchanged: Vec::new() };
        }
    };

    let mut dirty = Vec::new();
    let mut unchanged = Vec::new();

    for (path, frags) in &by_path {
        let current_hash = file_content_hash(frags);
        match stored.files.get(path) {
            Some(stored_hash) if stored_hash == &current_hash => unchanged.push(path.clone()),
            _ => dirty.push(path.clone()),
        }
    }

    let deleted: Vec<String> =
        stored.files.keys().filter(|p| !by_path.contains_key(*p)).cloned().collect();

    ManifestDiff { dirty, deleted, unchanged }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn frag(path: &str, content: &str, start: u32) -> CodeFragment {
        frag_span(path, content, start, start)
    }

    fn frag_span(path: &str, content: &str, start: u32, end: u32) -> CodeFragment {
        CodeFragment::new(PathBuf::from(path), content.to_owned(), start, end).unwrap()
    }

    #[test]
    fn test_manifest_sidecar_path_appends_manifest_suffix() {
        let db_path = PathBuf::from("/tmp/db");
        assert_eq!(manifest_sidecar_path(&db_path), PathBuf::from("/tmp/db.manifest"));
    }

    #[test]
    fn test_read_manifest_absent_sidecar_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        let result = read_manifest(&dir.path().join("nonexistent.manifest")).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_write_and_read_manifest_round_trips() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("db.manifest");
        let mut manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);
        manifest.files.insert("src/a.rs".to_owned(), "hash1".to_owned());

        write_manifest(&sidecar, &manifest).unwrap();
        let read_back = read_manifest(&sidecar).unwrap().unwrap();

        assert_eq!(read_back.embedding_model_id, EMBEDDING_MODEL_ID);
        assert_eq!(read_back.files.get("src/a.rs"), Some(&"hash1".to_owned()));
    }

    #[test]
    fn test_write_manifest_canonicalizes_keys_and_is_byte_stable() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("db.manifest");
        let mut manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);
        manifest.files.insert("src/z.rs".to_owned(), "hash-z".to_owned());
        manifest.files.insert("src/a.rs".to_owned(), "hash-a".to_owned());

        write_manifest(&sidecar, &manifest).unwrap();
        let first = std::fs::read_to_string(&sidecar).unwrap();
        write_manifest(&sidecar, &manifest).unwrap();
        let second = std::fs::read_to_string(&sidecar).unwrap();

        assert_eq!(first, second, "manifest encoding must not churn JSON bytes");
        assert!(
            first.starts_with("{\"embedding_model_id\":"),
            "manifest top-level keys must be canonicalized: {first}"
        );
        assert!(
            first.find("src/a.rs").unwrap() < first.find("src/z.rs").unwrap(),
            "manifest file keys must be recursively canonicalized: {first}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_write_manifest_symlinked_sidecar_returns_error_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_target = outside.path().join("target.manifest");
        std::fs::write(&outside_target, "do not overwrite").unwrap();
        let sidecar = dir.path().join("db.manifest");
        std::os::unix::fs::symlink(&outside_target, &sidecar).unwrap();
        let manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);

        let err = write_manifest(&sidecar, &manifest).unwrap_err();

        assert!(err.contains("symlink guard manifest"), "got: {err}");
        assert_eq!(std::fs::read_to_string(&outside_target).unwrap(), "do not overwrite");
    }

    #[cfg(unix)]
    #[test]
    fn test_write_manifest_symlinked_atomic_temp_returns_error_without_overwrite() {
        let dir = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let outside_target = outside.path().join("target.manifest");
        std::fs::write(&outside_target, "do not overwrite").unwrap();
        let sidecar = dir.path().join("db.manifest");
        let atomic_tmp = dir.path().join(format!(".tmp-db.manifest-{}", std::process::id()));
        std::os::unix::fs::symlink(&outside_target, &atomic_tmp).unwrap();
        let manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);

        let err = write_manifest(&sidecar, &manifest).unwrap_err();

        assert!(err.contains("failed to write manifest"), "got: {err}");
        assert_eq!(std::fs::read_to_string(&outside_target).unwrap(), "do not overwrite");
    }

    #[cfg(unix)]
    #[test]
    fn test_read_manifest_symlinked_sidecar_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let real_sidecar = dir.path().join("real.manifest");
        let link_sidecar = dir.path().join("db.manifest");
        let mut manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);
        manifest.files.insert("src/a.rs".to_owned(), "hash1".to_owned());
        std::fs::write(&real_sidecar, serde_json::to_string(&manifest).unwrap()).unwrap();
        std::os::unix::fs::symlink(&real_sidecar, &link_sidecar).unwrap();

        let err = read_manifest(&link_sidecar).unwrap_err();

        assert!(
            err.contains("symlink guard"),
            "symlinked manifest sidecar must fail closed, got: {err}"
        );
    }

    #[test]
    fn test_remove_manifest_absent_is_ok() {
        let dir = tempfile::tempdir().unwrap();
        assert!(remove_manifest(&dir.path().join("nonexistent.manifest")).is_ok());
    }

    #[test]
    fn test_remove_manifest_removes_existing_sidecar() {
        let dir = tempfile::tempdir().unwrap();
        let sidecar = dir.path().join("db.manifest");
        std::fs::write(&sidecar, "{}").unwrap();
        remove_manifest(&sidecar).unwrap();
        assert!(!sidecar.exists());
    }

    #[test]
    fn test_file_content_hash_is_order_independent() {
        let a = frag("src/a.rs", "fn a() {}", 1);
        let b = frag("src/a.rs", "fn b() {}", 5);
        let hash1 = file_content_hash(&[&a, &b]);
        let hash2 = file_content_hash(&[&b, &a]);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_file_content_hash_differs_for_different_content() {
        let a = frag("src/a.rs", "fn a() {}", 1);
        let b = frag("src/a.rs", "fn b() {}", 1);
        assert_ne!(file_content_hash(&[&a]), file_content_hash(&[&b]));
    }

    #[test]
    fn test_file_content_hash_differs_for_different_end_line() {
        let a = frag_span("src/a.rs", "fn a() {\n  work();\n}", 1, 3);
        let b = frag_span("src/a.rs", "fn a() {\n  work();\n}", 1, 4);
        assert_ne!(file_content_hash(&[&a]), file_content_hash(&[&b]));
    }

    #[test]
    fn test_compute_manifest_diff_no_manifest_all_dirty() {
        let fragments = vec![frag("src/a.rs", "fn a() {}", 1)];
        let diff = compute_manifest_diff(&fragments, None, EMBEDDING_MODEL_ID);
        assert_eq!(diff.dirty, vec!["src/a.rs".to_owned()]);
        assert!(diff.deleted.is_empty());
    }

    #[test]
    fn test_compute_manifest_diff_model_mismatch_all_dirty() {
        let fragments = vec![frag("src/a.rs", "fn a() {}", 1)];
        let mut manifest = IndexManifest::empty("old-model");
        manifest.files.insert("src/a.rs".to_owned(), file_content_hash(&[&fragments[0]]));
        let diff = compute_manifest_diff(&fragments, Some(&manifest), EMBEDDING_MODEL_ID);
        assert_eq!(diff.dirty, vec!["src/a.rs".to_owned()]);
    }

    #[test]
    fn test_compute_manifest_diff_dirty_and_deleted() {
        let fragments = vec![frag("src/a.rs", "fn a_changed() {}", 1)];
        let mut manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);
        manifest.files.insert("src/a.rs".to_owned(), "stale-hash".to_owned());
        manifest.files.insert("src/removed.rs".to_owned(), "some-hash".to_owned());

        let diff = compute_manifest_diff(&fragments, Some(&manifest), EMBEDDING_MODEL_ID);

        assert_eq!(diff.dirty, vec!["src/a.rs".to_owned()]);
        assert_eq!(diff.deleted, vec!["src/removed.rs".to_owned()]);
    }
}
