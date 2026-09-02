//! Error-path tests for `CatalogueImplSignalsInteractor`.
//!
//! Split from `interactor.rs` to keep the production-code file under 400 lines.
//! Loaded via `#[cfg(test)] #[path = "interactor_tests.rs"] mod tests;` in
//! `interactor.rs`.
//!
//! Happy-path / report-format tests live in `interactor_happy_tests.rs`, which
//! is included as a submodule below so it can share these helpers.

#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::type_complexity
)]

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::{Arc, Mutex};

use domain::tddd::catalogue_v2::{
    BaselineCaptureIoError, CatalogueDocument, CatalogueDocumentLoaderError,
    CatalogueItemNamespace, CrateName, RustdocBaselineCapturePort, RustdocCratePort,
    RustdocCratePortError, TdddLayerBinding, TdddLayerBindingsError, TdddLayerBindingsPort,
    TypeRef,
};
use domain::tddd::extended_crate::ExtendedCrate;
use domain::tddd::signal_evaluator::phase1_error::Phase1Error;
use domain::tddd::signal_evaluator::port::SignalEvaluatorPort;
use domain::tddd::{
    AttestedRustdocSnapshot, AuthoritativeRustdocContext, CapturedRustdocJson, CargoFeatureName,
    ExpectedRustdocJsonPath, ImplementationFingerprint, LayerId, ResolvedCargoTargetDirectory,
    RustdocExecutionIdentity, Sha256Digest, TdddFeatureDeclaration,
    construct_attested_rustdoc_snapshot, construct_captured_rustdoc_json,
};
// ThreeWaySignal is not pub-re-exported from the parent module, so it cannot be
// reached via `use super::*` and must be imported explicitly here.
use domain::tddd::signal_evaluator::region::{ThreeWayEvaluationReport, ThreeWaySignal};
use domain::tddd::test_obligation::ids::DiagnosticMessage;
use domain::{FreeText, SymlinkGuardError, SymlinkGuardPort, TrackId};
use rustdoc_types::{
    Crate, ExternalCrate, FORMAT_VERSION, Generics, Id, Item, ItemEnum, ItemKind, ItemSummary,
    Module, Struct, Visibility,
};

use super::super::ports::{EvaluationStartCaptureError, EvaluationStartCapturePort};
use super::super::service::{CatalogueImplSignalsError, CatalogueImplSignalsService};
use super::CatalogueImplSignalsInteractor;
use crate::baseline_capture::{
    BaselineCaptureInteractor, BaselineCaptureRequest, BaselineCaptureService,
};
use crate::catalogue_document_loader::AttestedCatalogueDocumentLoaderPort;
use crate::tddd_feature_declaration::{
    TdddActualFeatureDeclarationPort, TdddActualFeatureDeclarationPortError,
    TdddBaselineFeatureDeclarationPort, TdddBaselineFeatureDeclarationPortError,
};

// -------------------------------------------------------------------------
// Test helpers — also re-used by `happy_tests`
// -------------------------------------------------------------------------

/// Build a minimal `rustdoc_types::Crate` with no items.
pub(super) fn empty_rustdoc_crate() -> Crate {
    Crate {
        root: rustdoc_types::Id(0),
        crate_version: None,
        includes_private: false,
        index: HashMap::new(),
        paths: HashMap::new(),
        external_crates: HashMap::new(),
        format_version: FORMAT_VERSION,
        target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
    }
}

fn decode_test_rustdoc(bytes: &[u8]) -> Result<Crate, RustdocCratePortError> {
    serde_json::from_slice(bytes).map_err(|error| RustdocCratePortError::ParseFailed {
        crate_name: CrateName::new("test").unwrap(),
        reason: FreeText::new(error.to_string()),
    })
}

fn captured_rustdoc(crate_data: Crate) -> CapturedRustdocJson {
    let bytes = serde_json::to_vec(&crate_data).unwrap();
    construct_captured_rustdoc_json(&bytes, decode_test_rustdoc).unwrap()
}

fn current_rustdoc(
    crate_name: &CrateName,
    crate_data: Crate,
    evaluation_start: &ImplementationFingerprint,
) -> AttestedRustdocSnapshot {
    let target = ResolvedCargoTargetDirectory::try_new(std::path::PathBuf::from(
        "/tmp/usecase-rustdoc-test-target",
    ))
    .unwrap();
    let expected = ExpectedRustdocJsonPath::try_new(
        target.as_path().join(format!("{}.json", crate_name.as_str())),
        &target,
    )
    .unwrap();
    let identity = RustdocExecutionIdentity::new(
        target,
        crate_name.clone(),
        vec![],
        domain::CargoProfileName::try_new("dev".to_owned()).unwrap(),
        expected,
    )
    .unwrap();
    let bytes = serde_json::to_vec(&crate_data).unwrap();
    construct_attested_rustdoc_snapshot(
        evaluation_start.clone(),
        identity,
        &bytes,
        decode_test_rustdoc,
    )
    .unwrap()
}

fn evaluation_start_fingerprint() -> ImplementationFingerprint {
    ImplementationFingerprint::new(Sha256Digest::try_new("a".repeat(64)).unwrap())
}

fn rustdoc_crate_with_gated_public_item() -> Crate {
    let root_id = Id(0);
    let item_id = Id(1);
    let item_name = "FeatureGatedPublicItem";
    let mut crate_ = empty_rustdoc_crate();
    crate_.index.insert(
        root_id,
        Item {
            id: root_id,
            crate_id: 0,
            name: Some("domain".to_owned()),
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Module(Module {
                is_crate: true,
                items: vec![item_id],
                is_stripped: false,
            }),
        },
    );
    crate_.index.insert(
        item_id,
        Item {
            id: item_id,
            crate_id: 0,
            name: Some(item_name.to_owned()),
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Struct(Struct {
                kind: rustdoc_types::StructKind::Plain {
                    fields: vec![],
                    has_stripped_fields: false,
                },
                generics: rustdoc_types::Generics { params: vec![], where_predicates: vec![] },
                impls: vec![],
            }),
        },
    );
    crate_.paths.insert(
        item_id,
        ItemSummary {
            crate_id: 0,
            path: vec!["domain".to_owned(), item_name.to_owned()],
            kind: ItemKind::Struct,
        },
    );
    crate_
}

fn rustdoc_crate_without_gated_public_item() -> Crate {
    let root_id = Id(0);
    let mut crate_ = empty_rustdoc_crate();
    crate_.index.insert(
        root_id,
        Item {
            id: root_id,
            crate_id: 0,
            name: Some("domain".to_owned()),
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Module(Module { is_crate: true, items: vec![], is_stripped: false }),
        },
    );
    crate_
}

pub(super) fn minimal_catalogue_doc(crate_name: &str) -> CatalogueDocument {
    use domain::tddd::catalogue_v2::CrateName;
    let layer = LayerId::try_new(crate_name).unwrap();
    let name = CrateName::new(crate_name).unwrap();
    CatalogueDocument::new(3, name, layer)
}

pub(super) fn stub_binding(layer_id: &str) -> TdddLayerBinding {
    TdddLayerBinding {
        layer_id: layer_id.to_owned(),
        catalogue_file: format!("{layer_id}-types.json"),
        baseline_file: format!("{layer_id}-types-baseline.json"),
        targets: vec![layer_id.to_owned()],
    }
}

fn stub_binding_with_target(layer_id: &str, target: &str) -> TdddLayerBinding {
    let mut binding = stub_binding(layer_id);
    binding.targets = vec![target.to_owned()];
    binding
}

// -------------------------------------------------------------------------
// Mock ports — also re-used by `happy_tests`
// -------------------------------------------------------------------------

pub(super) struct StubLoader {
    pub(super) doc: CatalogueDocument,
}

impl AttestedCatalogueDocumentLoaderPort for StubLoader {
    fn load(
        &self,
        _path: &Path,
    ) -> Result<domain::tddd::catalogue_v2::AttestedCatalogueDocument, CatalogueDocumentLoaderError>
    {
        Ok(domain::tddd::catalogue_v2::AttestedCatalogueDocument::attest(
            b"T014 test catalogue",
            |_| Ok::<_, std::convert::Infallible>(self.doc.clone()),
        )
        .unwrap())
    }
}

pub(super) struct FailingLoader;

impl AttestedCatalogueDocumentLoaderPort for FailingLoader {
    fn load(
        &self,
        path: &Path,
    ) -> Result<domain::tddd::catalogue_v2::AttestedCatalogueDocument, CatalogueDocumentLoaderError>
    {
        Err(CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })
    }
}

/// `CatalogueToExtendedCratePort` that always fails.
pub(super) struct FailingCodec;

impl domain::tddd::CatalogueToExtendedCratePort for FailingCodec {
    fn encode(
        &self,
        _target_layer: &LayerId,
        _track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        _rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        Err(domain::tddd::NewTypeGraphCodecError::InvalidTypeRef(
            TypeRef::new("stub").unwrap(),
            DiagnosticMessage::try_new("stub diagnostic".to_owned()).unwrap(),
        ))
    }
}

/// `SignalEvaluatorPort` that always returns an empty report.
pub(super) struct EmptyEvaluator;

impl SignalEvaluatorPort for EmptyEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        _b: Crate,
        _c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        Ok(ThreeWayEvaluationReport::new(vec![]))
    }
}

struct RecordingExtendedCrateEvaluator {
    observed: Arc<Mutex<Vec<ExtendedCrate>>>,
}

impl SignalEvaluatorPort for RecordingExtendedCrateEvaluator {
    fn evaluate(
        &self,
        catalogue: ExtendedCrate,
        _baseline: Crate,
        _current: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        self.observed.lock().unwrap().push(catalogue);
        Ok(ThreeWayEvaluationReport::new(vec![]))
    }
}

/// `SignalEvaluatorPort` that always returns a single Blue signal.
pub(super) struct SingleBlueEvaluator;

