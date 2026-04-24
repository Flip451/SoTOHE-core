//! Filesystem adapter implementing `CatalogueSpecSignalsWriter` via
//! `<items_dir>/<track_id>/<layer_id>-catalogue-spec-signals.json` atomic write.
//!
//! Companion to `FsTrackStore` / `FsReviewStore`: items-dir + atomic
//! temp-file + fsync + rename pattern so partial writes cannot be observed
//! by concurrent readers. The codec (schema_version 1 DTO, `deny_unknown_fields`,
//! deterministic output) lives in `catalogue_spec_signals_codec.rs` (T010).
//!
//! Caller passes the items directory (typically `<workspace_root>/track/items`
//! or a custom path via `--items-dir`) so read and write paths resolve against
//! the same tree — matching `FsTrackStore::new(items_dir)`.
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D2 / §D3.1 / IN-08.

use std::path::PathBuf;

use domain::{CatalogueSpecSignalsDocument, RepositoryError, TrackId};
use usecase::catalogue_spec_signals::CatalogueSpecSignalsWriter;

use crate::tddd::catalogue_spec_signals_codec;
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

/// Filesystem adapter for `<layer>-catalogue-spec-signals.json`.
///
/// Construct with [`FsCatalogueSpecSignalsStore::new`] passing the items
/// directory. The adapter joins the items dir with
/// `<track_id>/<layer_id>-catalogue-spec-signals.json` at write time.
pub struct FsCatalogueSpecSignalsStore {
    items_dir: PathBuf,
}

impl FsCatalogueSpecSignalsStore {
    /// Creates a new store rooted at the given items directory.
    #[must_use]
    pub fn new(items_dir: PathBuf) -> Self {
        Self { items_dir }
    }

    /// Resolves the signals file path for a track + layer pair.
    fn resolve_path(&self, track_id: &TrackId, layer_id: &str) -> PathBuf {
        self.items_dir
            .join(track_id.as_ref())
            .join(format!("{layer_id}-catalogue-spec-signals.json"))
    }
}

/// Rejects unsafe path characters in `layer_id` to prevent path traversal.
///
/// Mirrors the validation used in `type_graph_render::validate_layer_id`.
fn validate_layer_id(layer_id: &str) -> Result<(), std::io::Error> {
    if layer_id.is_empty()
        || layer_id.contains('/')
        || layer_id.contains('\\')
        || layer_id.contains(':')
        || layer_id == ".."
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            format!("layer_id contains unsafe path characters: {layer_id:?}"),
        ));
    }
    Ok(())
}

