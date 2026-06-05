use std::collections::HashMap;
use std::path::{Path, PathBuf};

use domain::semantic_dup::CodeFragment;

/// The stable embedding model identity string for manifest keying.
///
/// This must change whenever the embedding model changes so that a model
/// upgrade marks all files as dirty and triggers a full rebuild.
/// Matches `JINA_V2_CODE_MODEL_CODE` in `infrastructure::semantic_dup::embedding`.
pub(super) const EMBEDDING_MODEL_ID: &str = "jinaai/jina-embeddings-v2-base-code";
pub(super) const SEMANTIC_INDEX_CACHE_MARKER_SUFFIX: &str = ".sotp-cache";

/// File-level content-hash manifest persisted alongside the semantic index.
///
/// Replaces the single-fingerprint sidecar of D6 with a per-file map.
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
/// This is the single canonical implementation for all sidecar-path helpers that
/// live alongside the semantic index directory:
/// - `{db_path}.manifest`  - see [`manifest_sidecar_path`]
/// - `{db_path}.lock`      - see `persistent_index_lock_path`
/// - `{db_path}.sotp-cache` - see `persistent_index_marker_path`
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

/// Write the manifest to the sidecar file atomically (temp -> rename).
pub(super) fn write_manifest(sidecar: &Path, manifest: &IndexManifest) -> Result<(), String> {
    if let Some(parent) = sidecar.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create manifest parent dir: {e}"))?;
        }
    }
    let json = serde_json::to_string(manifest)
        .map_err(|e| format!("failed to serialize manifest: {e}"))?;
    let mut tmp_path = sidecar.as_os_str().to_os_string();
    tmp_path.push(".tmp");
    let tmp_path = PathBuf::from(tmp_path);
    std::fs::write(&tmp_path, &json)
        .map_err(|e| format!("failed to write temp manifest {}: {e}", tmp_path.display()))?;
    std::fs::rename(&tmp_path, sidecar)
        .map_err(|e| format!("failed to rename manifest to {}: {e}", sidecar.display()))
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
/// The hash covers the concatenation of all fragment contents for that file,
/// sorted by (start_line, content) to be stable across fragment ordering.
pub(super) fn file_content_hash(fragments_for_file: &[&CodeFragment]) -> String {
    use sha2::Digest as _;
    let mut sorted: Vec<(u32, &str)> =
        fragments_for_file.iter().map(|f| (f.start_line(), f.content())).collect();
    sorted.sort_unstable_by(|a, b| a.0.cmp(&b.0).then_with(|| a.1.cmp(b.1)));
    let mut hasher = sha2::Sha256::new();
    for (line, content) in &sorted {
        hasher.update(line.to_le_bytes());
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
