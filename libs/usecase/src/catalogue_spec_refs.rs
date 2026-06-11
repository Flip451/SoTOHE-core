//! Catalogue ↔ spec integrity verification (SoT Chain ② binary gate).
//!
//! Implements the `sotp verify catalogue-spec-refs` usecase via
//! [`VerifyCatalogueSpecRefsInteractor`]. Composes the T006
//! [`TrackBlobReader`] extension with a new [`SpecElementHashReader`]
//! secondary port that supplies canonical SHA-256 digests per spec element.
//! Ref integrity check logic is inlined in `execute` (T024 catalogue-native migration):
//! iterates `CatalogueDocument` types/traits/functions BTreeMaps and emits
//! `SpecRefFinding` for dangling spec anchors; stale signal findings are
//! appended separately.
//!
//! ADR reference: `2026-04-23-0344-catalogue-spec-signal-activation.md`
//! §D1.5 / §D3.2 / §D3.6 / IN-06.

use std::collections::BTreeMap;

use domain::tddd::LayerId;
use domain::{
    ContentHash, SpecElementId, SpecRef, SpecRefFinding, SpecRefFindingKind, TrackId,
    ValidationError,
};
use thiserror::Error;

use crate::catalogue_traversal::iter_catalogue_entries;
use crate::merge_gate::{BlobFetchResult, TrackBlobReader};

/// Secondary port returning the canonical SHA-256 digest of every spec
/// element in a track's `spec.json`, keyed by [`SpecElementId`].
///
/// The canonical-JSON serialization lives in infrastructure (it reuses the
/// `sotp verify plan-artifact-refs` codec). Splitting this into its own
/// trait keeps the T009 interactor pure (no sha2 / serde_json in usecase)
/// while letting the domain `check_catalogue_spec_ref_integrity` receive
/// pre-computed hashes via a `BTreeMap`.
///
/// Implementations are registered alongside [`TrackBlobReader`] in the
/// infrastructure composition root. A real implementation is authored in
/// T011 atop the existing `canonical_json_sha256` helper.
pub trait SpecElementHashReader {
    /// Reads `track/items/<track_id>/spec.json` on the given branch and
    /// returns the SHA-256 of each requirement's canonical JSON subtree.
    ///
    /// `Found(map)` carries one entry per requirement id (goal, constraints,
    /// acceptance_criteria, scope.in_scope, scope.out_of_scope). `NotFound`
    /// mirrors `TrackBlobReader::read_spec_document` and means the
    /// `spec.json` file does not exist on the target ref. `FetchError`
    /// carries an adapter-level description.
    fn read_spec_element_hashes(
        &self,
        branch: &str,
        track_id: &str,
    ) -> BlobFetchResult<BTreeMap<SpecElementId, ContentHash>>;
}

/// Command input for [`VerifyCatalogueSpecRefs::execute`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyCatalogueSpecRefsCommand {
    /// Current git branch.
    pub branch: String,
    /// Track identifier the verification is scoped to.
    pub track_id: TrackId,
    /// Layer whose catalogue is being verified.
    pub layer_id: LayerId,
    /// When `true`, skip the [`SpecRefFindingKind::StaleSignals`] check
    /// (used in the pre-commit path where the signals file is regenerated
    /// immediately after this step).
    ///
    /// [`SpecRefFindingKind::StaleSignals`]: domain::SpecRefFindingKind::StaleSignals
    pub skip_stale: bool,
}

/// Failure modes of [`VerifyCatalogueSpecRefsInteractor::execute`].
#[derive(Debug, Error)]
pub enum VerifyCatalogueSpecRefsError {
    /// Branch does not start with `track/` — CN-07 guard rejects.
    #[error("verify rejected: branch '{branch}' is not an active track branch (CN-07)")]
    NonActiveTrack { branch: String },

