use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use domain::semantic_dup::{CodeFragment, SimilarFragment, TopK};
use infrastructure::semantic_dup::index::LanceDbSemanticIndexAdapter;
use usecase::semantic_dup::{EmbeddingPort, SemanticIndexError, SemanticIndexPort};

use super::manifest::{
    EMBEDDING_MODEL_ID, IndexManifest, SEMANTIC_INDEX_CACHE_MARKER_SUFFIX, compute_manifest_diff,
    file_content_hash, manifest_sidecar_path, manifest_source_path_key,
    persistent_index_suffixed_path, read_manifest, remove_manifest, write_manifest,
};

const LANCEDB_TABLE_MARKER: &str = "fragments.lance";

fn remove_persistent_index_marker(db_path: &Path) -> Result<(), String> {
    let marker = persistent_index_marker_path(db_path);
    match std::fs::remove_file(&marker) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(format!("failed to remove index cache marker {}: {e}", marker.display())),
    }
}

pub(super) fn persistent_index_lock_path(db_path: &Path) -> std::path::PathBuf {
    persistent_index_suffixed_path(db_path, ".lock")
}

pub(super) fn persistent_index_marker_path(db_path: &Path) -> std::path::PathBuf {
    persistent_index_suffixed_path(db_path, SEMANTIC_INDEX_CACHE_MARKER_SUFFIX)
}

pub(super) fn write_persistent_index_marker(db_path: &Path) -> Result<(), String> {
    let created_db_path = match std::fs::symlink_metadata(db_path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!("index cache dir {} is a symlink", db_path.display()));
        }
        Ok(metadata) if metadata.is_dir() => false,
        Ok(_) => {
            return Err(format!("index cache path {} is not a directory", db_path.display()));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => true,
        Err(e) => {
            return Err(format!("failed to inspect index cache dir {}: {e}", db_path.display()));
        }
    };

    if let Some(parent) = db_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create index cache parent dir: {e}"))?;
        }
    }
    std::fs::create_dir_all(db_path)
        .map_err(|e| format!("failed to create index cache dir {}: {e}", db_path.display()))?;

    let marker = persistent_index_marker_path(db_path);
    match std::fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(cleanup_failed_marker_write(
                db_path,
                &marker,
                created_db_path,
                format!("index cache marker {} is a symlink", marker.display()),
            ));
        }
        Ok(_) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => {
            return Err(cleanup_failed_marker_write(
                db_path,
                &marker,
                created_db_path,
                format!("failed to inspect index cache marker {}: {e}", marker.display()),
            ));
        }
    }

    let canonical_db_path = db_path.canonicalize().map_err(|e| {
        format!("failed to canonicalize index cache dir {}: {e}", db_path.display())
    })?;
    std::fs::write(
        &marker,
        format!("sotp semantic index cache\npath={}\n", canonical_db_path.display()),
    )
    .map_err(|e| {
        cleanup_failed_marker_write(
            db_path,
            &marker,
            created_db_path,
            format!("failed to write index cache marker {}: {e}", marker.display()),
        )
    })
}

fn cleanup_failed_marker_write(
    db_path: &Path,
    marker: &Path,
    remove_db_path: bool,
    error_message: String,
) -> String {
    let marker_cleanup = match std::fs::remove_file(marker) {
        Ok(()) => None,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => Some(format!("marker cleanup failed: {e}")),
    };
    let db_cleanup = if remove_db_path {
        match std::fs::remove_dir_all(db_path) {
            Ok(()) => None,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => Some(format!("db cleanup failed: {e}")),
        }
    } else {
        None
    };

    match (marker_cleanup, db_cleanup) {
        (None, None) => error_message,
        (marker_cleanup, db_cleanup) => format!(
            "{error_message}; additionally failed to clean incomplete index cache at {} ({}, {})",
            db_path.display(),
            marker_cleanup.unwrap_or_else(|| "marker cleanup ok".to_owned()),
            db_cleanup.unwrap_or_else(|| "db cleanup ok".to_owned())
        ),
    }
}