impl SignalEvaluatorPort for SingleBlueEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        _b: Crate,
        _c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        use domain::tddd::signal_evaluator::region::SignalRegion;
        let signal =
            ThreeWaySignal::label(FreeText::new("MyType"), SignalRegion::SIntersectC_Match_Add);
        Ok(ThreeWayEvaluationReport::new(vec![signal]))
    }
}

/// `SignalEvaluatorPort` that always returns a single Red signal
/// (used by `any_red = true` coverage tests).
pub(super) struct SingleRedEvaluator;

impl SignalEvaluatorPort for SingleRedEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        _b: Crate,
        _c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        use domain::tddd::signal_evaluator::region::SignalRegion;
        // `SMinusC_Reference` is a Red region — see signal_evaluator/region.rs.
        let signal =
            ThreeWaySignal::label(FreeText::new("RemovedType"), SignalRegion::SMinusC_Reference);
        Ok(ThreeWayEvaluationReport::new(vec![signal]))
    }
}

/// `SignalEvaluatorPort` that always returns an Evaluation failure.
pub(super) struct FailingEvaluator;

impl SignalEvaluatorPort for FailingEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        _b: Crate,
        _c: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        Err(Phase1Error::action_contradiction("stub contradiction"))
    }
}

/// `RustdocCratePort` that always panics — used in tests that stop before
/// the rustdoc ports are reached.
pub(super) struct NeverCalledRustdocPort;

impl RustdocCratePort for NeverCalledRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        panic!("NeverCalledRustdocPort::load_from_path must not be called in these tests")
    }

    fn capture_current(
        &self,
        _crate_name: &domain::tddd::catalogue_v2::CrateName,
        _features: &[CargoFeatureName],
        _evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        panic!("NeverCalledRustdocPort::capture_current must not be called in these tests")
    }
}

/// `RustdocCratePort` that returns empty rustdoc crates for load and capture.
pub(super) struct EmptyRustdocPort;

impl RustdocCratePort for EmptyRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(empty_rustdoc_crate()))
    }

    fn capture_current(
        &self,
        _crate_name: &domain::tddd::catalogue_v2::CrateName,
        _features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        Ok(current_rustdoc(_crate_name, empty_rustdoc_crate(), evaluation_start))
    }
}

/// `RustdocCratePort` that always returns `NotFound` for load and `CaptureFailed` for capture.
pub(super) struct FailingRustdocPort;

impl RustdocCratePort for FailingRustdocPort {
    fn load_from_path(&self, path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Err(RustdocCratePortError::NotFound { path: path.to_path_buf() })
    }

    fn capture_current(
        &self,
        crate_name: &domain::tddd::catalogue_v2::CrateName,
        _features: &[CargoFeatureName],
        _evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        Err(RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.clone(),
            reason: FreeText::new("stub capture failure"),
        })
    }
}

/// `RustdocCratePort` whose current capture reports the authoritative lock
/// failure unchanged after a readable baseline has been supplied.
struct CaptureFailureRustdocPort;

impl RustdocCratePort for CaptureFailureRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(empty_rustdoc_crate()))
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        _features: &[CargoFeatureName],
        _evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        Err(RustdocCratePortError::CaptureFailed {
            crate_name: crate_name.clone(),
            reason: FreeText::new("exclusive rustdoc lock sentinel failure"),
        })
    }
}

struct StartCaptureFailureRustdocPort {
    current_calls: Arc<Mutex<usize>>,
}

impl RustdocCratePort for StartCaptureFailureRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        panic!("a failed evaluation-start capture must stop before baseline loading")
    }

    fn capture_current(
        &self,
        _crate_name: &CrateName,
        _features: &[CargoFeatureName],
        _evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        *self.current_calls.lock().unwrap() += 1;
        panic!("a failed evaluation-start capture must stop before current capture")
    }
}

struct RecordingStartBindingRustdocPort {
    start_calls: Arc<Mutex<usize>>,
    captures: Arc<Mutex<Vec<ImplementationFingerprint>>>,
}

impl RustdocCratePort for RecordingStartBindingRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(empty_rustdoc_crate()))
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        _features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        self.captures.lock().unwrap().push(evaluation_start.clone());
        Ok(current_rustdoc(crate_name, empty_rustdoc_crate(), evaluation_start))
    }
}

impl EvaluationStartCapturePort for StartCaptureFailureRustdocPort {
    fn capture_evaluation_start(
        &self,
    ) -> Result<ImplementationFingerprint, EvaluationStartCaptureError> {
        Err(EvaluationStartCaptureError::AuthoritativeInput {
            reason: FreeText::new("stub evaluation-start fingerprint failure"),
        })
    }
}

impl EvaluationStartCapturePort for RecordingStartBindingRustdocPort {
    fn capture_evaluation_start(
        &self,
    ) -> Result<ImplementationFingerprint, EvaluationStartCaptureError> {
        *self.start_calls.lock().unwrap() += 1;
        Ok(evaluation_start_fingerprint())
    }
}

struct MixedGenerationRustdocPort {
    captures: Arc<Mutex<Vec<ImplementationFingerprint>>>,
}

impl RustdocCratePort for MixedGenerationRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(empty_rustdoc_crate()))
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        _features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        let mut captures = self.captures.lock().unwrap();
        captures.push(evaluation_start.clone());
        let fingerprint = if captures.len() == 1 {
            evaluation_start.clone()
        } else {
            ImplementationFingerprint::new(Sha256Digest::try_new("b".repeat(64)).unwrap())
        };
        Ok(current_rustdoc(crate_name, empty_rustdoc_crate(), &fingerprint))
    }
}

impl EvaluationStartCapturePort for MixedGenerationRustdocPort {
    fn capture_evaluation_start(
        &self,
    ) -> Result<ImplementationFingerprint, EvaluationStartCaptureError> {
        Ok(evaluation_start_fingerprint())
    }
}

pub(super) struct StubLayerBindings {
    pub(super) bindings: Vec<TdddLayerBinding>,
}

impl TdddLayerBindingsPort for StubLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Ok(self.bindings.clone())
    }
}

struct FilteringLayerBindings {
    bindings: Vec<TdddLayerBinding>,
    calls: Arc<Mutex<Vec<Option<String>>>>,
}

impl TdddLayerBindingsPort for FilteringLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        self.calls.lock().unwrap().push(layer_filter.map(str::to_owned));
        Ok(self
            .bindings
            .iter()
            .filter(|binding| layer_filter.is_none_or(|filter| binding.layer_id == filter))
            .cloned()
            .collect())
    }
}

pub(super) struct EmptyLayerBindings;

impl TdddLayerBindingsPort for EmptyLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Ok(vec![])
    }
}

pub(super) struct FailingLayerBindings;

impl TdddLayerBindingsPort for FailingLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Err(TdddLayerBindingsError::LoadFailed {
            reason: "architecture-rules.json not found".to_owned(),
        })
    }
}

pub(super) struct LayerNotFoundLayerBindings {
    pub(super) missing_layer_id: String,
}

impl TdddLayerBindingsPort for LayerNotFoundLayerBindings {
    fn load(
        &self,
        _workspace_root: &Path,
        _layer_filter: Option<&str>,
    ) -> Result<Vec<TdddLayerBinding>, TdddLayerBindingsError> {
        Err(TdddLayerBindingsError::LayerNotFound { layer_id: self.missing_layer_id.clone() })
    }
}

/// Actual-capture declaration stub that supplies no features for every requested layer.
pub(super) struct EmptyFeatureDeclaration;

impl TdddActualFeatureDeclarationPort for EmptyFeatureDeclaration {
    fn load_for_actual(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        let required_layers = layers
            .iter()
            .map(|binding| LayerId::try_new(binding.layer_id.clone()))
            .collect::<Result<Vec<_>, _>>()
            .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)?;
        let declared_layers = required_layers
            .iter()
            .cloned()
            .map(|layer| (layer, Vec::new()))
            .collect::<BTreeMap<_, _>>();
        TdddFeatureDeclaration::try_new(declared_layers, &required_layers)
            .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)
    }
}

struct FrozenFeatureDeclaration;

impl TdddBaselineFeatureDeclarationPort for FrozenFeatureDeclaration {
    fn load_for_baseline(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddBaselineFeatureDeclarationPortError> {
        frozen_feature_declaration(layers)
            .map_err(|_| TdddBaselineFeatureDeclarationPortError::BaselineSnapshotMismatch)
    }
}

impl TdddActualFeatureDeclarationPort for FrozenFeatureDeclaration {
    fn load_for_actual(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        frozen_feature_declaration(layers)
            .map_err(|_| TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)
    }
}

fn frozen_feature_declaration(layers: &[TdddLayerBinding]) -> Result<TdddFeatureDeclaration, ()> {
    let required_layers = layers
        .iter()
        .map(|binding| LayerId::try_new(binding.layer_id.clone()))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| ())?;
    let feature = CargoFeatureName::try_new("semantic-dup".to_owned()).map_err(|_| ())?;
    let declared_layers = required_layers
        .iter()
        .cloned()
        .map(|layer| (layer, vec![feature.clone()]))
        .collect::<BTreeMap<_, _>>();
    TdddFeatureDeclaration::try_new(declared_layers, &required_layers).map_err(|_| ())
}

struct MissingFrozenFeatureDeclaration;

impl TdddActualFeatureDeclarationPort for MissingFrozenFeatureDeclaration {
    fn load_for_actual(
        &self,
        track_dir: &Path,
        _workspace_root: &Path,
        _layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        Err(TdddActualFeatureDeclarationPortError::MissingBaselineSnapshot {
            path: track_dir.join("tddd-features-baseline.json"),
        })
    }
}

struct MismatchedFrozenFeatureDeclaration;

impl TdddActualFeatureDeclarationPort for MismatchedFrozenFeatureDeclaration {
    fn load_for_actual(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        _layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        Err(TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch)
    }
}

