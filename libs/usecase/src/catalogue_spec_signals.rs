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

use domain::tddd::LayerId;
use domain::{
    CatalogueSpecSignal, CatalogueSpecSignalsDocument, ContentHash, RepositoryError, TrackId,
    ValidationError, evaluate_catalogue_entry_signal,
};
use thiserror::Error;

use crate::merge_gate::{BlobFetchResult, TrackBlobReader};

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
/// Command input for [`RefreshCatalogueSpecSignals::execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RefreshCatalogueSpecSignalsCommand {
    /// Current git branch (e.g. `"track/my-feature-2026-04-24"`). Used by the
    /// active-track guard (CN-07) to reject non-`track/` branches.
    pub branch: String,
    /// Track identifier the signals file is scoped to.
    pub track_id: TrackId,
    /// Validated layer identifier for the catalogue being refreshed (e.g.
    /// `"domain"`). Using [`LayerId`] ensures malformed layer strings are
    /// rejected at the CLI boundary before reaching the port adapters.
    pub layer_id: LayerId,
}

/// Failure modes of [`RefreshCatalogueSpecSignalsInteractor::execute`].
#[derive(Debug, Error)]
pub enum RefreshCatalogueSpecSignalsError {
    /// The supplied branch does not start with `track/`, so the guard
    /// (CN-07) rejects it to keep signals off archived / main / plan/
    /// branches.
    #[error("refresh rejected: branch '{branch}' is not an active track branch (CN-07)")]
    NonActiveTrack { branch: String },

    /// The branch `track/<suffix>` disagrees with the track_id argument.
    /// Safeguards against CLI wrappers that mishandle branch/track_id mapping.
    #[error(
        "refresh rejected: branch '{branch}' does not match track_id '{track_id}' (expected 'track/{track_id}')"
    )]
    BranchTrackMismatch { branch: String, track_id: String },

    /// `read_catalogue_for_spec_ref_check` did not find the catalogue file
    /// for the requested layer on the branch.
    #[error("catalogue not found for layer '{layer_id}' on branch '{branch}'")]
    CatalogueNotFound { branch: String, layer_id: String },

    /// The blob-fetch port returned a fetch error (I/O, decode, etc).
    #[error("catalogue fetch error for layer '{layer_id}': {message}")]
    FetchError { layer_id: String, message: String },

    /// The raw-bytes hash returned by the port was not valid canonical hex.
    /// Callers construct a [`ContentHash`] from the string; this variant
    /// surfaces the parse failure.
    #[error(
        "catalogue declaration hash for layer '{layer_id}' is not valid canonical SHA-256 hex: {source}"
    )]
    InvalidCatalogueHash {
        layer_id: String,
        #[source]
        source: ValidationError,
    },

    /// The infrastructure writer failed to persist
    /// `<layer>-catalogue-spec-signals.json`.
    #[error("failed to write catalogue-spec signals for layer '{layer_id}': {source}")]
    WriteFailed {
        layer_id: String,
        #[source]
        source: RepositoryError,
    },
}

/// Primary port for the "refresh catalogue-spec signals" use case.
///
/// Executed by `sotp track catalogue-spec-signals` (T013) once per
/// catalogue-spec-signal-enabled layer. Re-exported via `Interactor` so
/// the CLI can depend on the trait rather than the concrete struct.
pub trait RefreshCatalogueSpecSignals: Send + Sync {
    /// Runs the 5-step refresh pipeline defined in ADR §D2 / §D3.1:
    ///
    /// 1. Read `<layer>-types.json` via
    ///    [`TrackBlobReader::read_catalogue_for_spec_ref_check`] to obtain
    ///    `(TypeCatalogueDocument, String)` where the `String` is the
    ///    raw-bytes SHA-256 hex used as `catalogue_declaration_hash`.
    /// 2. Evaluate per-entry signals with
    ///    [`evaluate_catalogue_entry_signal`].
    /// 3. Parse the step-1 hex into a [`ContentHash`] for schema integrity.
    /// 4. Build a [`CatalogueSpecSignalsDocument`] (schema_version pinned to
    ///    1, no `generated_at`).
    /// 5. Atomically persist via
    ///    [`CatalogueSpecSignalsWriter::write_catalogue_spec_signals`].
    ///
    /// The active-track guard (CN-07) runs before step 1 and short-circuits
    /// non-`track/` branches + branch/track_id mismatches.
    ///
    /// # Errors
    ///
    /// Returns [`RefreshCatalogueSpecSignalsError`] on any pipeline failure.
    fn execute(
        &self,
        cmd: &RefreshCatalogueSpecSignalsCommand,
    ) -> Result<(), RefreshCatalogueSpecSignalsError>;
}