fn persistent_index_marker_matches(db_path: &Path) -> Result<bool, String> {
    let marker = persistent_index_marker_path(db_path);
    match std::fs::symlink_metadata(&marker) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            return Err(format!("index cache marker {} is a symlink", marker.display()));
        }
        Ok(metadata) if metadata.is_file() => {}
        Ok(_) => {
            return Err(format!("index cache marker {} is not a regular file", marker.display()));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(format!("failed to inspect index cache marker {}: {e}", marker.display()));
        }
    }
    let marker_content = match std::fs::read_to_string(&marker) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(e) => {
            return Err(format!("failed to read index cache marker {}: {e}", marker.display()));
        }
    };
    let canonical_db_path = db_path.canonicalize().map_err(|e| {
        format!("failed to canonicalize index cache dir {}: {e}", db_path.display())
    })?;
    let expected_path_line = format!("path={}", canonical_db_path.display());
    Ok(marker_content.lines().any(|line| line == expected_path_line))
}

#[derive(Clone, Copy)]
enum ExistingDirectoryKind {
    PersistentIndexPath,
    TableMarker,
}

impl ExistingDirectoryKind {
    fn symlink_error(self, path: &Path) -> String {
        match self {
            Self::PersistentIndexPath => format!(
                "refusing to use semantic index path {} because it is a symlink",
                path.display()
            ),
            Self::TableMarker => {
                format!("semantic index table marker {} is a symlink", path.display())
            }
        }
    }

    fn not_directory_error(self, path: &Path) -> String {
        match self {
            Self::PersistentIndexPath => format!(
                "refusing to use semantic index path {} because it is not a directory",
                path.display()
            ),
            Self::TableMarker => {
                format!("semantic index table marker {} is not a directory", path.display())
            }
        }
    }

    fn inspect_error(self, path: &Path, error: &std::io::Error) -> String {
        match self {
            Self::PersistentIndexPath => {
                format!("failed to inspect semantic index path {}: {error}", path.display())
            }
            Self::TableMarker => {
                format!("failed to inspect semantic index table marker {}: {error}", path.display())
            }
        }
    }
}

fn existing_directory_state(path: &Path, kind: ExistingDirectoryKind) -> Result<bool, String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(kind.symlink_error(path)),
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err(kind.not_directory_error(path)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(kind.inspect_error(path, &e)),
    }
}

fn persistent_index_table_exists(db_path: &Path) -> Result<bool, String> {
    let table_marker = db_path.join(LANCEDB_TABLE_MARKER);
    existing_directory_state(&table_marker, ExistingDirectoryKind::TableMarker)
}

fn validate_existing_persistent_index_dir(db_path: &Path) -> Result<bool, String> {
    existing_directory_state(db_path, ExistingDirectoryKind::PersistentIndexPath)
}

fn require_persistent_index_marker(db_path: &Path, operation: &str) -> Result<(), String> {
    if persistent_index_marker_matches(db_path)? {
        Ok(())
    } else {
        Err(format!(
            "refusing to {operation} unmarked semantic index directory {}; \
             remove it manually or choose an empty --db-path",
            db_path.display()
        ))
    }
}

pub(super) struct PersistentIndexLock {
    _file: std::fs::File,
}

pub(super) fn acquire_persistent_index_lock(db_path: &Path) -> Result<PersistentIndexLock, String> {
    use fs4::fs_std::FileExt as _;

    let lock_path = persistent_index_lock_path(db_path);
    if let Some(parent) = lock_path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create index lock parent dir: {e}"))?;
        }
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|e| format!("failed to open index lock {}: {e}", lock_path.display()))?;
    file.lock_exclusive()
        .map_err(|e| format!("failed to lock index cache {}: {e}", lock_path.display()))?;

    Ok(PersistentIndexLock { _file: file })
}