    /// Branch `track/<suffix>` disagrees with `cmd.track_id`.
    #[error(
        "verify rejected: branch '{branch}' does not match track_id '{track_id}' (expected 'track/{track_id}')"
    )]
    BranchTrackMismatch { branch: String, track_id: String },

    /// `<layer>-types.json` not present on the target branch.
    #[error("catalogue not found for layer '{layer_id}' on branch '{branch}'")]
    CatalogueNotFound { branch: String, layer_id: String },

    /// Catalogue fetch failed at the blob-fetch port.
    #[error("catalogue fetch error for layer '{layer_id}': {message}")]
    CatalogueFetchError { layer_id: String, message: String },

    /// Raw-bytes hash from the port was not valid canonical hex.
    #[error(
        "catalogue declaration hash for layer '{layer_id}' is not valid canonical SHA-256 hex: {source}"
    )]
    InvalidCatalogueHash {
        layer_id: String,
        #[source]
        source: ValidationError,
    },

    /// `spec.json` not present on the target branch.
    #[error("spec.json not found on branch '{branch}'")]
    SpecNotFound { branch: String },

    /// Spec fetch failed at the blob-fetch port.
    #[error("spec fetch error: {message}")]
    SpecFetchError { message: String },

    /// Signals fetch failed at the blob-fetch port.
    #[error("signals fetch error for layer '{layer_id}': {message}")]
    SignalsFetchError { layer_id: String, message: String },
}

/// Primary port for the "verify catalogue-spec refs" use case.
pub trait VerifyCatalogueSpecRefs: Send + Sync {
    /// Runs the 3-step verification pipeline (ADR §D1.5 / §D3.2):
    ///
    /// 1. Active-track guard (CN-07): reject non-`track/` branch / branch
    ///    track_id mismatch.
    /// 2. Read catalogue + raw-bytes hash (T006 port); optionally read
    ///    spec + element hashes + signals (skipped when `cmd.skip_stale`).
    /// 3. Inline spec-ref integrity check over `CatalogueDocument` and return
    ///    the resulting `Vec<SpecRefFinding>` (empty = pass).
    ///
    /// # Errors
    /// Returns [`VerifyCatalogueSpecRefsError`] on guard / port failures.
    /// Integrity violations are reported as `Vec<SpecRefFinding>` (Ok),
    /// not as errors — the CLI maps findings to exit codes downstream.
    fn execute(
        &self,
        cmd: &VerifyCatalogueSpecRefsCommand,
    ) -> Result<Vec<SpecRefFinding>, VerifyCatalogueSpecRefsError>;
}

/// Default [`VerifyCatalogueSpecRefs`] implementation wiring
/// [`TrackBlobReader`] + [`SpecElementHashReader`] into the 3-step pipeline.
pub struct VerifyCatalogueSpecRefsInteractor<R, H>
where
    R: TrackBlobReader,
    H: SpecElementHashReader,
{
    reader: R,
    hash_reader: H,
}

impl<R, H> VerifyCatalogueSpecRefsInteractor<R, H>
where
    R: TrackBlobReader,
    H: SpecElementHashReader,
{
    /// Creates a new interactor wrapping the supplied secondary ports.
    #[must_use]
    pub fn new(reader: R, hash_reader: H) -> Self {
        Self { reader, hash_reader }
    }
}