impl CatalogueSpecSignalsWriter for FsCatalogueSpecSignalsStore {
    fn write_catalogue_spec_signals(
        &self,
        track_id: &TrackId,
        layer_id: &str,
        doc: &CatalogueSpecSignalsDocument,
    ) -> Result<(), RepositoryError> {
        // Reject unsafe path characters before composing the output path.
        validate_layer_id(layer_id).map_err(|e| {
            RepositoryError::Message(format!(
                "catalogue-spec signals: invalid layer_id '{layer_id}': {e}"
            ))
        })?;

        let path = self.resolve_path(track_id, layer_id);

        // Reject symlinks at any path component below items_dir before writing.
        reject_symlinks_below(&path, &self.items_dir).map_err(|source| {
            RepositoryError::Message(format!(
                "catalogue-spec signals symlink guard failed for layer '{layer_id}' at '{}': {source}",
                path.display()
            ))
        })?;

        // Encode via the T010 codec — deterministic output, schema_version=1 pinned.
        let content = catalogue_spec_signals_codec::encode(doc).map_err(|e| {
            RepositoryError::Message(format!(
                "catalogue-spec signals encode error for layer '{layer_id}': {e}"
            ))
        })?;

        atomic_write_file(&path, content.as_bytes()).map_err(|source| {
            RepositoryError::Message(format!(
                "catalogue-spec signals write error for layer '{layer_id}' at '{}': {source}",
                path.display()
            ))
        })
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::fs;

    use domain::{CatalogueSpecSignal, ConfidenceSignal, ContentHash};
    use tempfile::tempdir;

    use super::*;

    fn hex_pattern(byte: u8) -> String {
        let mut s = String::with_capacity(64);
        for _ in 0..32 {
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }

    fn sample_doc() -> CatalogueSpecSignalsDocument {
        CatalogueSpecSignalsDocument::new(
            ContentHash::try_from_hex(hex_pattern(0xab)).unwrap(),
            vec![
                CatalogueSpecSignal::new("Foo", ConfidenceSignal::Blue),
                CatalogueSpecSignal::new("Bar", ConfidenceSignal::Yellow),
            ],
        )
    }

    /// Builds a fresh `items_dir` rooted at a tempdir and pre-creates the track
    /// directory inside it. Returns `(tempdir, items_dir)`.
    fn setup_items_dir(track_id: &str) -> (tempfile::TempDir, PathBuf) {
        let dir = tempdir().unwrap();
        let items_dir = dir.path().to_path_buf();
        fs::create_dir_all(items_dir.join(track_id)).unwrap();
        (dir, items_dir)
    }

    #[test]
    fn resolve_path_concatenates_track_and_layer() {
        let store = FsCatalogueSpecSignalsStore::new(PathBuf::from("/items"));
        let track = TrackId::try_new("my-track").unwrap();
        let path = store.resolve_path(&track, "domain");
        assert_eq!(path, PathBuf::from("/items/my-track/domain-catalogue-spec-signals.json"));
    }

    #[test]
    fn write_persists_document_to_filesystem() {
        let (_tmp, items_dir) = setup_items_dir("my-track");
        let store = FsCatalogueSpecSignalsStore::new(items_dir.clone());
        let track = TrackId::try_new("my-track").unwrap();
        let doc = sample_doc();

        store.write_catalogue_spec_signals(&track, "domain", &doc).unwrap();

        let path = items_dir.join("my-track/domain-catalogue-spec-signals.json");
        assert!(path.exists());
        let content = fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"schema_version\": 1"));
        assert!(content.contains("\"Foo\""));
        assert!(content.contains("\"Bar\""));
    }

    #[test]
    fn write_round_trips_via_codec() {
        let (_tmp, items_dir) = setup_items_dir("my-track");
        let store = FsCatalogueSpecSignalsStore::new(items_dir.clone());
        let track = TrackId::try_new("my-track").unwrap();
        let doc = sample_doc();

        store.write_catalogue_spec_signals(&track, "domain", &doc).unwrap();

        let path = items_dir.join("my-track/domain-catalogue-spec-signals.json");
        let content = fs::read_to_string(&path).unwrap();
        let decoded = catalogue_spec_signals_codec::decode(&content).unwrap();
        assert_eq!(decoded, doc);
    }

    #[test]
    fn write_is_idempotent_on_repeated_calls() {
        let (_tmp, items_dir) = setup_items_dir("my-track");
        let store = FsCatalogueSpecSignalsStore::new(items_dir.clone());
        let track = TrackId::try_new("my-track").unwrap();
        let doc = sample_doc();

        // First write.
        store.write_catalogue_spec_signals(&track, "domain", &doc).unwrap();
        let path = items_dir.join("my-track/domain-catalogue-spec-signals.json");
        let first = fs::read_to_string(&path).unwrap();

        // Second write with identical document.
        store.write_catalogue_spec_signals(&track, "domain", &doc).unwrap();
        let second = fs::read_to_string(&path).unwrap();

        assert_eq!(first, second, "deterministic encode + atomic write ⇒ byte-identical output");
    }

    #[test]
    fn write_overwrites_existing_file() {
        let (_tmp, items_dir) = setup_items_dir("my-track");
        let store = FsCatalogueSpecSignalsStore::new(items_dir.clone());
        let track = TrackId::try_new("my-track").unwrap();

        let first_doc = sample_doc();
        store.write_catalogue_spec_signals(&track, "domain", &first_doc).unwrap();

        let second_doc = CatalogueSpecSignalsDocument::new(
            ContentHash::try_from_hex(hex_pattern(0xcd)).unwrap(),
            vec![CatalogueSpecSignal::new("Updated", ConfidenceSignal::Blue)],
        );
        store.write_catalogue_spec_signals(&track, "domain", &second_doc).unwrap();

        let path = items_dir.join("my-track/domain-catalogue-spec-signals.json");
        let content = fs::read_to_string(&path).unwrap();
        let decoded = catalogue_spec_signals_codec::decode(&content).unwrap();
        assert_eq!(decoded, second_doc);
        assert!(!content.contains("Foo")); // first_doc contents replaced
    }