/// Remove `db_path` directory (and all its contents) to clear stale index data.
///
/// If the directory does not exist, returns `Ok(())` (idempotent).
/// Used by the full-rebuild path before opening a fresh index.
pub(super) fn clear_persistent_index_dir(db_path: &Path) -> Result<(), String> {
    if !validate_existing_persistent_index_dir(db_path)? {
        remove_persistent_index_marker(db_path)?;
        return Ok(());
    }
    require_persistent_index_marker(db_path, "clear")?;
    std::fs::remove_dir_all(db_path)
        .map_err(|e| format!("failed to clear stale index at {}: {e}", db_path.display()))?;
    remove_persistent_index_marker(db_path)
}

/// A `SemanticIndexPort` proxy that makes `insert` and `insert_batch` no-ops
/// while forwarding `delete_by_source_path` and `search` to the wrapped adapter.
///
/// Used on the reuse / incremental path: the persistent index is already
/// populated (or partially updated) before the interactors run.
pub(super) struct NullInsertIndexProxy {
    inner: Arc<LanceDbSemanticIndexAdapter>,
    _cache_lock: PersistentIndexLock,
}

impl NullInsertIndexProxy {
    pub(super) fn new(
        inner: Arc<LanceDbSemanticIndexAdapter>,
        cache_lock: PersistentIndexLock,
    ) -> Self {
        Self { inner, _cache_lock: cache_lock }
    }
}

impl SemanticIndexPort for NullInsertIndexProxy {
    fn insert(
        &self,
        _fragment: &CodeFragment,
        _embedding: &[f32],
    ) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn insert_batch(&self, _items: &[(CodeFragment, Vec<f32>)]) -> Result<(), SemanticIndexError> {
        Ok(())
    }

    fn delete_by_source_path(&self, source_path: &Path) -> Result<(), SemanticIndexError> {
        self.inner.delete_by_source_path(source_path)
    }

    fn search(
        &self,
        embedding: &[f32],
        top_k: TopK,
    ) -> Result<Vec<SimilarFragment>, SemanticIndexError> {
        self.inner.search(embedding, top_k)
    }
}

fn group_fragment_refs_by_path(fragments: &[CodeFragment]) -> HashMap<String, Vec<&CodeFragment>> {
    let mut by_path: HashMap<String, Vec<&CodeFragment>> = Default::default();
    for fragment in fragments {
        by_path.entry(manifest_source_path_key(fragment)).or_default().push(fragment);
    }
    by_path
}

fn cloned_fragment_refs(fragments: &[&CodeFragment]) -> Vec<CodeFragment> {
    fragments.iter().map(|fragment| (*fragment).clone()).collect()
}

fn manifest_for_fragments(fragments: &[CodeFragment]) -> IndexManifest {
    let by_path = group_fragment_refs_by_path(fragments);
    let mut manifest = IndexManifest::empty(EMBEDDING_MODEL_ID);
    for (path_str, frags) in &by_path {
        manifest.files.insert(path_str.clone(), file_content_hash(frags));
    }
    manifest
}

fn updated_manifest_after_diff(
    stored_manifest: Option<IndexManifest>,
    by_path: &HashMap<String, Vec<&CodeFragment>>,
    dirty: &[String],
    deleted: &[String],
) -> IndexManifest {
    let mut new_manifest =
        stored_manifest.unwrap_or_else(|| IndexManifest::empty(EMBEDDING_MODEL_ID));
    new_manifest.embedding_model_id = EMBEDDING_MODEL_ID.to_owned();
    for path_str in deleted {
        new_manifest.files.remove(path_str);
    }
    for path_str in dirty {
        if let Some(frags) = by_path.get(path_str) {
            new_manifest.files.insert(path_str.clone(), file_content_hash(frags));
        }
    }
    new_manifest
}