/// Default [`RefreshCatalogueSpecSignals`] implementation wiring
/// [`TrackBlobReader`] + [`CatalogueSpecSignalsWriter`] ports together.
pub struct RefreshCatalogueSpecSignalsInteractor<R, W>
where
    R: TrackBlobReader,
    W: CatalogueSpecSignalsWriter,
{
    reader: R,
    writer: W,
}

impl<R, W> RefreshCatalogueSpecSignalsInteractor<R, W>
where
    R: TrackBlobReader,
    W: CatalogueSpecSignalsWriter,
{
    /// Creates a new interactor wrapping the supplied secondary ports.
    #[must_use]
    pub fn new(reader: R, writer: W) -> Self {
        Self { reader, writer }
    }
}

impl<R, W> RefreshCatalogueSpecSignals for RefreshCatalogueSpecSignalsInteractor<R, W>
where
    R: TrackBlobReader + Send + Sync,
    W: CatalogueSpecSignalsWriter,
{
    fn execute(
        &self,
        cmd: &RefreshCatalogueSpecSignalsCommand,
    ) -> Result<(), RefreshCatalogueSpecSignalsError> {
        // Active-track guard (CN-07): reject non-`track/` branches.
        let suffix = cmd.branch.strip_prefix("track/").ok_or_else(|| {
            RefreshCatalogueSpecSignalsError::NonActiveTrack { branch: cmd.branch.clone() }
        })?;
        if suffix != cmd.track_id.as_ref() {
            return Err(RefreshCatalogueSpecSignalsError::BranchTrackMismatch {
                branch: cmd.branch.clone(),
                track_id: cmd.track_id.as_ref().to_owned(),
            });
        }

        // Step 1: read the catalogue + raw-bytes SHA-256.
        let (catalogue, catalogue_hash_hex) = match self.reader.read_catalogue_for_spec_ref_check(
            &cmd.branch,
            cmd.track_id.as_ref(),
            cmd.layer_id.as_ref(),
        ) {
            BlobFetchResult::Found(pair) => pair,
            BlobFetchResult::NotFound => {
                return Err(RefreshCatalogueSpecSignalsError::CatalogueNotFound {
                    branch: cmd.branch.clone(),
                    layer_id: cmd.layer_id.as_ref().to_owned(),
                });
            }
            BlobFetchResult::FetchError(message) => {
                return Err(RefreshCatalogueSpecSignalsError::FetchError {
                    layer_id: cmd.layer_id.as_ref().to_owned(),
                    message,
                });
            }
        };

        // Step 2: evaluate per-entry signals.
        let signals: Vec<CatalogueSpecSignal> = catalogue
            .entries()
            .iter()
            .map(|entry| {
                let signal =
                    evaluate_catalogue_entry_signal(entry.spec_refs(), entry.informal_grounds());
                CatalogueSpecSignal::new(entry.name(), signal)
            })
            .collect();

        // Step 3: parse the raw-bytes hex into a domain ContentHash.
        let catalogue_declaration_hash =
            ContentHash::try_from_hex(&catalogue_hash_hex).map_err(|source| {
                RefreshCatalogueSpecSignalsError::InvalidCatalogueHash {
                    layer_id: cmd.layer_id.as_ref().to_owned(),
                    source,
                }
            })?;

        // Step 4: build the document (schema_version=1 pinned, generated_at absent).
        let doc = CatalogueSpecSignalsDocument::new(catalogue_declaration_hash, signals);

        // Step 5: atomic write via the writer port.
        self.writer
            .write_catalogue_spec_signals(&cmd.track_id, cmd.layer_id.as_ref(), &doc)
            .map_err(|source| RefreshCatalogueSpecSignalsError::WriteFailed {
                layer_id: cmd.layer_id.as_ref().to_owned(),
                source,
            })?;

        Ok(())
    }
}

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
    use domain::tddd::LayerId;

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

    // ---------------------------------------------------------------------------
    // RefreshCatalogueSpecSignalsInteractor (T008, IN-05)
    // ---------------------------------------------------------------------------

    use domain::spec::SpecDocument;
    use domain::tddd::catalogue::{
        TypeAction, TypeCatalogueDocument, TypeCatalogueEntry, TypeDefinitionKind,
    };
    use domain::{ImplPlanDocument, InformalGroundKind, InformalGroundRef, InformalGroundSummary};

    /// Test-only mock reader that returns a fixed catalogue + raw-bytes hash.
    struct FixedReader {
        catalogue: Mutex<Option<TypeCatalogueDocument>>,
        hash_hex: String,
        /// If true, `read_catalogue_for_spec_ref_check` returns `NotFound`.
        not_found: bool,
        /// If non-empty, `read_catalogue_for_spec_ref_check` returns `FetchError(...)`.
        fetch_error: Option<String>,
    }

    impl FixedReader {
        fn found(catalogue: TypeCatalogueDocument, hash_hex: impl Into<String>) -> Self {
            Self {
                catalogue: Mutex::new(Some(catalogue)),
                hash_hex: hash_hex.into(),
                not_found: false,
                fetch_error: None,
            }
        }

        fn not_found() -> Self {
            Self {
                catalogue: Mutex::new(None),
                hash_hex: String::new(),
                not_found: true,
                fetch_error: None,
            }
        }

        fn fetch_error(message: impl Into<String>) -> Self {
            Self {
                catalogue: Mutex::new(None),
                hash_hex: String::new(),
                not_found: false,
                fetch_error: Some(message.into()),
            }
        }
    }

    impl TrackBlobReader for FixedReader {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            panic!("FixedReader::read_spec_document should not be called by T008 tests")
        }

        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(TypeCatalogueDocument, String)> {
            panic!("FixedReader::read_type_catalogue should not be called by T008 tests")
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<ImplPlanDocument> {
            panic!("FixedReader::read_impl_plan should not be called by T008 tests")
        }

        fn read_catalogue_for_spec_ref_check(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(TypeCatalogueDocument, String)> {
            if self.not_found {
                return BlobFetchResult::NotFound;
            }
            if let Some(msg) = &self.fetch_error {
                return BlobFetchResult::FetchError(msg.clone());
            }
            match self.catalogue.lock().unwrap().take() {
                Some(doc) => BlobFetchResult::Found((doc, self.hash_hex.clone())),
                None => panic!("FixedReader::read_catalogue_for_spec_ref_check called twice"),
            }
        }
    }

    /// Mock writer that captures every write call. Reuses the `Mutex<Vec>`
    /// recorder from the earlier tests but exposes the document payload too.
    struct CapturingWriter {
        calls: Mutex<Vec<(String, String, CatalogueSpecSignalsDocument)>>,
    }

    impl CapturingWriter {
        fn new() -> Self {
            Self { calls: Mutex::new(Vec::new()) }
        }

        fn single(&self) -> (String, String, CatalogueSpecSignalsDocument) {
            let calls = self.calls.lock().unwrap();
            assert_eq!(calls.len(), 1, "expected exactly one write call");
            calls[0].clone()
        }

        fn call_count(&self) -> usize {
            self.calls.lock().unwrap().len()
        }
    }

    impl CatalogueSpecSignalsWriter for CapturingWriter {
        fn write_catalogue_spec_signals(
            &self,
            track_id: &TrackId,
            layer_id: &str,
            doc: &CatalogueSpecSignalsDocument,
        ) -> Result<(), RepositoryError> {
            self.calls.lock().unwrap().push((
                track_id.as_ref().to_owned(),
                layer_id.to_owned(),
                doc.clone(),
            ));
            Ok(())
        }
    }

    fn catalogue_entry(name: &str, with_informal: bool) -> TypeCatalogueEntry {
        let informal = if with_informal {
            vec![InformalGroundRef::new(
                InformalGroundKind::UserDirective,
                InformalGroundSummary::try_new("test ground").unwrap(),
            )]
        } else {
            Vec::new()
        };
        TypeCatalogueEntry::with_refs(
            name,
            "test entry",
            TypeDefinitionKind::ValueObject,
            TypeAction::Add,
            true,
            Vec::new(),
            informal,
        )
        .unwrap()
    }

    fn catalogue_with(entries: Vec<TypeCatalogueEntry>) -> TypeCatalogueDocument {
        TypeCatalogueDocument::new(1, entries)
    }

    /// A 64-char lowercase hex string made of the given byte repeated 32 times.
    fn hex_pattern(byte: u8) -> String {
        let mut s = String::with_capacity(64);
        for _ in 0..32 {
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }

    fn refresh_cmd(
        branch: &str,
        track_id: &str,
        layer_id: &str,
    ) -> RefreshCatalogueSpecSignalsCommand {
        RefreshCatalogueSpecSignalsCommand {
            branch: branch.to_owned(),
            track_id: TrackId::try_new(track_id).unwrap(),
            layer_id: LayerId::try_new(layer_id).unwrap(),
        }
    }

    #[test]
    fn refresh_writes_signals_with_correct_hash_and_signals_ordering() {
        let cat = catalogue_with(vec![
            catalogue_entry("TypeA", false), // → Red (no spec_refs, no informal)
            catalogue_entry("TypeB", true),  // → Yellow
        ]);
        let hash_hex = hex_pattern(0xab);
        let reader = FixedReader::found(cat, hash_hex.clone());
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        interactor.execute(&refresh_cmd("track/my-track", "my-track", "domain")).unwrap();

        let (track_id, layer_id, doc) = interactor.writer.single();
        assert_eq!(track_id, "my-track");
        assert_eq!(layer_id, "domain");
        assert_eq!(doc.schema_version(), 1);
        assert_eq!(doc.catalogue_declaration_hash.to_hex(), hash_hex);
        assert_eq!(doc.signals.len(), 2);
        assert_eq!(doc.signals[0].type_name, "TypeA");
        assert_eq!(doc.signals[0].signal, domain::ConfidenceSignal::Red);
        assert_eq!(doc.signals[1].type_name, "TypeB");
        assert_eq!(doc.signals[1].signal, domain::ConfidenceSignal::Yellow);
    }

    #[test]
    fn refresh_rejects_non_track_branch() {
        let reader = FixedReader::not_found(); // should not be called
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        let err = interactor.execute(&refresh_cmd("main", "my-track", "domain")).unwrap_err();

        assert!(
            matches!(err, RefreshCatalogueSpecSignalsError::NonActiveTrack { ref branch } if branch == "main")
        );
        assert_eq!(interactor.writer.call_count(), 0);
    }

    #[test]
    fn refresh_rejects_branch_track_id_mismatch() {
        let reader = FixedReader::not_found(); // should not be called
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        let err = interactor
            .execute(&refresh_cmd("track/other-track", "my-track", "domain"))
            .unwrap_err();

        assert!(matches!(
            err,
            RefreshCatalogueSpecSignalsError::BranchTrackMismatch { ref branch, ref track_id }
                if branch == "track/other-track" && track_id == "my-track"
        ));
        assert_eq!(interactor.writer.call_count(), 0);
    }

    #[test]
    fn refresh_returns_catalogue_not_found_when_layer_absent() {
        let reader = FixedReader::not_found();
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        let err =
            interactor.execute(&refresh_cmd("track/my-track", "my-track", "domain")).unwrap_err();

        assert!(matches!(
            err,
            RefreshCatalogueSpecSignalsError::CatalogueNotFound { ref layer_id, .. }
                if layer_id == "domain"
        ));
        assert_eq!(interactor.writer.call_count(), 0);
    }

    #[test]
    fn refresh_propagates_fetch_errors() {
        let reader = FixedReader::fetch_error("git show failed");
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        let err =
            interactor.execute(&refresh_cmd("track/my-track", "my-track", "domain")).unwrap_err();

        assert!(matches!(
            err,
            RefreshCatalogueSpecSignalsError::FetchError { ref layer_id, ref message }
                if layer_id == "domain" && message == "git show failed"
        ));
        assert_eq!(interactor.writer.call_count(), 0);
    }

    #[test]
    fn refresh_rejects_invalid_catalogue_hash_hex() {
        let cat = catalogue_with(vec![]);
        let reader = FixedReader::found(cat, "not-a-valid-64char-hex");
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        let err =
            interactor.execute(&refresh_cmd("track/my-track", "my-track", "domain")).unwrap_err();

        assert!(matches!(
            err,
            RefreshCatalogueSpecSignalsError::InvalidCatalogueHash { ref layer_id, .. }
                if layer_id == "domain"
        ));
        assert_eq!(interactor.writer.call_count(), 0);
    }

    /// Writer that always fails — exercises the step-5 error path.
    struct FailingWriter;
    impl CatalogueSpecSignalsWriter for FailingWriter {
        fn write_catalogue_spec_signals(
            &self,
            _track_id: &TrackId,
            _layer_id: &str,
            _doc: &CatalogueSpecSignalsDocument,
        ) -> Result<(), RepositoryError> {
            Err(RepositoryError::Message("disk full".to_owned()))
        }
    }

    #[test]
    fn refresh_wraps_writer_errors() {
        let cat = catalogue_with(vec![catalogue_entry("X", false)]);
        let reader = FixedReader::found(cat, hex_pattern(0x00));
        let writer = FailingWriter;
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        let err =
            interactor.execute(&refresh_cmd("track/my-track", "my-track", "domain")).unwrap_err();

        assert!(matches!(
            err,
            RefreshCatalogueSpecSignalsError::WriteFailed { ref layer_id, .. }
                if layer_id == "domain"
        ));
    }
}