impl<R, H> VerifyCatalogueSpecRefs for VerifyCatalogueSpecRefsInteractor<R, H>
where
    R: TrackBlobReader + Send + Sync,
    H: SpecElementHashReader + Send + Sync,
{
    fn execute(
        &self,
        cmd: &VerifyCatalogueSpecRefsCommand,
    ) -> Result<Vec<SpecRefFinding>, VerifyCatalogueSpecRefsError> {
        // 1. Active-track guard.
        let suffix = cmd.branch.strip_prefix("track/").ok_or_else(|| {
            VerifyCatalogueSpecRefsError::NonActiveTrack { branch: cmd.branch.clone() }
        })?;
        if suffix != cmd.track_id.as_ref() {
            return Err(VerifyCatalogueSpecRefsError::BranchTrackMismatch {
                branch: cmd.branch.clone(),
                track_id: cmd.track_id.as_ref().to_owned(),
            });
        }

        // 2a. Catalogue + raw-bytes hash + per-entry hashes.
        // The third element (entry_hashes) is not used in this verify path
        // (it is used by the signal-refresh path in catalogue_spec_signals.rs).
        let (catalogue, catalogue_hash_hex, _entry_hashes) =
            match self.reader.read_catalogue_for_spec_ref_check(
                &cmd.branch,
                cmd.track_id.as_ref(),
                cmd.layer_id.as_ref(),
            ) {
                BlobFetchResult::Found(triple) => triple,
                BlobFetchResult::NotFound => {
                    return Err(VerifyCatalogueSpecRefsError::CatalogueNotFound {
                        branch: cmd.branch.clone(),
                        layer_id: cmd.layer_id.as_ref().to_owned(),
                    });
                }
                BlobFetchResult::FetchError(message) => {
                    return Err(VerifyCatalogueSpecRefsError::CatalogueFetchError {
                        layer_id: cmd.layer_id.as_ref().to_owned(),
                        message,
                    });
                }
            };
        // Validate the hash immediately: an invalid hex string from the port
        // indicates a port contract violation and is always an error, regardless
        // of whether stale detection will run.  Failing early avoids silently
        // accepting a corrupt hash from the adapter.
        let catalogue_hash = ContentHash::try_from_hex(&catalogue_hash_hex).map_err(|source| {
            VerifyCatalogueSpecRefsError::InvalidCatalogueHash {
                layer_id: cmd.layer_id.as_ref().to_owned(),
                source,
            }
        })?;

        // 2b. Spec element hashes.
        let spec_element_hashes =
            match self.hash_reader.read_spec_element_hashes(&cmd.branch, cmd.track_id.as_ref()) {
                BlobFetchResult::Found(map) => map,
                BlobFetchResult::NotFound => {
                    return Err(VerifyCatalogueSpecRefsError::SpecNotFound {
                        branch: cmd.branch.clone(),
                    });
                }
                BlobFetchResult::FetchError(message) => {
                    return Err(VerifyCatalogueSpecRefsError::SpecFetchError { message });
                }
            };

        // 2c. Optional: signals document (skipped when skip_stale).
        let (current_hash_opt, signals_opt) = if cmd.skip_stale {
            (None, None)
        } else {
            match self.reader.read_catalogue_spec_signals_document(
                &cmd.branch,
                cmd.track_id.as_ref(),
                cmd.layer_id.as_ref(),
            ) {
                BlobFetchResult::Found(signals) => (Some(catalogue_hash.clone()), Some(signals)),
                BlobFetchResult::NotFound => {
                    // No signals file yet — treat as "nothing to compare against"
                    // rather than an error. Stale detection is a no-op in this
                    // case; dangling-anchor checks still run.
                    (None, None)
                }
                BlobFetchResult::FetchError(message) => {
                    return Err(VerifyCatalogueSpecRefsError::SignalsFetchError {
                        layer_id: cmd.layer_id.as_ref().to_owned(),
                        message,
                    });
                }
            }
        };

        // 3. Inline spec-ref integrity check over `CatalogueDocument`.
        //
        // T024: equivalent of `check_catalogue_spec_ref_integrity` over the
        // catalogue-native `CatalogueDocument`. Iterates types → traits →
        // functions (BTreeMap order) and checks each entry's `spec_refs` for
        // DanglingAnchor. StaleSignals check appended last.
        let mut findings: Vec<SpecRefFinding> = Vec::new();

        // Helper closure: check spec_refs for one (name, spec_refs_slice) pair.
        // Hash verification is removed (spec-ref-embedded-hash-removal IN-04):
        // staleness is detected by verify-cache runtime recomputation.
        let mut check_entry_refs = |entry_name: String, spec_refs: &[SpecRef]| {
            for (ref_index, spec_ref) in spec_refs.iter().enumerate() {
                if !spec_element_hashes.contains_key(&spec_ref.anchor) {
                    findings.push(SpecRefFinding::new(
                        cmd.layer_id.clone(),
                        SpecRefFindingKind::DanglingAnchor {
                            catalogue_entry: entry_name.clone(),
                            ref_index,
                            spec_file: spec_ref.file.clone(),
                            anchor: spec_ref.anchor.clone(),
                        },
                    ));
                }
            }
        };

        for e in iter_catalogue_entries(&catalogue) {
            check_entry_refs(e.key, e.spec_refs);
        }

        // StaleSignals: compare signals_opt.catalogue_declaration_hash to current hash.
        if let (Some(current), Some(signals)) = (current_hash_opt.as_ref(), signals_opt.as_ref()) {
            if &signals.catalogue_declaration_hash != current {
                findings.push(SpecRefFinding::new(
                    cmd.layer_id.clone(),
                    SpecRefFindingKind::StaleSignals {
                        declared_catalogue_hash: signals.catalogue_declaration_hash.clone(),
                        actual_catalogue_hash: current.clone(),
                    },
                ));
            }
        }

        Ok(findings)
    }
}