fn batch_operation_label(context: &str, operation: &str) -> String {
    if context.starts_with("for ") {
        format!("{operation} {context}")
    } else {
        format!("{context} {operation}")
    }
}

fn embed_and_insert(
    adapter: &LanceDbSemanticIndexAdapter,
    embedding_port: &dyn EmbeddingPort,
    fragments: Vec<CodeFragment>,
    context: &str,
) -> Result<(), String> {
    let insert_label = batch_operation_label(context, "insert_batch");
    if fragments.is_empty() {
        return adapter
            .insert_batch(&[])
            .map_err(|e| format!("{insert_label} (empty) failed: {e}"));
    }

    let embed_label = batch_operation_label(context, "embed_batch");
    let embeddings =
        embedding_port.embed_batch(&fragments).map_err(|e| format!("{embed_label} failed: {e}"))?;
    if embeddings.len() != fragments.len() {
        return Err(format!(
            "{embed_label} returned {} embeddings for {} fragments",
            embeddings.len(),
            fragments.len()
        ));
    }

    let items: Vec<(CodeFragment, Vec<f32>)> = fragments.into_iter().zip(embeddings).collect();
    adapter.insert_batch(&items).map_err(|e| format!("{insert_label} failed: {e}"))
}

fn cleanup_incomplete_index_error(db_path: &Path, error: String) -> String {
    if let Err(cleanup_error) = clear_persistent_index_dir(db_path) {
        format!("{error}; additionally failed to clean incomplete index cache: {cleanup_error}")
    } else {
        error
    }
}

fn finalize_index_with_manifest(
    db_path: &Path,
    manifest_sidecar: &Path,
    manifest: &IndexManifest,
    adapter: LanceDbSemanticIndexAdapter,
    cache_lock: PersistentIndexLock,
) -> Result<Arc<dyn SemanticIndexPort>, String> {
    if let Err(e) = write_manifest(manifest_sidecar, manifest) {
        let mut cleanup_errors = Vec::new();
        if let Err(cleanup_error) = remove_manifest(manifest_sidecar) {
            cleanup_errors.push(cleanup_error);
        }
        drop(adapter);
        if let Err(cleanup_error) = clear_persistent_index_dir(db_path) {
            cleanup_errors.push(cleanup_error);
        }
        if !cleanup_errors.is_empty() {
            return Err(format!(
                "{e}; additionally failed to clean incomplete index cache: {}",
                cleanup_errors.join("; ")
            ));
        }
        return Err(e);
    }

    Ok(Arc::new(NullInsertIndexProxy::new(Arc::new(adapter), cache_lock)))
}