struct InvalidFeatureDeclaration;

impl TdddActualFeatureDeclarationPort for InvalidFeatureDeclaration {
    fn load_for_actual(
        &self,
        _track_dir: &Path,
        _workspace_root: &Path,
        _layers: &[TdddLayerBinding],
    ) -> Result<TdddFeatureDeclaration, TdddActualFeatureDeclarationPortError> {
        Err(TdddActualFeatureDeclarationPortError::Read(
            crate::tddd_feature_declaration::TdddFeatureDeclarationReadError::UnknownCargoFeature {
                layer: LayerId::try_new("domain".to_owned()).unwrap(),
                feature: CargoFeatureName::try_new("undeclared".to_owned()).unwrap(),
            },
        ))
    }
}

struct FeatureGatedRustdocPort {
    observed_actual_features: Arc<Mutex<Vec<Vec<String>>>>,
}

impl RustdocCratePort for FeatureGatedRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(rustdoc_crate_with_gated_public_item()))
    }

    fn capture_current(
        &self,
        _crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        self.observed_actual_features
            .lock()
            .unwrap()
            .push(features.iter().map(|feature| feature.as_str().to_owned()).collect());
        if !features.iter().any(|feature| feature.as_str() == "semantic-dup") {
            return Err(RustdocCratePortError::CaptureFailed {
                crate_name: CrateName::new("domain").unwrap(),
                reason: FreeText::new("feature-gated public item requires semantic-dup"),
            });
        }
        Ok(current_rustdoc(_crate_name, rustdoc_crate_with_gated_public_item(), evaluation_start))
    }
}

struct GatedPublicItemEvaluator {
    observed_surfaces: Arc<Mutex<Vec<(bool, bool)>>>,
}

impl SignalEvaluatorPort for GatedPublicItemEvaluator {
    fn evaluate(
        &self,
        _a: ExtendedCrate,
        baseline: Crate,
        current: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        let has_gated_item = |crate_: &Crate| {
            crate_.index.values().any(|item| item.name.as_deref() == Some("FeatureGatedPublicItem"))
        };
        self.observed_surfaces
            .lock()
            .unwrap()
            .push((has_gated_item(&baseline), has_gated_item(&current)));
        Ok(ThreeWayEvaluationReport::new(vec![]))
    }
}

struct RecordingBaselineCapture {
    observed_features: Arc<Mutex<Vec<Vec<String>>>>,
}

struct CatalogueGatedItemCodec;

impl domain::tddd::CatalogueToExtendedCratePort for CatalogueGatedItemCodec {
    fn encode(
        &self,
        _target_layer: &LayerId,
        _track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        _rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        Ok(ExtendedCrate::new(rustdoc_crate_with_gated_public_item(), BTreeMap::new()))
    }
}

struct UndeclaredFeatureRustdocPort;

impl RustdocCratePort for UndeclaredFeatureRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(rustdoc_crate_without_gated_public_item()))
    }

    fn capture_current(
        &self,
        _crate_name: &CrateName,
        features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        assert!(features.is_empty(), "the track declares no feature for the gated catalogue item");
        Ok(current_rustdoc(
            _crate_name,
            rustdoc_crate_without_gated_public_item(),
            evaluation_start,
        ))
    }
}

struct CatalogueItemMissingFromActualEvaluator {
    observed_membership: Arc<Mutex<Vec<(bool, bool, bool)>>>,
}

impl SignalEvaluatorPort for CatalogueItemMissingFromActualEvaluator {
    fn evaluate(
        &self,
        catalogue: ExtendedCrate,
        baseline: Crate,
        actual: Crate,
    ) -> Result<ThreeWayEvaluationReport, Phase1Error> {
        let has_gated_item = |crate_: &Crate| {
            crate_.index.values().any(|item| item.name.as_deref() == Some("FeatureGatedPublicItem"))
        };
        let observed =
            (has_gated_item(catalogue.krate()), has_gated_item(&baseline), has_gated_item(&actual));
        self.observed_membership.lock().unwrap().push(observed);
        let signals = if observed == (true, false, false) {
            vec![ThreeWaySignal::catalogue_item(
                FreeText::new("FeatureGatedPublicItem"),
                CatalogueItemNamespace::Type,
                domain::tddd::signal_evaluator::region::SignalRegion::SMinusC_Reference,
            )]
        } else {
            vec![]
        };
        Ok(ThreeWayEvaluationReport::new(signals))
    }
}

impl RustdocBaselineCapturePort for RecordingBaselineCapture {
    fn capture(
        &self,
        _items_dir: &Path,
        _track_id: &TrackId,
        _rustdoc_workspace: &Path,
        _binding: &TdddLayerBinding,
        features: &[CargoFeatureName],
    ) -> Result<(), BaselineCaptureIoError> {
        self.observed_features
            .lock()
            .unwrap()
            .push(features.iter().map(|feature| feature.as_str().to_owned()).collect());
        Ok(())
    }
}

struct EmptyExtendedCrateCodec;

impl domain::tddd::CatalogueToExtendedCratePort for EmptyExtendedCrateCodec {
    fn encode(
        &self,
        _target_layer: &LayerId,
        _track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        _rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        Ok(ExtendedCrate::new(empty_rustdoc_crate(), BTreeMap::new()))
    }
}

struct RecordingExtendedCrateCodec {
    observed: Arc<
        Mutex<
            Vec<(
                LayerId,
                BTreeMap<LayerId, CatalogueDocument>,
                BTreeMap<LayerId, AuthoritativeRustdocContext>,
            )>,
        >,
    >,
}

fn synthesized_cross_layer_handoff_crate() -> ExtendedCrate {
    let external_crate_id = 7;
    let mut crate_ = empty_rustdoc_crate();
    crate_.external_crates.insert(
        external_crate_id,
        ExternalCrate {
            name: "domain".to_owned(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        },
    );
    for (id, name, module_path) in
        [(1, "UserId", vec!["domain", "model"]), (2, "PendingId", vec!["domain", "model"])]
    {
        let id = Id(id);
        crate_.index.insert(
            id,
            Item {
                id,
                crate_id: external_crate_id,
                name: Some(name.to_owned()),
                span: None,
                visibility: Visibility::Public,
                docs: None,
                links: HashMap::new(),
                attrs: vec![],
                deprecation: None,
                inner: ItemEnum::Struct(Struct {
                    kind: rustdoc_types::StructKind::Unit,
                    generics: Generics { params: vec![], where_predicates: vec![] },
                    impls: vec![],
                }),
            },
        );
        let mut path = module_path.into_iter().map(str::to_owned).collect::<Vec<_>>();
        path.push(name.to_owned());
        crate_
            .paths
            .insert(id, ItemSummary { crate_id: external_crate_id, path, kind: ItemKind::Struct });
    }
    ExtendedCrate::new(crate_, BTreeMap::new())
}

impl domain::tddd::CatalogueToExtendedCratePort for RecordingExtendedCrateCodec {
    fn encode(
        &self,
        target_layer: &LayerId,
        track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        self.observed.lock().unwrap().push((
            target_layer.clone(),
            track_catalogues.clone(),
            rustdoc_contexts.clone(),
        ));
        Ok(ExtendedCrate::new(empty_rustdoc_crate(), BTreeMap::new()))
    }
}

struct TraitResolutionHandoffCodec {
    observed: Arc<Mutex<Vec<ExtendedCrate>>>,
}

fn external_trait_resolution_set() -> ExtendedCrate {
    let external_crate_id = 7;
    let trait_id = Id(1);
    let mut crate_ = empty_rustdoc_crate();
    crate_.external_crates.insert(
        external_crate_id,
        ExternalCrate {
            name: "domain".to_owned(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        },
    );
    crate_.index.insert(
        trait_id,
        Item {
            id: trait_id,
            crate_id: external_crate_id,
            name: Some("Repository".to_owned()),
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Trait(rustdoc_types::Trait {
                is_auto: false,
                is_unsafe: false,
                is_dyn_compatible: true,
                items: vec![],
                generics: Generics { params: vec![], where_predicates: vec![] },
                bounds: vec![],
                implementations: vec![],
            }),
        },
    );
    crate_.paths.insert(
        trait_id,
        ItemSummary {
            crate_id: external_crate_id,
            path: vec!["domain".to_owned(), "ports".to_owned(), "Repository".to_owned()],
            kind: ItemKind::Trait,
        },
    );
    ExtendedCrate::new(crate_, BTreeMap::new())
}

impl domain::tddd::CatalogueToExtendedCratePort for TraitResolutionHandoffCodec {
    fn encode(
        &self,
        target_layer: &LayerId,
        track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        _rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        assert_eq!(target_layer.as_ref(), "usecase");

        let domain_layer = LayerId::try_new("domain").unwrap();
        let usecase_layer = LayerId::try_new("usecase").unwrap();
        let trait_key = domain::tddd::catalogue_v2::CatalogueEntryKey::try_new(
            "domain::ports::Repository".to_owned(),
        )
        .unwrap();
        let declaring = track_catalogues
            .get(&domain_layer)
            .expect("the declaring catalogue must reach the codec");
        let repository = declaring
            .traits()
            .get(&trait_key)
            .expect("the declaring trait add must reach the codec resolution input");
        assert_eq!(repository.action(), domain::tddd::catalogue_v2::ItemAction::Add);
        assert_eq!(repository.module_path().map(ToString::to_string), Some("ports".to_owned()));

        let referring = track_catalogues
            .get(&usecase_layer)
            .expect("the referring catalogue must reach the codec");
        assert!(
            referring
                .trait_impls()
                .iter()
                .any(|decl| decl.trait_ref().as_str() == "domain::ports::Repository"),
            "the referring layer must retain the trait reference used for resolution"
        );
        assert!(
            !referring.traits().contains_key(&trait_key),
            "the referring catalogue must not duplicate the declaring trait add"
        );

        let resolution_set = external_trait_resolution_set();
        self.observed.lock().unwrap().push(resolution_set.clone());
        Ok(resolution_set)
    }
}

struct TrackCatalogueLoader {
    documents: BTreeMap<String, CatalogueDocument>,
}

impl AttestedCatalogueDocumentLoaderPort for TrackCatalogueLoader {
    fn load(
        &self,
        path: &Path,
    ) -> Result<domain::tddd::catalogue_v2::AttestedCatalogueDocument, CatalogueDocumentLoaderError>
    {
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() })?;
        let document =
            self.documents.get(file_name).cloned().ok_or_else(|| {
                CatalogueDocumentLoaderError::NotFound { path: path.to_path_buf() }
            })?;
        Ok(domain::tddd::catalogue_v2::AttestedCatalogueDocument::attest(
            b"T004 track catalogue",
            |_| Ok::<_, std::convert::Infallible>(document),
        )
        .unwrap())
    }
}