#[cfg(test)]
#[allow(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::type_complexity
)]
mod tests {
    use std::collections::HashMap;
    use std::sync::Mutex;

    use domain::spec::SpecDocument;
    use domain::tddd::catalogue_v2::CatalogueDocument;
    use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::identifiers::{CrateName, ModulePath, TypeName};
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
    use domain::{
        CatalogueSpecSignal, CatalogueSpecSignalsDocument, ConfidenceSignal, ImplPlanDocument,
        SpecRef, SpecRefFindingKind,
    };

    use super::*;

    /// Build an empty schema-v4 `CatalogueDocument` with the given layer id.
    fn empty_catalogue(layer_id: &str) -> CatalogueDocument {
        let crate_name = CrateName::new(layer_id).unwrap();
        let layer = LayerId::try_new(layer_id).unwrap();
        CatalogueDocument::new(4, crate_name, layer)
    }

    /// Add a `TypeEntry` with the given name and spec_refs to a `CatalogueDocument`.
    fn add_type_entry(doc: &mut CatalogueDocument, name: &str, spec_refs: Vec<SpecRef>) {
        let entry = TypeEntry {
            action: ItemAction::Add,
            role: DataRole::ValueObject,
            kind: TypeKindV2::Struct(StructKind::new(
                StructShape::Plain { fields: vec![], has_stripped_fields: false },
                None,
            )),
            methods: vec![],

            module_path: ModulePath::root(),
            docs: None,
            spec_refs,
            informal_grounds: vec![],
        };
        doc.types.insert(TypeName::new(name).unwrap(), entry);
    }

    /// Consume the catalogue from a `Mutex<Option<...>>`, returning `Found` or panicking on
    /// double-consume. Shared by `FixedReader` and `PanicOnSignalsReader` to avoid duplicating
    /// the lock/take/Found pattern.
    fn take_catalogue(
        catalogue: &Mutex<Option<(CatalogueDocument, String, HashMap<String, ContentHash>)>>,
        reader_name: &str,
    ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
        match catalogue.lock().unwrap().take() {
            Some(triple) => BlobFetchResult::Found(triple),
            None => panic!("{reader_name} catalogue consumed twice"),
        }
    }

    struct FixedReader {
        catalogue: Mutex<Option<(CatalogueDocument, String, HashMap<String, ContentHash>)>>,
        signals: Mutex<Option<CatalogueSpecSignalsDocument>>,
        signals_not_found: bool,
    }

    impl FixedReader {
        fn new(
            catalogue: CatalogueDocument,
            hash_hex: impl Into<String>,
            signals: Option<CatalogueSpecSignalsDocument>,
        ) -> Self {
            let signals_not_found = signals.is_none();
            Self {
                catalogue: Mutex::new(Some((catalogue, hash_hex.into(), HashMap::new()))),
                signals: Mutex::new(signals),
                signals_not_found,
            }
        }
    }