/// Open (or create) the persistent LanceDB semantic index at `db_path` and
/// apply incremental updates based on the file-level content-hash manifest.
///
/// In all outcomes the returned `Arc<dyn SemanticIndexPort>` is wrapped in
/// [`NullInsertIndexProxy`] so that the interactors' unconditional
/// `build_corpus_index` call is always a no-op; the corpus is correct before
/// the interactors run.
pub(super) fn open_persistent_index_with_corpus(
    db_path: &Path,
    corpus_fragments: Vec<CodeFragment>,
    embedding_port: &dyn EmbeddingPort,
) -> Result<Arc<dyn SemanticIndexPort>, String> {
    let cache_lock = acquire_persistent_index_lock(db_path)?;
    let manifest_sidecar = manifest_sidecar_path(db_path);

    let stored_manifest = read_manifest(&manifest_sidecar)?;
    let diff =
        compute_manifest_diff(&corpus_fragments, stored_manifest.as_ref(), EMBEDDING_MODEL_ID);

    let index_exists = validate_existing_persistent_index_dir(db_path)?;
    if index_exists {
        require_persistent_index_marker(db_path, "use")?;
    }

    let table_exists = if index_exists { persistent_index_table_exists(db_path)? } else { false };
    let table_manifest_mismatch = stored_manifest.as_ref().is_some_and(|manifest| {
        if manifest.files.is_empty() { table_exists } else { !table_exists }
    });

    let needs_full_rebuild = stored_manifest
        .as_ref()
        .map(|m| m.embedding_model_id != EMBEDDING_MODEL_ID)
        .unwrap_or(true)
        || !index_exists
        || table_manifest_mismatch;

    if needs_full_rebuild {
        return full_rebuild_index(
            db_path,
            corpus_fragments,
            embedding_port,
            &manifest_sidecar,
            cache_lock,
        );
    }

    if diff.dirty.is_empty() && diff.deleted.is_empty() {
        let adapter = LanceDbSemanticIndexAdapter::new(db_path.to_path_buf()).map_err(|e| {
            format!("failed to open persistent index at {}: {e}", db_path.display())
        })?;
        return Ok(Arc::new(NullInsertIndexProxy::new(Arc::new(adapter), cache_lock)));
    }

    let adapter = LanceDbSemanticIndexAdapter::new(db_path.to_path_buf())
        .map_err(|e| format!("failed to open persistent index at {}: {e}", db_path.display()))?;

    let by_path = group_fragment_refs_by_path(&corpus_fragments);

    remove_manifest(&manifest_sidecar)?;

    let update_result: Result<IndexManifest, String> = (|| {
        for path_str in &diff.dirty {
            let path = Path::new(path_str);
            adapter
                .delete_by_source_path(path)
                .map_err(|e| format!("incremental delete for '{path_str}' failed: {e}"))?;
        }

        for path_str in &diff.deleted {
            let path = Path::new(path_str);
            adapter.delete_by_source_path(path).map_err(|e| {
                format!("incremental delete (removed file) for '{path_str}' failed: {e}")
            })?;
        }

        for path_str in &diff.dirty {
            let frags = match by_path.get(path_str) {
                Some(f) if !f.is_empty() => f,
                _ => continue,
            };
            embed_and_insert(
                &adapter,
                embedding_port,
                cloned_fragment_refs(frags),
                &format!("for '{path_str}'"),
            )?;
        }

        Ok(updated_manifest_after_diff(stored_manifest, &by_path, &diff.dirty, &diff.deleted))
    })();

    let new_manifest = match update_result {
        Ok(manifest) => manifest,
        Err(e) => {
            drop(adapter);
            return Err(cleanup_incomplete_index_error(db_path, e));
        }
    };

    finalize_index_with_manifest(db_path, &manifest_sidecar, &new_manifest, adapter, cache_lock)
}

/// Full rebuild: clear the existing index, embed all corpus fragments, insert
/// all, write a fresh manifest, return the proxy.
fn full_rebuild_index(
    db_path: &Path,
    corpus_fragments: Vec<CodeFragment>,
    embedding_port: &dyn EmbeddingPort,
    manifest_sidecar: &Path,
    cache_lock: PersistentIndexLock,
) -> Result<Arc<dyn SemanticIndexPort>, String> {
    remove_manifest(manifest_sidecar)?;
    clear_persistent_index_dir(db_path)?;
    write_persistent_index_marker(db_path)?;

    let adapter = match LanceDbSemanticIndexAdapter::new(db_path.to_path_buf()) {
        Ok(a) => a,
        Err(e) => {
            let _ = clear_persistent_index_dir(db_path);
            return Err(format!(
                "failed to open fresh persistent index at {}: {e}",
                db_path.display()
            ));
        }
    };

    let manifest = manifest_for_fragments(&corpus_fragments);

    let embed_insert_result =
        embed_and_insert(&adapter, embedding_port, corpus_fragments, "full rebuild");

    if let Err(e) = embed_insert_result {
        drop(adapter);
        return Err(cleanup_incomplete_index_error(db_path, e));
    }

    finalize_index_with_manifest(db_path, manifest_sidecar, &manifest, adapter, cache_lock)
}