struct ChangingHashCatalogueLoader {
    document: CatalogueDocument,
    calls: Arc<Mutex<usize>>,
}

impl AttestedCatalogueDocumentLoaderPort for ChangingHashCatalogueLoader {
    fn load(
        &self,
        _path: &Path,
    ) -> Result<domain::tddd::catalogue_v2::AttestedCatalogueDocument, CatalogueDocumentLoaderError>
    {
        let call = {
            let mut calls = self.calls.lock().unwrap();
            *calls += 1;
            *calls
        };
        let source: &[u8] = if call == 1 {
            b"T014 initial catalogue generation"
        } else {
            b"T014 changed catalogue generation"
        };
        Ok(domain::tddd::catalogue_v2::AttestedCatalogueDocument::attest(source, |_| {
            Ok::<_, std::convert::Infallible>(self.document.clone())
        })
        .unwrap())
    }
}

struct DistinguishableRustdocPort {
    baseline: Crate,
    current: Crate,
}

impl RustdocCratePort for DistinguishableRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(self.baseline.clone()))
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        _features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        Ok(current_rustdoc(crate_name, self.current.clone(), evaluation_start))
    }
}

struct LayerAwareRustdocPort;

fn rustdoc_crate_with_layer_marker(layer: &str, phase: &str) -> Crate {
    let mut crate_ = empty_rustdoc_crate();
    crate_.crate_version = Some(format!("{phase}-{layer}"));
    crate_.paths.insert(
        Id(1),
        ItemSummary {
            crate_id: 0,
            path: vec![layer.to_owned(), "model".to_owned(), "Shared".to_owned()],
            kind: ItemKind::Struct,
        },
    );
    crate_
}

impl RustdocCratePort for LayerAwareRustdocPort {
    fn load_from_path(&self, path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        let layer = path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix("-types-baseline"))
            .unwrap_or("unknown");
        Ok(captured_rustdoc(rustdoc_crate_with_layer_marker(layer, "baseline")))
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        _features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        Ok(current_rustdoc(
            crate_name,
            rustdoc_crate_with_layer_marker(crate_name.as_str(), "current"),
            evaluation_start,
        ))
    }
}

struct CrossLayerHandoffRustdocPort;

fn rustdoc_crate_with_cross_layer_handoff_items(layer: &str, phase: &str) -> Crate {
    let mut crate_ = empty_rustdoc_crate();
    crate_.crate_version = Some(format!("{phase}-{layer}"));
    if layer == "domain" && phase == "current" {
        for (id, name) in [(1, "UserId"), (2, "PendingId")] {
            crate_.paths.insert(
                Id(id),
                ItemSummary {
                    crate_id: 0,
                    path: vec!["domain".to_owned(), "model".to_owned(), name.to_owned()],
                    kind: ItemKind::Struct,
                },
            );
        }
    }
    crate_
}

impl RustdocCratePort for CrossLayerHandoffRustdocPort {
    fn load_from_path(&self, path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        let layer = path
            .file_stem()
            .and_then(|name| name.to_str())
            .and_then(|name| name.strip_suffix("-types-baseline"))
            .unwrap_or("unknown");
        Ok(captured_rustdoc(rustdoc_crate_with_cross_layer_handoff_items(layer, "baseline")))
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        _features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        Ok(current_rustdoc(
            crate_name,
            rustdoc_crate_with_cross_layer_handoff_items(crate_name.as_str(), "current"),
            evaluation_start,
        ))
    }
}

struct BinAliasHandoffRustdocPort {
    requested_targets: Arc<Mutex<Vec<String>>>,
}

fn rustdoc_crate_with_root_name(root_name: &str) -> Crate {
    let mut crate_ = empty_rustdoc_crate();
    crate_.index.insert(
        Id(0),
        Item {
            id: Id(0),
            crate_id: 0,
            name: Some(root_name.to_owned()),
            span: None,
            visibility: Visibility::Public,
            docs: None,
            links: HashMap::new(),
            attrs: vec![],
            deprecation: None,
            inner: ItemEnum::Module(Module { is_crate: true, items: vec![], is_stripped: false }),
        },
    );
    crate_
}

fn bin_alias_domain_current() -> Crate {
    let mut crate_ = rustdoc_crate_with_root_name("sotp");
    crate_.paths.insert(
        Id(1),
        ItemSummary {
            crate_id: 0,
            path: vec!["sotp".to_owned(), "model".to_owned(), "UserId".to_owned()],
            kind: ItemKind::Struct,
        },
    );
    crate_
}

fn referring_side_current() -> Crate {
    let mut crate_ = rustdoc_crate_with_root_name("usecase");
    crate_.external_crates.insert(
        7,
        ExternalCrate {
            name: "domain".to_owned(),
            html_root_url: None,
            path: std::path::PathBuf::new(),
        },
    );
    crate_.paths.insert(
        Id(1),
        ItemSummary {
            crate_id: 7,
            path: vec!["domain".to_owned(), "model".to_owned(), "UserId".to_owned()],
            kind: ItemKind::Struct,
        },
    );
    crate_
}

impl RustdocCratePort for BinAliasHandoffRustdocPort {
    fn load_from_path(&self, _path: &Path) -> Result<CapturedRustdocJson, RustdocCratePortError> {
        Ok(captured_rustdoc(empty_rustdoc_crate()))
    }

    fn capture_current(
        &self,
        crate_name: &CrateName,
        _features: &[CargoFeatureName],
        evaluation_start: &ImplementationFingerprint,
    ) -> Result<AttestedRustdocSnapshot, RustdocCratePortError> {
        self.requested_targets.lock().unwrap().push(crate_name.as_str().to_owned());
        match crate_name.as_str() {
            "domain_bin" => {
                Ok(current_rustdoc(crate_name, bin_alias_domain_current(), evaluation_start))
            }
            "usecase" => {
                Ok(current_rustdoc(crate_name, referring_side_current(), evaluation_start))
            }
            target => panic!("unexpected test rustdoc target: {target}"),
        }
    }
}

macro_rules! impl_fixed_evaluation_start_capture {
    ($($port:ty),+ $(,)?) => {
        $(
            impl EvaluationStartCapturePort for $port {
                fn capture_evaluation_start(
                    &self,
                ) -> Result<ImplementationFingerprint, EvaluationStartCaptureError> {
                    Ok(evaluation_start_fingerprint())
                }
            }
        )+
    };
}

impl_fixed_evaluation_start_capture!(
    NeverCalledRustdocPort,
    EmptyRustdocPort,
    FailingRustdocPort,
    CaptureFailureRustdocPort,
    FeatureGatedRustdocPort,
    UndeclaredFeatureRustdocPort,
    DistinguishableRustdocPort,
    LayerAwareRustdocPort,
    CrossLayerHandoffRustdocPort,
    BinAliasHandoffRustdocPort,
);

struct RustdocWinsAndAliasCodec {
    observed: Arc<Mutex<Vec<ExtendedCrate>>>,
}

impl domain::tddd::CatalogueToExtendedCratePort for RustdocWinsAndAliasCodec {
    fn encode(
        &self,
        target_layer: &LayerId,
        track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        use domain::tddd::catalogue_v2::CatalogueEntryKey;
        use domain::tddd::catalogue_v2::composite::{StructShape, TypeKindV2};

        assert_eq!(target_layer.as_ref(), "usecase");
        let domain_layer = LayerId::try_new("domain").unwrap();
        let usecase_layer = LayerId::try_new("usecase").unwrap();
        let declaring = track_catalogues.get(&domain_layer).expect("declaring catalogue");
        let referring = track_catalogues.get(&usecase_layer).expect("referring catalogue");
        let handler = referring
            .types()
            .get(&CatalogueEntryKey::try_new("Handler".to_owned()).unwrap())
            .expect("the referring catalogue must contain the cross-crate reference");
        let TypeKindV2::Struct(kind) = handler.kind() else {
            panic!("the referring catalogue must contain a struct handler");
        };
        let StructShape::Plain { fields, .. } = &kind.shape else {
            panic!("the referring handler must contain a named field");
        };
        assert_eq!(fields[0].ty.as_str(), "domain::model::UserId");

        let declaring_current =
            rustdoc_contexts.get(&domain_layer).expect("declaring-layer rustdoc context").current();
        assert_eq!(declaring_current.index[&declaring_current.root].name.as_deref(), Some("sotp"));
        let raw_declaring = declaring_current
            .paths
            .values()
            .find(|summary| summary.path == ["sotp", "model", "UserId"])
            .expect("the bin target must expose the raw rustdoc identity");
        let mut canonical_declaring = raw_declaring.path.clone();
        canonical_declaring[0] = declaring.crate_name().as_str().to_owned();

        let referring_current = rustdoc_contexts
            .get(&usecase_layer)
            .expect("referring-layer rustdoc context")
            .current();
        let rustdoc_identity = referring_current
            .paths
            .values()
            .filter(|summary| summary.path == ["domain", "model", "UserId"])
            .collect::<Vec<_>>();
        assert_eq!(rustdoc_identity.len(), 1, "the referencing rustdoc identity must be unique");
        assert_eq!(rustdoc_identity[0].crate_id, 7);
        assert_eq!(canonical_declaring, rustdoc_identity[0].path);

        let encoded = ExtendedCrate::new(referring_current.clone(), BTreeMap::new());
        self.observed.lock().unwrap().push(encoded.clone());
        Ok(encoded)
    }
}