    impl TrackBlobReader for FixedReader {
        fn read_spec_document(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<SpecDocument> {
            panic!("FixedReader::read_spec_document should not be called by T009 tests")
        }

        fn read_type_catalogue(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            panic!("FixedReader::read_type_catalogue should not be called by T009 tests")
        }

        fn read_impl_plan(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<ImplPlanDocument> {
            panic!("FixedReader::read_impl_plan should not be called by T009 tests")
        }

        fn read_catalogue_for_spec_ref_check(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
            take_catalogue(&self.catalogue, "FixedReader")
        }

        fn read_catalogue_spec_signals_document(
            &self,
            _branch: &str,
            _track_id: &str,
            _layer_id: &str,
        ) -> BlobFetchResult<CatalogueSpecSignalsDocument> {
            if self.signals_not_found {
                return BlobFetchResult::NotFound;
            }
            match self.signals.lock().unwrap().take() {
                Some(doc) => BlobFetchResult::Found(doc),
                None => panic!("FixedReader signals consumed twice"),
            }
        }
    }

    struct FixedHashReader {
        hashes: BTreeMap<SpecElementId, ContentHash>,
    }

    impl SpecElementHashReader for FixedHashReader {
        fn read_spec_element_hashes(
            &self,
            _branch: &str,
            _track_id: &str,
        ) -> BlobFetchResult<BTreeMap<SpecElementId, ContentHash>> {
            BlobFetchResult::Found(self.hashes.clone())
        }
    }

    /// Reader that returns `NotFound` from `read_catalogue_for_spec_ref_check`.
    struct CatalogueNotFoundReader;

    impl TrackBlobReader for CatalogueNotFoundReader {
        fn read_spec_document(&self, _: &str, _: &str) -> BlobFetchResult<SpecDocument> {
            panic!("should not be called")
        }

        fn read_type_catalogue(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            panic!("should not be called")
        }

        fn read_impl_plan(&self, _: &str, _: &str) -> BlobFetchResult<ImplPlanDocument> {
            panic!("should not be called")
        }

        fn read_catalogue_for_spec_ref_check(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
            BlobFetchResult::NotFound
        }

        fn read_catalogue_spec_signals_document(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<CatalogueSpecSignalsDocument> {
            panic!("should not be called")
        }
    }

    /// Reader that returns `FetchError` from `read_catalogue_for_spec_ref_check`.
    struct CatalogueFetchErrorReader;

    impl TrackBlobReader for CatalogueFetchErrorReader {
        fn read_spec_document(&self, _: &str, _: &str) -> BlobFetchResult<SpecDocument> {
            panic!("should not be called")
        }

        fn read_type_catalogue(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            panic!("should not be called")
        }

        fn read_impl_plan(&self, _: &str, _: &str) -> BlobFetchResult<ImplPlanDocument> {
            panic!("should not be called")
        }

        fn read_catalogue_for_spec_ref_check(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
            BlobFetchResult::FetchError("git object not found".to_owned())
        }

        fn read_catalogue_spec_signals_document(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<CatalogueSpecSignalsDocument> {
            panic!("should not be called")
        }
    }

    /// Hash reader that returns `NotFound`.
    struct SpecHashNotFoundReader;

    impl SpecElementHashReader for SpecHashNotFoundReader {
        fn read_spec_element_hashes(
            &self,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<BTreeMap<SpecElementId, ContentHash>> {
            BlobFetchResult::NotFound
        }
    }

    /// Hash reader that returns `FetchError`.
    struct SpecHashFetchErrorReader;

    impl SpecElementHashReader for SpecHashFetchErrorReader {
        fn read_spec_element_hashes(
            &self,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<BTreeMap<SpecElementId, ContentHash>> {
            BlobFetchResult::FetchError("spec.json decode error".to_owned())
        }
    }

    /// Reader that returns a valid catalogue but panics if signals are ever accessed.
    /// Used to verify that `skip_stale=true` skips the signals port entirely.
    struct PanicOnSignalsReader {
        catalogue: Mutex<Option<(CatalogueDocument, String, HashMap<String, ContentHash>)>>,
    }

    impl PanicOnSignalsReader {
        fn new(catalogue: CatalogueDocument, hash_hex: impl Into<String>) -> Self {
            Self { catalogue: Mutex::new(Some((catalogue, hash_hex.into(), HashMap::new()))) }
        }
    }

    impl TrackBlobReader for PanicOnSignalsReader {
        fn read_spec_document(&self, _: &str, _: &str) -> BlobFetchResult<SpecDocument> {
            panic!("should not be called")
        }

        fn read_type_catalogue(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            panic!("should not be called")
        }

        fn read_impl_plan(&self, _: &str, _: &str) -> BlobFetchResult<ImplPlanDocument> {
            panic!("should not be called")
        }

        fn read_catalogue_for_spec_ref_check(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
            take_catalogue(&self.catalogue, "PanicOnSignalsReader")
        }

        fn read_catalogue_spec_signals_document(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<CatalogueSpecSignalsDocument> {
            panic!("read_catalogue_spec_signals_document called despite skip_stale=true")
        }
    }

    /// Reader that returns a valid catalogue but `FetchError` from the signals fetch.
    struct SignalsFetchErrorReader {
        catalogue: Mutex<Option<(CatalogueDocument, String, HashMap<String, ContentHash>)>>,
    }

    impl SignalsFetchErrorReader {
        fn new(catalogue: CatalogueDocument, hash_hex: impl Into<String>) -> Self {
            Self { catalogue: Mutex::new(Some((catalogue, hash_hex.into(), HashMap::new()))) }
        }
    }

    impl TrackBlobReader for SignalsFetchErrorReader {
        fn read_spec_document(&self, _: &str, _: &str) -> BlobFetchResult<SpecDocument> {
            panic!("should not be called")
        }

        fn read_type_catalogue(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(Vec<u8>, String)> {
            panic!("should not be called")
        }

        fn read_impl_plan(&self, _: &str, _: &str) -> BlobFetchResult<ImplPlanDocument> {
            panic!("should not be called")
        }

        fn read_catalogue_for_spec_ref_check(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<(CatalogueDocument, String, HashMap<String, ContentHash>)> {
            take_catalogue(&self.catalogue, "SignalsFetchErrorReader")
        }

        fn read_catalogue_spec_signals_document(
            &self,
            _: &str,
            _: &str,
            _: &str,
        ) -> BlobFetchResult<CatalogueSpecSignalsDocument> {
            BlobFetchResult::FetchError("signals file corrupted".to_owned())
        }
    }

    fn anchor(id: &str) -> SpecElementId {
        SpecElementId::try_new(id).unwrap()
    }

    fn hash(byte: u8) -> ContentHash {
        ContentHash::from_bytes([byte; 32])
    }

    fn hex_pattern(byte: u8) -> String {
        let mut s = String::with_capacity(64);
        for _ in 0..32 {
            s.push_str(&format!("{byte:02x}"));
        }
        s
    }

    /// Build a schema-v4 `CatalogueDocument` with one type entry that has a single spec_ref.
    fn catalogue_with_ref(name: &str, anchor_id: &str) -> CatalogueDocument {
        let spec_refs = vec![SpecRef::new("track/items/x/spec.json", anchor(anchor_id))];
        let mut doc = empty_catalogue("domain");
        add_type_entry(&mut doc, name, spec_refs);
        doc
    }

    fn cmd(
        branch: &str,
        track_id: &str,
        layer_id: &str,
        skip_stale: bool,
    ) -> VerifyCatalogueSpecRefsCommand {
        VerifyCatalogueSpecRefsCommand {
            branch: branch.to_owned(),
            track_id: TrackId::try_new(track_id).unwrap(),
            layer_id: LayerId::try_new(layer_id).unwrap(),
            skip_stale,
        }
    }

    #[test]
    fn verify_returns_empty_when_everything_aligns() {
        let cat = catalogue_with_ref("X", "IN-01");
        let mut hashes = BTreeMap::new();
        hashes.insert(anchor("IN-01"), hash(0xab));
        let signals = CatalogueSpecSignalsDocument::new(
            ContentHash::try_from_hex(hex_pattern(0xcd)).unwrap(),
            vec![],
        );
        let reader = FixedReader::new(cat, hex_pattern(0xcd), Some(signals));
        let interactor = VerifyCatalogueSpecRefsInteractor::new(reader, FixedHashReader { hashes });

        let findings =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap();
        assert!(findings.is_empty());
    }

    #[test]
    fn verify_reports_dangling_anchor() {
        let cat = catalogue_with_ref("X", "IN-99");
        let hashes = BTreeMap::new(); // empty — anchor missing
        let reader = FixedReader::new(cat, hex_pattern(0xcd), None);
        let interactor = VerifyCatalogueSpecRefsInteractor::new(reader, FixedHashReader { hashes });

        let findings =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0].kind, SpecRefFindingKind::DanglingAnchor { .. }));
    }

    #[test]
    fn verify_reports_stale_signals_when_signals_hash_differs() {
        let cat = empty_catalogue("domain");
        let hashes = BTreeMap::new();
        let signals = CatalogueSpecSignalsDocument::new(
            ContentHash::try_from_hex(hex_pattern(0x00)).unwrap(),
            vec![CatalogueSpecSignal::new(
                "X",
                ConfidenceSignal::Blue,
                ContentHash::from_bytes([0u8; 32]),
            )],
        );
        let reader = FixedReader::new(cat, hex_pattern(0xff), Some(signals));
        let interactor = VerifyCatalogueSpecRefsInteractor::new(reader, FixedHashReader { hashes });

        let findings =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap();
        assert_eq!(findings.len(), 1);
        assert!(matches!(findings[0].kind, SpecRefFindingKind::StaleSignals { .. }));
    }

    #[test]
    fn verify_skip_stale_bypasses_signals_read() {
        let cat = catalogue_with_ref("X", "IN-01");
        let mut hashes = BTreeMap::new();
        hashes.insert(anchor("IN-01"), hash(0xab));
        // PanicOnSignalsReader panics if the signals port is accessed — proving
        // that skip_stale=true skips the signals read entirely (not just treats
        // NotFound as a no-op).
        let reader = PanicOnSignalsReader::new(cat, hex_pattern(0xcd));
        let interactor = VerifyCatalogueSpecRefsInteractor::new(reader, FixedHashReader { hashes });

        let findings =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", true)).unwrap();
        assert!(findings.is_empty(), "skip_stale=true + everything-aligned should be empty");
    }

    #[test]
    fn verify_rejects_non_track_branch() {
        let cat = empty_catalogue("domain");
        let reader = FixedReader::new(cat, hex_pattern(0x00), None);
        let interactor = VerifyCatalogueSpecRefsInteractor::new(
            reader,
            FixedHashReader { hashes: BTreeMap::new() },
        );

        let err = interactor.execute(&cmd("main", "my-track", "domain", false)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::NonActiveTrack { .. }));
    }

    #[test]
    fn verify_rejects_branch_track_id_mismatch() {
        let cat = empty_catalogue("domain");
        let reader = FixedReader::new(cat, hex_pattern(0x00), None);
        let interactor = VerifyCatalogueSpecRefsInteractor::new(
            reader,
            FixedHashReader { hashes: BTreeMap::new() },
        );

        let err = interactor.execute(&cmd("track/other", "my-track", "domain", false)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::BranchTrackMismatch { .. }));
    }

    #[test]
    fn verify_rejects_invalid_catalogue_hash() {
        // An invalid hex hash from the port is always an error (port contract
        // violation), even when skip_stale=true and the hash would not be
        // consumed for stale detection.  The test uses skip_stale=true to confirm
        // that the check runs unconditionally.
        let cat = empty_catalogue("domain");
        let reader = FixedReader::new(cat, "not-hex", None);
        let interactor = VerifyCatalogueSpecRefsInteractor::new(
            reader,
            FixedHashReader { hashes: BTreeMap::new() },
        );

        let err =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", true)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::InvalidCatalogueHash { .. }));
    }

    #[test]
    fn verify_rejects_catalogue_not_found() {
        let interactor = VerifyCatalogueSpecRefsInteractor::new(
            CatalogueNotFoundReader,
            FixedHashReader { hashes: BTreeMap::new() },
        );

        let err =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::CatalogueNotFound { .. }));
    }

    #[test]
    fn verify_rejects_catalogue_fetch_error() {
        let interactor = VerifyCatalogueSpecRefsInteractor::new(
            CatalogueFetchErrorReader,
            FixedHashReader { hashes: BTreeMap::new() },
        );

        let err =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::CatalogueFetchError { .. }));
    }