    #[test]
    fn write_fails_when_track_dir_missing() {
        let dir = tempdir().unwrap();
        let store = FsCatalogueSpecSignalsStore::new(dir.path().to_path_buf());
        let track = TrackId::try_new("missing-track").unwrap();
        let doc = sample_doc();

        let err = store.write_catalogue_spec_signals(&track, "domain", &doc).unwrap_err();
        match err {
            RepositoryError::Message(msg) => {
                assert!(
                    msg.contains("missing-track") || msg.contains("write error"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected RepositoryError::Message, got {other:?}"),
        }
    }

    #[test]
    fn write_rejects_layer_id_with_path_separator() {
        let (_tmp, items_dir) = setup_items_dir("my-track");
        let store = FsCatalogueSpecSignalsStore::new(items_dir);
        let track = TrackId::try_new("my-track").unwrap();
        let doc = sample_doc();

        let err = store.write_catalogue_spec_signals(&track, "../evil", &doc).unwrap_err();
        match err {
            RepositoryError::Message(msg) => {
                assert!(
                    msg.contains("invalid layer_id") || msg.contains("unsafe path"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected RepositoryError::Message, got {other:?}"),
        }
    }

    #[test]
    fn write_rejects_empty_layer_id() {
        let (_tmp, items_dir) = setup_items_dir("my-track");
        let store = FsCatalogueSpecSignalsStore::new(items_dir);
        let track = TrackId::try_new("my-track").unwrap();
        let doc = sample_doc();

        let err = store.write_catalogue_spec_signals(&track, "", &doc).unwrap_err();
        match err {
            RepositoryError::Message(msg) => {
                assert!(
                    msg.contains("invalid layer_id") || msg.contains("unsafe path"),
                    "unexpected error message: {msg}"
                );
            }
            other => panic!("expected RepositoryError::Message, got {other:?}"),
        }
    }

    #[cfg(unix)]
    #[test]
    fn write_rejects_symlinked_track_dir() {
        let dir = tempdir().unwrap();
        let items_dir = dir.path().to_path_buf();

        // Create a real track directory elsewhere.
        let real_track = dir.path().join("real-track-target");
        fs::create_dir_all(&real_track).unwrap();

        // Create a symlink at the expected track path pointing to the real directory.
        fs::create_dir_all(&items_dir).unwrap();
        let symlink_path = items_dir.join("my-track");
        std::os::unix::fs::symlink(&real_track, &symlink_path).unwrap();

        let store = FsCatalogueSpecSignalsStore::new(items_dir);
        let track = TrackId::try_new("my-track").unwrap();
        let doc = sample_doc();

        let err = store.write_catalogue_spec_signals(&track, "domain", &doc).unwrap_err();
        match err {
            RepositoryError::Message(msg) => {
                assert!(
                    msg.contains("symlink") || msg.contains("refusing"),
                    "expected symlink guard error, got: {msg}"
                );
            }
            other => panic!("expected RepositoryError::Message, got {other:?}"),
        }
    }

    #[test]
    fn multiple_layers_write_to_distinct_paths() {
        let (_tmp, items_dir) = setup_items_dir("my-track");
        let store = FsCatalogueSpecSignalsStore::new(items_dir.clone());
        let track = TrackId::try_new("my-track").unwrap();
        let doc = sample_doc();

        store.write_catalogue_spec_signals(&track, "domain", &doc).unwrap();
        store.write_catalogue_spec_signals(&track, "usecase", &doc).unwrap();

        assert!(items_dir.join("my-track/domain-catalogue-spec-signals.json").exists());
        assert!(items_dir.join("my-track/usecase-catalogue-spec-signals.json").exists());
    }
}