struct CrossLayerHandoffCodec {
    calls: Arc<Mutex<Vec<LayerId>>>,
}

impl domain::tddd::CatalogueToExtendedCratePort for CrossLayerHandoffCodec {
    fn encode(
        &self,
        target_layer: &LayerId,
        track_catalogues: &BTreeMap<LayerId, CatalogueDocument>,
        rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    ) -> Result<ExtendedCrate, domain::tddd::NewTypeGraphCodecError> {
        self.calls.lock().unwrap().push(target_layer.clone());
        assert_eq!(target_layer.as_ref(), "usecase");

        let domain_layer = LayerId::try_new("domain").unwrap();
        let usecase_layer = LayerId::try_new("usecase").unwrap();
        let declaring = track_catalogues
            .get(&domain_layer)
            .expect("the codec handoff must include the declaring-layer catalogue");
        assert_eq!(declaring.crate_name().as_str(), "domain");

        let explicit = declaring
            .types()
            .get(
                &domain::tddd::catalogue_v2::CatalogueEntryKey::try_new(
                    "domain::model::UserId".to_owned(),
                )
                .unwrap(),
            )
            .expect("the explicit cross-crate add must reach the codec");
        assert_eq!(explicit.action(), domain::tddd::catalogue_v2::ItemAction::Add);
        assert_eq!(explicit.module_path().map(ToString::to_string), Some("model".to_owned()));

        let omitted = declaring
            .types()
            .get(
                &domain::tddd::catalogue_v2::CatalogueEntryKey::try_new("PendingId".to_owned())
                    .unwrap(),
            )
            .expect("the omitted-placement cross-crate add must reach the codec");
        assert!(omitted.module_path().is_none());

        let referring = track_catalogues
            .get(&usecase_layer)
            .expect("the codec handoff must include the referring-layer catalogue");
        assert!(
            !referring
                .types()
                .keys()
                .any(|key| key.as_str().contains("UserId") || key.as_str().contains("PendingId")),
            "the referring catalogue must not duplicate declaring-layer additions"
        );

        let declaring_current = rustdoc_contexts
            .get(&domain_layer)
            .expect("the codec handoff must include the declaring-layer rustdoc context")
            .current();
        for expected in [["domain", "model", "UserId"], ["domain", "model", "PendingId"]] {
            assert!(
                declaring_current.paths.values().any(|summary| summary.path == expected),
                "declaring-layer rustdoc must expose the cross-crate identity {expected:?}"
            );
        }
        assert!(
            !declaring_current
                .paths
                .values()
                .any(|summary| summary.path == ["domain", "model", "Shared"]),
            "an unrelated rustdoc item must not stand in for the add identity"
        );

        Ok(synthesized_cross_layer_handoff_crate())
    }
}

/// No-op `SymlinkGuardPort` that always reports "no symlink found".
///
/// Used as the default in tests that don't exercise the symlink guard path.
pub(super) struct NoopSymlinkGuard;

impl SymlinkGuardPort for NoopSymlinkGuard {
    fn reject_symlinks_from_root(&self, _path: &Path) -> Result<(), SymlinkGuardError> {
        Ok(())
    }

    fn reject_symlinks_below(
        &self,
        _path: &Path,
        _trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        Ok(())
    }
}

/// `SymlinkGuardPort` that always rejects with `SymlinkFound`.
///
/// Used to test that the interactor correctly propagates symlink rejection.
pub(super) struct AlwaysRejectSymlinkGuard;

impl SymlinkGuardPort for AlwaysRejectSymlinkGuard {
    fn reject_symlinks_from_root(&self, path: &Path) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::SymlinkFound { path: path.display().to_string() })
    }

    fn reject_symlinks_below(
        &self,
        path: &Path,
        _trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::SymlinkFound { path: path.display().to_string() })
    }
}

/// `SymlinkGuardPort` that fails with an I/O error for every guard call.
struct AlwaysIoSymlinkGuard;

impl SymlinkGuardPort for AlwaysIoSymlinkGuard {
    fn reject_symlinks_from_root(&self, path: &Path) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::Io {
            path: path.display().to_string(),
            reason: "permission denied".to_owned(),
        })
    }

    fn reject_symlinks_below(
        &self,
        path: &Path,
        _trusted_root: &Path,
    ) -> Result<(), SymlinkGuardError> {
        Err(SymlinkGuardError::Io {
            path: path.display().to_string(),
            reason: "permission denied".to_owned(),
        })
    }
}

// -------------------------------------------------------------------------
// Interactor builder helper — also re-used by `happy_tests`
// -------------------------------------------------------------------------

pub(super) fn build_interactor<R>(
    loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
    codec: Arc<dyn domain::tddd::CatalogueToExtendedCratePort>,
    evaluator: Arc<dyn SignalEvaluatorPort>,
    rustdoc: Arc<R>,
    bindings: Arc<dyn TdddLayerBindingsPort>,
) -> CatalogueImplSignalsInteractor
where
    R: RustdocCratePort + EvaluationStartCapturePort + 'static,
{
    build_interactor_with_guard(
        loader,
        codec,
        evaluator,
        rustdoc,
        bindings,
        Arc::new(EmptyFeatureDeclaration),
        Arc::new(NoopSymlinkGuard),
    )
}

pub(super) fn build_interactor_with_guard<R>(
    loader: Arc<dyn AttestedCatalogueDocumentLoaderPort>,
    codec: Arc<dyn domain::tddd::CatalogueToExtendedCratePort>,
    evaluator: Arc<dyn SignalEvaluatorPort>,
    rustdoc: Arc<R>,
    bindings: Arc<dyn TdddLayerBindingsPort>,
    feature_declaration: Arc<dyn TdddActualFeatureDeclarationPort>,
    symlink_guard: Arc<dyn SymlinkGuardPort>,
) -> CatalogueImplSignalsInteractor
where
    R: RustdocCratePort + EvaluationStartCapturePort + 'static,
{
    let evaluation_start_capture_port: Arc<dyn EvaluationStartCapturePort> = rustdoc.clone();
    let rustdoc_crate_port: Arc<dyn RustdocCratePort> = rustdoc;
    CatalogueImplSignalsInteractor::new(
        loader,
        codec,
        evaluator,
        evaluation_start_capture_port,
        rustdoc_crate_port,
        bindings,
        feature_declaration,
        symlink_guard,
    )
}

#[test]
fn test_run_binds_one_evaluation_start_to_every_current_capture() {
    let start_calls = Arc::new(Mutex::new(0));
    let captures = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([
                ("domain-types.json".to_owned(), minimal_catalogue_doc("domain")),
                ("usecase-types.json".to_owned(), minimal_catalogue_doc("usecase")),
            ]),
        }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(RecordingStartBindingRustdocPort {
            start_calls: Arc::clone(&start_calls),
            captures: Arc::clone(&captures),
        }),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap();

    assert_eq!(*start_calls.lock().unwrap(), 1);
    let expected_start = evaluation_start_fingerprint();
    assert_eq!(captures.lock().unwrap().as_slice(), &[expected_start.clone(), expected_start]);
}

#[test]
fn test_run_rejects_a_later_generation_snapshot_before_evaluation() {
    let captures = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([
                ("domain-types.json".to_owned(), minimal_catalogue_doc("domain")),
                ("usecase-types.json".to_owned(), minimal_catalogue_doc("usecase")),
            ]),
        }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(MixedGenerationRustdocPort { captures: Arc::clone(&captures) }),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    let error =
        interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap_err();

    assert!(matches!(
        error,
        CatalogueImplSignalsError::SchemaExport(layer, reason)
            if layer.as_ref() == "usecase"
                && reason.as_str().contains("evaluation-start fingerprint")
    ));
    assert_eq!(
        captures.lock().unwrap().as_slice(),
        &[evaluation_start_fingerprint(), evaluation_start_fingerprint()]
    );
}