    #[test]
    fn verify_rejects_spec_not_found() {
        let cat = empty_catalogue("domain");
        let reader = FixedReader::new(cat, hex_pattern(0xcd), None);
        // Catalogue is found; hash_reader returns NotFound.
        let interactor = VerifyCatalogueSpecRefsInteractor::new(reader, SpecHashNotFoundReader);

        let err =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::SpecNotFound { .. }));
    }

    #[test]
    fn verify_rejects_spec_fetch_error() {
        let cat = empty_catalogue("domain");
        let reader = FixedReader::new(cat, hex_pattern(0xcd), None);
        // Catalogue is found; hash_reader returns FetchError.
        let interactor = VerifyCatalogueSpecRefsInteractor::new(reader, SpecHashFetchErrorReader);

        let err =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::SpecFetchError { .. }));
    }

    #[test]
    fn verify_propagates_signals_fetch_error() {
        let cat = empty_catalogue("domain");
        let reader = SignalsFetchErrorReader::new(cat, hex_pattern(0xcd));
        let interactor = VerifyCatalogueSpecRefsInteractor::new(
            reader,
            FixedHashReader { hashes: BTreeMap::new() },
        );

        // skip_stale=false so signals are read; FetchError → SignalsFetchError
        let err =
            interactor.execute(&cmd("track/my-track", "my-track", "domain", false)).unwrap_err();
        assert!(matches!(err, VerifyCatalogueSpecRefsError::SignalsFetchError { .. }));
    }
}
