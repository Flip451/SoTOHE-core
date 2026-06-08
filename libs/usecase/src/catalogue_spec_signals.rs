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

use crate::catalogue_traversal::iter_catalogue_entries;
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

    /// The `entry_hashes` map returned by the infrastructure port is missing
    /// a hash for the given catalogue entry key. This indicates a mismatch
    /// between the catalogue entries and the hashes computed by the adapter
    /// (e.g. a key-derivation drift). Fail-closed: persisting a fabricated
    /// zero hash would corrupt integrity metadata silently.
    #[error(
        "catalogue entry hash missing for entry '{entry_key}' in layer '{layer_id}': \
        the infrastructure adapter did not supply a hash for this key"
    )]
    MissingEntryHash { layer_id: String, entry_key: String },

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
    ///    `(CatalogueDocument, String)` where the `String` is the
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

        // Step 1: read the catalogue + raw-bytes SHA-256 + per-entry canonical JSON hashes.
        // Infrastructure computes `entry_hashes` so domain/usecase never import
        // `canonical_json_sha256` (CN-04 / IN-05 of ADR `2026-05-27-1601`).
        let (catalogue, catalogue_hash_hex, entry_hashes) =
            match self.reader.read_catalogue_for_spec_ref_check(
                &cmd.branch,
                cmd.track_id.as_ref(),
                cmd.layer_id.as_ref(),
            ) {
                BlobFetchResult::Found(triple) => triple,
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
        // T024: v3-native — iterate types then traits then functions (BTreeMap order,
        // matching the signal refresher entry ordering) via the shared
        // `iter_catalogue_entries` helper (CN-04 / catalogue_traversal.rs).
        // `entry_hashes` supplies the per-entry SHA-256, computed in infrastructure.
        // Fail-closed: a missing hash means the adapter's key-derivation has drifted
        // from the catalogue entries. Persisting a fabricated zero hash would produce
        // corrupted integrity metadata, so we return an error instead.
        //
        // Hash lookup uses `e.section_key` ("types:Foo", "traits:Foo", …) rather than
        // the bare `e.key` so that a type and a trait sharing the same short name map
        // to distinct hash slots.  The bare `e.key` is still passed to
        // `CatalogueSpecSignal::new` so that `type_name` in the signals document
        // continues to store the human-readable short name expected by the merge gate.
        let signals: Vec<CatalogueSpecSignal> = iter_catalogue_entries(&catalogue)
            .map(|e| {
                let signal =
                    evaluate_catalogue_entry_signal(e.action, e.spec_refs, e.informal_grounds);
                let entry_hash =
                    entry_hashes.get(e.section_key.as_str()).cloned().ok_or_else(|| {
                        RefreshCatalogueSpecSignalsError::MissingEntryHash {
                            layer_id: cmd.layer_id.as_ref().to_owned(),
                            entry_key: e.section_key.as_str().to_owned(),
                        }
                    })?;
                Ok(CatalogueSpecSignal::new(e.key, signal, entry_hash))
            })
            .collect::<Result<Vec<_>, _>>()?;

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
    use std::collections::HashMap;
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
    use domain::tddd::catalogue_v2::CatalogueDocument;
    use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, TypeName};
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
    use domain::{ImplPlanDocument, InformalGroundKind, InformalGroundRef, InformalGroundSummary};

    /// Builds a `HashMap<String, ContentHash>` from a list of entry keys,
    /// assigning a fixed deterministic hash (`[0xde; 32]`) to each.
    /// Used in tests that need non-empty `entry_hashes` but do not verify the
    /// actual hash values of individual signals.
    fn make_entry_hashes(keys: &[&str]) -> HashMap<String, ContentHash> {
        keys.iter().map(|k| ((*k).to_owned(), ContentHash::from_bytes([0xde; 32]))).collect()
    }

    /// Test-only mock reader that returns a fixed catalogue + raw-bytes hash +
    /// optional per-entry content hashes.
    struct FixedReader {
        catalogue: Mutex<Option<CatalogueDocument>>,
        hash_hex: String,
        entry_hashes: HashMap<String, ContentHash>,
        /// If true, `read_catalogue_for_spec_ref_check` returns `NotFound`.
        not_found: bool,
        /// If non-empty, `read_catalogue_for_spec_ref_check` returns `FetchError(...)`.
        fetch_error: Option<String>,
    }

    impl FixedReader {
        fn found(
            catalogue: CatalogueDocument,
            hash_hex: impl Into<String>,
            entry_hashes: HashMap<String, ContentHash>,
        ) -> Self {
            Self {
                catalogue: Mutex::new(Some(catalogue)),
                hash_hex: hash_hex.into(),
                entry_hashes,
                not_found: false,
                fetch_error: None,
            }
        }

        fn not_found() -> Self {
            Self {
                catalogue: Mutex::new(None),
                hash_hex: String::new(),
                entry_hashes: HashMap::new(),
                not_found: true,
                fetch_error: None,
            }
        }

        fn fetch_error(message: impl Into<String>) -> Self {
            Self {
                catalogue: Mutex::new(None),
                hash_hex: String::new(),
                entry_hashes: HashMap::new(),
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
        ) -> BlobFetchResult<(Vec<u8>, String)> {
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
        ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
            if self.not_found {
                return BlobFetchResult::NotFound;
            }
            if let Some(msg) = &self.fetch_error {
                return BlobFetchResult::FetchError(msg.clone());
            }
            match self.catalogue.lock().unwrap().take() {
                Some(doc) => {
                    BlobFetchResult::Found((doc, self.hash_hex.clone(), self.entry_hashes.clone()))
                }
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

    /// Build a v3 `CatalogueDocument` with one type entry under the given name.
    fn catalogue_with_one_type(
        name: &str,
        action: ItemAction,
        spec_refs: Vec<domain::SpecRef>,
        informal_grounds: Vec<InformalGroundRef>,
    ) -> CatalogueDocument {
        let crate_name = CrateName::new("domain").unwrap();
        let layer = domain::tddd::LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        let entry = TypeEntry {
            action,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

            module_path: ModulePath::root(),
            docs: None,
            spec_refs,
            informal_grounds,
        };
        doc.types.insert(TypeName::new(name).unwrap(), entry);
        doc
    }

    fn catalogue_entry_with_action(
        name: &str,
        action: ItemAction,
        spec_refs: Vec<domain::SpecRef>,
        informal_grounds: Vec<InformalGroundRef>,
    ) -> (CatalogueDocument, String) {
        let doc = catalogue_with_one_type(name, action, spec_refs, informal_grounds);
        (doc, name.to_owned())
    }

    /// Build a v3 `CatalogueDocument` with multiple named type entries.
    /// Each `(name, action, spec_refs, informal_grounds)` becomes one `TypeEntry`.
    fn catalogue_with_types(
        entries: Vec<(&str, ItemAction, Vec<domain::SpecRef>, Vec<InformalGroundRef>)>,
    ) -> CatalogueDocument {
        let crate_name = CrateName::new("domain").unwrap();
        let layer = domain::tddd::LayerId::try_new("domain").unwrap();
        let mut doc = CatalogueDocument::new(3, crate_name, layer);
        for (name, action, spec_refs, informal_grounds) in entries {
            let entry = TypeEntry {
                action,
                role: DataRole::ValueObject,
                kind: TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain { fields: vec![], has_stripped_fields: false },
                    None,
                )),
                methods: vec![],

                module_path: ModulePath::root(),
                docs: None,
                spec_refs,
                informal_grounds,
            };
            doc.types.insert(TypeName::new(name).unwrap(), entry);
        }
        doc
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
        // BTreeMap sorts keys alphabetically: TypeA < TypeB, so ordering is preserved.
        let cat = catalogue_with_types(vec![
            ("TypeA", ItemAction::Add, Vec::new(), Vec::new()), // → Red (no spec_refs, no informal)
            (
                "TypeB",
                ItemAction::Add,
                Vec::new(),
                vec![
                    // → Yellow
                    InformalGroundRef::new(
                        InformalGroundKind::UserDirective,
                        InformalGroundSummary::try_new("test ground").unwrap(),
                    ),
                ],
            ),
        ]);
        let hash_hex = ContentHash::from_bytes([0xab; 32]).to_hex();
        // Section-qualified keys match the contract from `iter_catalogue_entries`.
        let entry_hashes = make_entry_hashes(&["types:TypeA", "types:TypeB"]);
        let reader = FixedReader::found(cat, hash_hex.clone(), entry_hashes);
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
        let cat = catalogue_with_types(vec![]);
        let reader = FixedReader::found(cat, "not-a-valid-64char-hex", HashMap::new());
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
        let cat = catalogue_with_types(vec![("X", ItemAction::Add, Vec::new(), Vec::new())]);
        let entry_hashes = make_entry_hashes(&["types:X"]);
        let reader =
            FixedReader::found(cat, ContentHash::from_bytes([0x00; 32]).to_hex(), entry_hashes);
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

    /// An `ItemAction::Reference` entry with empty `spec_refs` and empty
    /// `informal_grounds` must evaluate to `Blue` (ADR
    /// `2026-05-11-1257-tddd-v2-catalogue-spec-link-restoration.md` D5).
    /// This test verifies the action-threading path for the `Reference` variant
    /// specifically — a regression in `entry.action` access or the
    /// `evaluate_catalogue_entry_signal` call site would produce `Red` instead of `Blue`.
    #[test]
    fn refresh_reference_action_with_no_spec_refs_or_informal_evaluates_to_blue() {
        let (cat, _) = catalogue_entry_with_action(
            "ExternalType",
            ItemAction::Reference,
            Vec::new(),
            Vec::new(),
        );
        let entry_hashes = make_entry_hashes(&["types:ExternalType"]);
        let reader =
            FixedReader::found(cat, ContentHash::from_bytes([0xcc; 32]).to_hex(), entry_hashes);
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        interactor.execute(&refresh_cmd("track/my-track", "my-track", "domain")).unwrap();

        let (_, _, doc) = interactor.writer.single();
        assert_eq!(doc.signals.len(), 1);
        assert_eq!(doc.signals[0].type_name, "ExternalType");
        assert_eq!(
            doc.signals[0].signal,
            domain::ConfidenceSignal::Blue,
            "Reference action with empty spec_refs + informal_grounds must be Blue (D5 exemption)"
        );
    }

    /// When `entry_hashes` does not contain a key that exists in the catalogue,
    /// the interactor must return `MissingEntryHash` rather than silently
    /// fabricating a zero hash. This prevents corrupted integrity metadata from
    /// being persisted when the infrastructure adapter's key-derivation drifts.
    #[test]
    fn refresh_returns_missing_entry_hash_error_when_adapter_omits_key() {
        let cat = catalogue_with_types(vec![(
            "MissingHashType",
            ItemAction::Add,
            Vec::new(),
            Vec::new(),
        )]);
        // Intentionally supply an empty map — simulates an adapter omitting the key.
        let reader =
            FixedReader::found(cat, ContentHash::from_bytes([0x01; 32]).to_hex(), HashMap::new());
        let writer = CapturingWriter::new();
        let interactor = RefreshCatalogueSpecSignalsInteractor::new(reader, writer);

        let err =
            interactor.execute(&refresh_cmd("track/my-track", "my-track", "domain")).unwrap_err();

        assert!(
            matches!(
                err,
                RefreshCatalogueSpecSignalsError::MissingEntryHash {
                    ref layer_id,
                    ref entry_key
                } if layer_id == "domain" && entry_key == "types:MissingHashType"
            ),
            "expected MissingEntryHash for 'types:MissingHashType', got: {err:?}"
        );
        assert_eq!(interactor.writer.call_count(), 0, "no write must occur when a hash is missing");
    }
}