#[test]
fn test_run_evaluation_start_capture_failure_fails_closed_before_rustdoc_access() {
    let current_calls = Arc::new(Mutex::new(0));
    let interactor = build_interactor(
        Arc::new(StubLoader { doc: minimal_catalogue_doc("domain") }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(StartCaptureFailureRustdocPort { current_calls: Arc::clone(&current_calls) }),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
    );
    let workspace = tempfile::tempdir().unwrap();

    let error =
        interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap_err();

    assert!(matches!(
        &error,
        CatalogueImplSignalsError::EvaluationStartCapture(
            EvaluationStartCaptureError::AuthoritativeInput { .. }
        )
    ));
    assert!(
        error.to_string().contains("stub evaluation-start fingerprint failure"),
        "the evaluation-start diagnostic must cross the usecase boundary: {error}"
    );
    assert_eq!(*current_calls.lock().unwrap(), 0);
}

#[test]
fn test_run_codec_receives_baseline_and_current_crates_in_order() {
    let mut expected_baseline = empty_rustdoc_crate();
    expected_baseline.crate_version = Some("baseline-fixture".to_owned());
    let mut expected_current = empty_rustdoc_crate();
    expected_current.crate_version = Some("current-fixture".to_owned());
    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(StubLoader { doc: minimal_catalogue_doc("domain") }),
        Arc::new(RecordingExtendedCrateCodec { observed: Arc::clone(&observed) }),
        Arc::new(EmptyEvaluator),
        Arc::new(DistinguishableRustdocPort {
            baseline: expected_baseline.clone(),
            current: expected_current.clone(),
        }),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap();

    let (actual_target, actual_catalogues, actual_rustdoc_contexts) =
        observed.lock().unwrap().pop().unwrap();
    assert_eq!(actual_target, LayerId::try_new("domain").unwrap());
    assert_eq!(actual_catalogues.len(), 1);
    assert_eq!(
        actual_catalogues.get(&LayerId::try_new("domain").unwrap()).unwrap().crate_name().as_str(),
        "domain"
    );
    let actual_rustdoc_context = actual_rustdoc_contexts
        .get(&actual_target)
        .expect("codec must receive the target layer's rustdoc context");
    assert_eq!(actual_rustdoc_context.baseline(), &expected_baseline);
    assert_eq!(actual_rustdoc_context.current(), &expected_current);
}

#[test]
fn test_run_codec_receives_all_track_catalogues_for_each_target_layer() {
    use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
    use domain::tddd::catalogue_v2::{CatalogueEntryKey, FieldDecl, FieldName, ModulePath};

    let domain_layer = LayerId::try_new("domain").unwrap();
    let usecase_layer = LayerId::try_new("usecase").unwrap();
    let mut domain_doc = minimal_catalogue_doc("domain");
    domain_doc.insert_type(
        CatalogueEntryKey::try_new("domain::model::UserId".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::from_segments(vec!["model".to_owned()]).unwrap()),
            None,
            vec![],
            vec![],
        ),
    );
    let mut usecase_doc = minimal_catalogue_doc("usecase");
    usecase_doc.insert_type(
        CatalogueEntryKey::try_new("Handler".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("id").unwrap(),
                        TypeRef::new("domain::model::UserId").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );
    let loader = TrackCatalogueLoader {
        documents: BTreeMap::from([
            ("domain-types.json".to_owned(), domain_doc.clone()),
            ("usecase-types.json".to_owned(), usecase_doc.clone()),
        ]),
    };
    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(loader),
        Arc::new(RecordingExtendedCrateCodec { observed: Arc::clone(&observed) }),
        Arc::new(EmptyEvaluator),
        Arc::new(LayerAwareRustdocPort),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap();

    let observations = observed.lock().unwrap();
    assert_eq!(observations.len(), 2);
    assert_eq!(observations[0].0, domain_layer);
    assert_eq!(observations[1].0, usecase_layer);
    for (_, catalogues, rustdoc_contexts) in observations.iter() {
        assert_eq!(catalogues.len(), 2);
        assert_eq!(catalogues.get(&domain_layer), Some(&domain_doc));
        assert_eq!(catalogues.get(&usecase_layer), Some(&usecase_doc));
        assert!(
            catalogues
                .get(&domain_layer)
                .and_then(|doc| doc
                    .types()
                    .get(&CatalogueEntryKey::try_new("domain::model::UserId".to_owned()).unwrap()))
                .is_some_and(|entry| entry.action() == ItemAction::Add),
            "the codec must receive the declaring-layer add without a usecase duplicate"
        );
        assert!(
            catalogues
                .get(&usecase_layer)
                .is_some_and(|doc| !doc.types().keys().any(|key| key.as_str().contains("UserId"))),
            "the referencing catalogue must not duplicate the declaring-layer add"
        );

        assert_eq!(rustdoc_contexts.len(), 2);
        for (layer, expected_baseline, expected_current) in [
            (&domain_layer, "baseline-domain", "current-domain"),
            (&usecase_layer, "baseline-usecase", "current-usecase"),
        ] {
            let context = rustdoc_contexts
                .get(layer)
                .expect("codec must receive every configured layer's rustdoc context");
            assert_eq!(context.baseline().crate_version.as_deref(), Some(expected_baseline));
            assert_eq!(context.current().crate_version.as_deref(), Some(expected_current));
            assert!(
                context
                    .current()
                    .paths
                    .values()
                    .any(|summary| { summary.path == [layer.as_ref(), "model", "Shared"] })
            );
        }
    }
}

#[test]
fn test_run_codec_forwards_cross_layer_trait_add_without_reference_duplicate() {
    use domain::tddd::catalogue_v2::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::entries::TraitEntry;
    use domain::tddd::catalogue_v2::roles::{ContractRole, ItemAction};
    use domain::tddd::catalogue_v2::{CatalogueEntryKey, ModulePath};

    let domain_layer = LayerId::try_new("domain").unwrap();
    let usecase_layer = LayerId::try_new("usecase").unwrap();
    let trait_key = CatalogueEntryKey::try_new("domain::ports::Repository".to_owned()).unwrap();
    let mut domain_doc = minimal_catalogue_doc("domain");
    domain_doc.insert_trait(
        trait_key.clone(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(ModulePath::from_segments(vec!["ports".to_owned()]).unwrap()),
            None,
            vec![],
            vec![],
        ),
    );
    let mut usecase_doc = minimal_catalogue_doc("usecase");
    usecase_doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("domain::ports::Repository").unwrap(),
        TypeRef::new("Handler").unwrap(),
    ));

    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([
                ("domain-types.json".to_owned(), domain_doc),
                ("usecase-types.json".to_owned(), usecase_doc),
            ]),
        }),
        Arc::new(RecordingExtendedCrateCodec { observed: Arc::clone(&observed) }),
        Arc::new(EmptyEvaluator),
        Arc::new(LayerAwareRustdocPort),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap();

    let observations = observed.lock().unwrap();
    assert_eq!(observations.len(), 2);
    for (_, catalogues, _) in observations.iter() {
        let declaring = catalogues
            .get(&domain_layer)
            .expect("the codec handoff must include the declaring layer");
        let repository = declaring
            .traits()
            .get(&trait_key)
            .expect("the declaring-layer trait add must reach the codec");
        assert_eq!(repository.action(), ItemAction::Add);
        assert_eq!(repository.module_path().map(ToString::to_string), Some("ports".to_owned()));

        let referring = catalogues
            .get(&usecase_layer)
            .expect("the codec handoff must include the referring layer");
        assert!(
            referring
                .trait_impls()
                .iter()
                .any(|decl| decl.trait_ref().as_str() == "domain::ports::Repository"),
            "the referring layer must retain its cross-layer trait reference"
        );
        assert!(
            !referring.traits().contains_key(&trait_key),
            "the referring catalogue must not duplicate the declaring-layer trait add"
        );
    }
}

#[test]
fn test_run_codec_resolution_set_adds_referenced_trait_as_declaring_crate_external_item() {
    use domain::tddd::catalogue_v2::TraitImplDeclV2;
    use domain::tddd::catalogue_v2::entries::TraitEntry;
    use domain::tddd::catalogue_v2::roles::{ContractRole, ItemAction};
    use domain::tddd::catalogue_v2::{CatalogueEntryKey, ModulePath};

    let trait_key = CatalogueEntryKey::try_new("domain::ports::Repository".to_owned()).unwrap();
    let mut domain_doc = minimal_catalogue_doc("domain");
    domain_doc.insert_trait(
        trait_key.clone(),
        TraitEntry::new(
            ItemAction::Add,
            ContractRole::SecondaryPort,
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            vec![],
            Some(ModulePath::from_segments(vec!["ports".to_owned()]).unwrap()),
            None,
            vec![],
            vec![],
        ),
    );
    let mut usecase_doc = minimal_catalogue_doc("usecase");
    usecase_doc.push_trait_impl(TraitImplDeclV2::new(
        TypeRef::new("domain::ports::Repository").unwrap(),
        TypeRef::new("Handler").unwrap(),
    ));

    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([
                ("domain-types.json".to_owned(), domain_doc),
                ("usecase-types.json".to_owned(), usecase_doc),
            ]),
        }),
        Arc::new(TraitResolutionHandoffCodec { observed: Arc::clone(&observed) }),
        Arc::new(RecordingExtendedCrateEvaluator { observed: Arc::clone(&observed) }),
        Arc::new(LayerAwareRustdocPort),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor
        .run("my-track".to_owned(), workspace.path().to_path_buf(), Some("usecase".to_owned()))
        .unwrap();

    let encoded = observed
        .lock()
        .unwrap()
        .pop()
        .expect("the evaluator must observe the codec resolution set");
    let repository = encoded
        .krate()
        .paths
        .iter()
        .find(|(_, summary)| summary.path == ["domain", "ports", "Repository"])
        .expect("the referenced trait must be present in the resolution set");
    assert_eq!(repository.1.kind, ItemKind::Trait);
    assert_ne!(repository.1.crate_id, 0, "the declaring trait must be external to usecase");
    assert_eq!(encoded.krate().external_crates[&repository.1.crate_id].name, "domain");
    assert!(matches!(&encoded.krate().index[repository.0].inner, ItemEnum::Trait(_)));
}

