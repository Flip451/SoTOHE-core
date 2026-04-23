//! Catalogue-spec signals (SoT Chain ②) usecase orchestration.
//!
//! This module introduces the `CatalogueSpecSignalsWriter` secondary port
//! required by ADR `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D2 / IN-05. The port exposes a single atomic-write entry point used by
//! the refresh interactor (authored in T008) to persist
//! `<layer>-catalogue-spec-signals.json`.
//!
//! The port lives in usecase rather than `libs/domain/src/repository.rs`
//! because the write concern is track-scoped and layer-specific — consistent
//! with the adjacent `TrackBlobReader` (in `merge_gate.rs`) which also lives
//! in usecase.

use domain::{CatalogueSpecSignalsDocument, RepositoryError, TrackId};

/// Secondary port for writing `<layer>-catalogue-spec-signals.json`.
///
/// The infrastructure adapter (`FsCatalogueSpecSignalsStore`, added in T012)
/// implements this trait over the filesystem at
/// `track/items/<track_id>/<layer_id>-catalogue-spec-signals.json`. Writes
/// must be atomic (temp file + rename) in the same pattern as `FsTrackStore`
/// / `FsReviewStore` so a partial write cannot be observed by a concurrent
/// reader.
///
/// Reference: ADR `2026-04-23-0344-catalogue-spec-signal-activation.md`
/// §D2.2 (schema + determinism) / §D2.3 (stale-detection hash) / IN-05.
pub trait CatalogueSpecSignalsWriter: Send + Sync {
    /// Atomically persists `<layer>-catalogue-spec-signals.json` for the
    /// given track + layer pair.
    ///
    /// # Errors
    ///
    /// Returns [`RepositoryError`] on I/O, encode, or filesystem layout
    /// failures (e.g. the track directory does not exist, a concurrent write
    /// could not acquire the rename slot, the layer id contains invalid
    /// filesystem characters). Callers treat any error as BLOCKED —
    /// catalogue-spec signal persistence is a pre-commit prerequisite
    /// (ADR §D3.4) so a failure here must surface at the CLI layer instead
    /// of being silently ignored.
    fn write_catalogue_spec_signals(
        &self,
        track_id: &TrackId,
        layer_id: &str,
        doc: &CatalogueSpecSignalsDocument,
    ) -> Result<(), RepositoryError>;
}

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::sync::Mutex;

    use domain::ContentHash;

    use super::*;

    /// In-memory recording implementation used by future interactor tests
    /// (T008 RefreshCatalogueSpecSignalsInteractor). Kept here so the port
    /// contract is exercised without waiting for the infrastructure
    /// adapter in T012.
    struct RecordingWriter {
        calls: Mutex<Vec<(String, String)>>, // (track_id, layer_id)
    }

    impl RecordingWriter {
        fn new() -> Self {
            Self { calls: Mutex::new(Vec::new()) }
        }
    }

    impl CatalogueSpecSignalsWriter for RecordingWriter {
        fn write_catalogue_spec_signals(
            &self,
            track_id: &TrackId,
            layer_id: &str,
            _doc: &CatalogueSpecSignalsDocument,
        ) -> Result<(), RepositoryError> {
            self.calls.lock().unwrap().push((track_id.as_ref().to_owned(), layer_id.to_owned()));
            Ok(())
        }
    }

    fn sample_doc() -> CatalogueSpecSignalsDocument {
        CatalogueSpecSignalsDocument::new(ContentHash::from_bytes([0xaa; 32]), Vec::new())
    }

    #[test]
    fn trait_is_object_safe() {
        // Compile-time check: `dyn CatalogueSpecSignalsWriter` must be usable.
        let writer: Box<dyn CatalogueSpecSignalsWriter> = Box::new(RecordingWriter::new());
        let track = TrackId::try_new("example-track").unwrap();
        let doc = sample_doc();
        writer.write_catalogue_spec_signals(&track, "domain", &doc).unwrap();
    }

    #[test]
    fn recording_writer_captures_track_and_layer() {
        let writer = RecordingWriter::new();
        let track = TrackId::try_new("example-track").unwrap();
        let doc = sample_doc();
        writer.write_catalogue_spec_signals(&track, "domain", &doc).unwrap();
        writer.write_catalogue_spec_signals(&track, "usecase", &doc).unwrap();

        let calls = writer.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0], ("example-track".to_owned(), "domain".to_owned()));
        assert_eq!(calls[1], ("example-track".to_owned(), "usecase".to_owned()));
    }
}