#[test]
fn test_run_codec_handoff_observes_synthesized_item_identity_and_module_placement() {
    use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
    use domain::tddd::catalogue_v2::{CatalogueEntryKey, FieldDecl, FieldName, ModulePath};

    let usecase_layer = LayerId::try_new("usecase").unwrap();
    let mut domain_doc = minimal_catalogue_doc("domain");
    domain_doc.insert_type(
        CatalogueEntryKey::try_new("domain::model::UserId".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::from_segments(vec!["model".to_owned()]).unwrap()),
            None,
            vec![],
            vec![],
        ),
    );
    domain_doc.insert_type(
        CatalogueEntryKey::try_new("PendingId".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            None,
            None,
            vec![],
            vec![],
        ),
    );

    let mut usecase_doc = minimal_catalogue_doc("usecase");
    usecase_doc.insert_type(
        CatalogueEntryKey::try_new("Handler".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![
                        FieldDecl::new(
                            FieldName::new("user_id").unwrap(),
                            TypeRef::new("domain::model::UserId").unwrap(),
                        ),
                        FieldDecl::new(
                            FieldName::new("pending_id").unwrap(),
                            TypeRef::new("domain::model::PendingId").unwrap(),
                        ),
                    ],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );

    let observed = Arc::new(Mutex::new(Vec::new()));
    let observed_items = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([
                ("domain-types.json".to_owned(), domain_doc),
                ("usecase-types.json".to_owned(), usecase_doc),
            ]),
        }),
        Arc::new(CrossLayerHandoffCodec { calls: Arc::clone(&observed) }),
        Arc::new(RecordingExtendedCrateEvaluator { observed: Arc::clone(&observed_items) }),
        Arc::new(CrossLayerHandoffRustdocPort),
        Arc::new(StubLayerBindings {
            bindings: vec![stub_binding("domain"), stub_binding("usecase")],
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor
        .run("my-track".to_owned(), workspace.path().to_path_buf(), Some("usecase".to_owned()))
        .unwrap();

    assert_eq!(observed.lock().unwrap().as_slice(), [usecase_layer]);
    let encoded =
        observed_items.lock().unwrap().pop().expect("the evaluator must observe TypeGraph A");
    for (name, expected_path) in
        [("UserId", ["domain", "model", "UserId"]), ("PendingId", ["domain", "model", "PendingId"])]
    {
        let (id, summary) = encoded
            .krate()
            .paths
            .iter()
            .find(|(_, summary)| summary.path == expected_path)
            .unwrap_or_else(|| panic!("synthesized {name} must retain its resolved path"));
        assert_eq!(summary.crate_id, 7);
        assert_eq!(encoded.krate().external_crates[&summary.crate_id].name, "domain");
        assert_eq!(encoded.krate().index[id].name.as_deref(), Some(name));
    }
    assert!(
        !encoded
            .krate()
            .paths
            .values()
            .any(|summary| summary.path.first().map(String::as_str) == Some("usecase")),
        "the synthesized items must not be rooted in the referencing crate"
    );
}

#[test]
fn test_run_codec_handoff_reuses_referencing_rustdoc_identity_and_canonicalizes_declaring_bin_target()
 {
    use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
    use domain::tddd::catalogue_v2::entries::TypeEntry;
    use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
    use domain::tddd::catalogue_v2::{CatalogueEntryKey, FieldDecl, FieldName, ModulePath};

    let mut domain_doc = minimal_catalogue_doc("domain");
    domain_doc.insert_type(
        CatalogueEntryKey::try_new("domain::model::UserId".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::from_segments(vec!["model".to_owned()]).unwrap()),
            None,
            vec![],
            vec![],
        ),
    );
    let mut usecase_doc = minimal_catalogue_doc("usecase");
    usecase_doc.insert_type(
        CatalogueEntryKey::try_new("Handler".to_owned()).unwrap(),
        TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("user_id").unwrap(),
                        TypeRef::new("domain::model::UserId").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            Some(ModulePath::root()),
            None,
            vec![],
            vec![],
        ),
    );

    let requested_targets = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([
                ("domain-types.json".to_owned(), domain_doc),
                ("usecase-types.json".to_owned(), usecase_doc),
            ]),
        }),
        Arc::new(RustdocWinsAndAliasCodec { observed: Arc::clone(&observed) }),
        Arc::new(EmptyEvaluator),
        Arc::new(BinAliasHandoffRustdocPort { requested_targets: Arc::clone(&requested_targets) }),
        Arc::new(StubLayerBindings {
            bindings: vec![
                stub_binding_with_target("domain", "domain_bin"),
                stub_binding("usecase"),
            ],
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor
        .run("my-track".to_owned(), workspace.path().to_path_buf(), Some("usecase".to_owned()))
        .unwrap();

    assert_eq!(
        requested_targets.lock().unwrap().clone(),
        vec!["domain_bin".to_owned(), "usecase".to_owned()]
    );
    let encoded = observed.lock().unwrap().pop().expect("the codec must be invoked once");
    let identity = encoded
        .krate()
        .paths
        .values()
        .filter(|summary| summary.path == ["domain", "model", "UserId"])
        .collect::<Vec<_>>();
    assert_eq!(identity.len(), 1, "rustdoc-wins must avoid a synthesized duplicate");
    assert_eq!(identity[0].crate_id, 7, "the reused item must remain external rustdoc");
    assert!(
        !encoded
            .krate()
            .paths
            .values()
            .any(|summary| summary.path.first().map(String::as_str) == Some("sotp")),
        "the bin-target root must be canonicalized to the declaring package name"
    );
}

#[test]
fn test_run_skips_missing_non_target_catalogue_when_layer_is_selected() {
    let domain_binding = stub_binding("domain");
    let usecase_binding = stub_binding("usecase");
    let binding_calls = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([(
                domain_binding.catalogue_file.clone(),
                minimal_catalogue_doc("domain"),
            )]),
        }),
        Arc::new(RecordingExtendedCrateCodec { observed: Arc::clone(&observed) }),
        Arc::new(EmptyEvaluator),
        Arc::new(LayerAwareRustdocPort),
        Arc::new(FilteringLayerBindings {
            bindings: vec![domain_binding, usecase_binding],
            calls: Arc::clone(&binding_calls),
        }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor
        .run("my-track".to_owned(), workspace.path().to_path_buf(), Some("domain".to_owned()))
        .unwrap();

    let observations = observed.lock().unwrap();
    assert_eq!(observations.len(), 1);
    assert_eq!(observations[0].0, LayerId::try_new("domain").unwrap());
    assert_eq!(observations[0].1.len(), 1);
    assert!(observations[0].1.contains_key(&LayerId::try_new("domain").unwrap()));
    assert!(!observations[0].1.contains_key(&LayerId::try_new("usecase").unwrap()));
    let domain_context = observations[0]
        .2
        .get(&LayerId::try_new("domain").unwrap())
        .expect("codec must receive the selected layer's rustdoc context");
    assert!(
        domain_context
            .current()
            .paths
            .values()
            .any(|summary| summary.path == ["domain", "model", "Shared"])
    );
    assert_eq!(binding_calls.lock().unwrap().as_slice(), &[None]);
}

#[test]
fn test_run_unfiltered_missing_enabled_catalogue_is_empty_declaration_set() {
    let domain_binding = stub_binding("domain");
    let usecase_binding = stub_binding("usecase");
    let observed = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([(
                domain_binding.catalogue_file.clone(),
                minimal_catalogue_doc("domain"),
            )]),
        }),
        Arc::new(RecordingExtendedCrateCodec { observed: Arc::clone(&observed) }),
        Arc::new(EmptyEvaluator),
        Arc::new(LayerAwareRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![domain_binding, usecase_binding] }),
    );
    let workspace = tempfile::tempdir().unwrap();

    interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap();

    let observations = observed.lock().unwrap();
    assert_eq!(observations.len(), 2, "unfiltered execution must still evaluate both layers");
    assert_eq!(observations[0].0, LayerId::try_new("domain").unwrap());
    assert_eq!(observations[1].0, LayerId::try_new("usecase").unwrap());
    for (_, catalogues, _) in observations.iter() {
        assert_eq!(catalogues.len(), 1);
        assert!(catalogues.contains_key(&LayerId::try_new("domain").unwrap()));
        assert!(!catalogues.contains_key(&LayerId::try_new("usecase").unwrap()));
    }
}

// -------------------------------------------------------------------------
// Happy-path / report-format tests (in sibling file to keep this file short)
// -------------------------------------------------------------------------

#[cfg(test)]
#[path = "interactor_happy_tests.rs"]
mod happy_tests;

// -------------------------------------------------------------------------
// CatalogueImplSignalsInteractor::run — error-path tests
// -------------------------------------------------------------------------

#[test]
fn test_run_workspace_root_with_dotdot_returns_symlink_rejected_error() {
    // A workspace_root containing `..` must be rejected before any I/O.
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![] }),
    );
    let bad_root = std::path::PathBuf::from("/tmp/../etc");
    let err = interactor.run("my-track".to_owned(), bad_root, None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::SymlinkRejected(_)),
        "expected SymlinkRejected for dot-dot workspace_root, got: {err:?}"
    );
}

#[test]
fn test_run_invalid_track_id_returns_invalid_track_id_error() {
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![] }),
    );
    let err = interactor
        .run("BAD TRACK ID!!".to_owned(), std::path::PathBuf::from("/tmp"), None)
        .unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::InvalidTrackId(_)),
        "expected InvalidTrackId, got: {err:?}"
    );
}

#[test]
fn test_run_no_layers_returns_no_layers_error() {
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(EmptyLayerBindings),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(matches!(err, CatalogueImplSignalsError::NoLayers), "expected NoLayers, got: {err:?}");
}

#[test]
fn test_run_layer_bindings_load_failure_returns_layer_bindings_load_error() {
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(FailingLayerBindings),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::LayerBindingsLoad(_)),
        "expected LayerBindingsLoad, got: {err:?}"
    );
}

#[test]
fn test_run_catalogue_load_failure_returns_catalogue_load_error() {
    let binding = stub_binding("domain");
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let err = interactor
        .run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), Some("domain".to_owned()))
        .unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::CatalogueLoad(_, _)),
        "expected CatalogueLoad, got: {err:?}"
    );
}

#[test]
fn test_run_rejects_catalogue_set_when_declaration_hash_changes_on_reread() {
    let calls = Arc::new(Mutex::new(0));
    let interactor = build_interactor(
        Arc::new(ChangingHashCatalogueLoader {
            document: minimal_catalogue_doc("domain"),
            calls: Arc::clone(&calls),
        }),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
    );

    let error =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();

    assert!(
        matches!(
            &error,
            CatalogueImplSignalsError::CatalogueLoad(layer, reason)
                if layer.as_ref() == "domain"
                    && reason.as_str().contains("declaration hash changed")
        ),
        "a changed catalogue generation must fail closed: {error:?}"
    );
    assert_eq!(
        *calls.lock().unwrap(),
        2,
        "the first set-level validation must reject the changed re-read before later I/O"
    );
}

#[test]
fn test_run_rejects_65th_configured_layer_before_feature_or_rustdoc_access() {
    let bindings = (0..65).map(|index| stub_binding(&format!("layer_{index}"))).collect();
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings }),
    );

    let error =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();

    assert!(
        matches!(error, CatalogueImplSignalsError::LayerLimitExceeded),
        "a 65th configured layer must stop the run before feature or rustdoc access: {error:?}"
    );
}

#[test]
fn test_run_catalogue_layer_mismatch_returns_catalogue_load_error() {
    let binding = stub_binding("domain");
    let interactor = build_interactor(
        Arc::new(TrackCatalogueLoader {
            documents: BTreeMap::from([(
                binding.catalogue_file.clone(),
                minimal_catalogue_doc("usecase"),
            )]),
        }),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();

    assert!(
        matches!(
            &err,
            CatalogueImplSignalsError::CatalogueLoad(layer, reason)
                if layer.as_ref() == "domain"
                    && reason.as_str().contains("declares layer 'usecase'")
                    && reason.as_str().contains("bound to layer 'domain'")
        ),
        "a catalogue layer mismatch must fail closed, got: {err:?}"
    );
}

#[test]
fn test_run_ext_crate_conversion_failure_returns_ext_crate_conversion_error() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor(
        Arc::new(StubLoader { doc }),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(EmptyRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::ExtendedCrateConversion(_, _)),
        "expected ExtendedCrateConversion, got: {err:?}"
    );
}

#[test]
fn test_run_layer_not_found_with_layer_filter_returns_layer_bindings_load_error() {
    // When a layer filter is supplied and the port returns `LayerNotFound`,
    // `run` must map this to `CatalogueImplSignalsError::LayerBindingsLoad`.
    // This covers the `TdddLayerBindingsError::LayerNotFound` branch in the
    // error mapping (the `LoadFailed` branch is covered by the test above).
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(LayerNotFoundLayerBindings { missing_layer_id: "nonexistent".to_owned() }),
    );
    let err = interactor
        .run(
            "my-track".to_owned(),
            std::path::PathBuf::from("/tmp"),
            Some("nonexistent".to_owned()),
        )
        .unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::LayerBindingsLoad(_)),
        "LayerNotFound must map to LayerBindingsLoad, got: {err:?}"
    );
}

#[test]
fn test_run_symlink_guard_rejection_returns_symlink_rejected_error() {
    // When the injected SymlinkGuardPort always rejects, run() must return SymlinkRejected.
    let interactor = build_interactor_with_guard(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![] }),
        Arc::new(EmptyFeatureDeclaration),
        Arc::new(AlwaysRejectSymlinkGuard),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(matches!(
        err,
        CatalogueImplSignalsError::SymlinkRejected(path) if path.as_path() == Path::new("/tmp")
    ));
}

#[test]
fn test_run_symlink_guard_io_preserves_path_and_reason() {
    let interactor = build_interactor_with_guard(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![] }),
        Arc::new(EmptyFeatureDeclaration),
        Arc::new(AlwaysIoSymlinkGuard),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(matches!(
        err,
        CatalogueImplSignalsError::SymlinkGuardIo(path, reason)
            if path.as_path() == Path::new("/tmp") && reason.as_str() == "permission denied"
    ));
}

#[test]
fn test_run_forwards_current_capture_failure_without_fallback() {
    let interactor = build_interactor(
        Arc::new(StubLoader { doc: minimal_catalogue_doc("domain") }),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(CaptureFailureRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![stub_binding("domain")] }),
    );
    let workspace = tempfile::tempdir().unwrap();

    let error =
        interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap_err();

    assert!(matches!(
        error,
        CatalogueImplSignalsError::SchemaExport(layer, reason)
            if layer.as_ref() == "domain"
                && reason.as_str().contains("exclusive rustdoc lock sentinel failure")
    ));
}

#[test]
fn test_run_path_traversal_in_catalogue_file_rejected() {
    let mut binding = stub_binding("domain");
    binding.catalogue_file = "../../../etc/passwd".to_owned();
    let interactor = build_interactor(
        Arc::new(FailingLoader),
        Arc::new(FailingCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
    );
    let err =
        interactor.run("my-track".to_owned(), std::path::PathBuf::from("/tmp"), None).unwrap_err();
    assert!(
        matches!(err, CatalogueImplSignalsError::SymlinkRejected(_)),
        "expected SymlinkRejected for path traversal in catalogue_file, got: {err:?}"
    );
}

#[test]
fn test_run_with_frozen_declaration_exposes_gated_public_item_in_both_captures() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let observed_baseline_features = Arc::new(Mutex::new(Vec::new()));
    let observed_actual_features = Arc::new(Mutex::new(Vec::new()));
    let observed_surfaces = Arc::new(Mutex::new(Vec::new()));
    let frozen_declaration = Arc::new(FrozenFeatureDeclaration);
    let baseline_declaration: Arc<dyn TdddBaselineFeatureDeclarationPort> =
        frozen_declaration.clone();
    let actual_declaration: Arc<dyn TdddActualFeatureDeclarationPort> = frozen_declaration;
    let workspace = tempfile::tempdir().unwrap();

    let baseline_interactor = BaselineCaptureInteractor::new(
        Arc::new(NoopSymlinkGuard),
        Arc::new(StubLayerBindings { bindings: vec![binding.clone()] }),
        Arc::new(RecordingBaselineCapture {
            observed_features: Arc::clone(&observed_baseline_features),
        }),
        baseline_declaration,
    );
    baseline_interactor
        .run(BaselineCaptureRequest {
            track_id: "my-track".to_owned(),
            workspace_root: workspace.path().to_path_buf(),
            source_workspace: None,
            layer: None,
        })
        .unwrap();

    let interactor = build_interactor_with_guard(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(GatedPublicItemEvaluator { observed_surfaces: Arc::clone(&observed_surfaces) }),
        Arc::new(FeatureGatedRustdocPort {
            observed_actual_features: Arc::clone(&observed_actual_features),
        }),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
        actual_declaration,
        Arc::new(NoopSymlinkGuard),
    );

    interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap();

    assert_eq!(
        observed_baseline_features.lock().unwrap().as_slice(),
        [vec!["semantic-dup".to_owned()]],
        "baseline rustdoc acquisition must receive the frozen declaration's feature content"
    );
    assert_eq!(
        observed_actual_features.lock().unwrap().as_slice(),
        [vec!["semantic-dup".to_owned()]],
        "actual rustdoc acquisition must receive the same frozen declaration content"
    );
    assert_eq!(
        observed_surfaces.lock().unwrap().as_slice(),
        [(true, true)],
        "the declared feature-gated item must be present in both baseline and actual surfaces"
    );
}

#[test]
fn test_run_with_undeclared_gated_catalogue_item_returns_non_blue_chain_three_signal() {
    let binding = stub_binding("domain");
    let observed_membership = Arc::new(Mutex::new(Vec::new()));
    let interactor = build_interactor_with_guard(
        Arc::new(StubLoader { doc: minimal_catalogue_doc("domain") }),
        Arc::new(CatalogueGatedItemCodec),
        Arc::new(CatalogueItemMissingFromActualEvaluator {
            observed_membership: Arc::clone(&observed_membership),
        }),
        Arc::new(UndeclaredFeatureRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
        Arc::new(EmptyFeatureDeclaration),
        Arc::new(NoopSymlinkGuard),
    );
    let workspace = tempfile::tempdir().unwrap();

    let report =
        interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap();

    assert_eq!(
        observed_membership.lock().unwrap().as_slice(),
        [(true, false, false)],
        "the undeclared feature must omit the catalogue item from both extracted surfaces"
    );
    assert!(report.any_red, "a non-Blue chain ③ signal must fail closed");
    assert!(
        report.text.contains("🔴 Red"),
        "the report must expose the non-Blue chain ③ signal: {}",
        report.text
    );
}

#[test]
fn test_run_missing_frozen_declaration_stops_before_rustdoc_capture() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor_with_guard(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
        Arc::new(MissingFrozenFeatureDeclaration),
        Arc::new(NoopSymlinkGuard),
    );
    let workspace = tempfile::tempdir().unwrap();

    let error =
        interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap_err();

    assert!(matches!(
        error,
        CatalogueImplSignalsError::FeatureDeclaration(
            TdddActualFeatureDeclarationPortError::MissingBaselineSnapshot { .. }
        )
    ));
}

#[test]
fn test_run_baseline_snapshot_mismatch_returns_feature_declaration_error_before_rustdoc_capture() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor_with_guard(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
        Arc::new(MismatchedFrozenFeatureDeclaration),
        Arc::new(NoopSymlinkGuard),
    );
    let workspace = tempfile::tempdir().unwrap();

    let error =
        interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap_err();

    assert!(matches!(
        error,
        CatalogueImplSignalsError::FeatureDeclaration(
            TdddActualFeatureDeclarationPortError::BaselineSnapshotMismatch
        )
    ));
}

#[test]
fn test_run_invalid_feature_declaration_returns_feature_declaration_error() {
    let binding = stub_binding("domain");
    let doc = minimal_catalogue_doc("domain");
    let interactor = build_interactor_with_guard(
        Arc::new(StubLoader { doc }),
        Arc::new(EmptyExtendedCrateCodec),
        Arc::new(EmptyEvaluator),
        Arc::new(NeverCalledRustdocPort),
        Arc::new(StubLayerBindings { bindings: vec![binding] }),
        Arc::new(InvalidFeatureDeclaration),
        Arc::new(NoopSymlinkGuard),
    );
    let workspace = tempfile::tempdir().unwrap();

    let error =
        interactor.run("my-track".to_owned(), workspace.path().to_path_buf(), None).unwrap_err();

    assert!(matches!(
        error,
        CatalogueImplSignalsError::FeatureDeclaration(TdddActualFeatureDeclarationPortError::Read(
            crate::tddd_feature_declaration::TdddFeatureDeclarationReadError::UnknownCargoFeature { .. }
        ))
    ));
}
