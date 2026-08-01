//! Per-layer type-signal evaluation with conservative rustdoc reuse.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[path = "type_signals_evaluator/build_inputs.rs"]
mod build_inputs;
#[path = "type_signals_evaluator/freshness.rs"]
mod freshness;
#[path = "type_signals_evaluator/inputs.rs"]
mod inputs;
#[path = "type_signals_evaluator/signal_builder.rs"]
mod signal_builder;
#[path = "type_signals_evaluator/signal_tags.rs"]
pub(crate) mod signal_tags;

use domain::tddd::CargoFeatureName;
use domain::tddd::catalogue_v2::CrateName;
use domain::tddd::type_signals_doc::{TypeSignalsDocument, TypeSignalsReuseDecision};
use domain::{Timestamp, TrackId};
use freshness::{
    RustdocJsonPathProvider, existing_rustdoc_content, reuse_decision_for_recorded_document,
};
use inputs::{hash_workspace_inputs, read_utf8_file_limited, verify_evaluation_inputs_unchanged};
use signal_builder::build_type_signals_from_report;
use signal_tags::{contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag};

#[cfg(feature = "test-helpers")]
pub use freshness::RustdocLaunchObserver;

use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::catalogue_document_codec::CatalogueDocumentCodec;
use crate::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;
use crate::tddd::signal_evaluator_v2::SignalEvaluatorV2;
use crate::tddd::type_signals_codec;
use crate::tddd::{CatalogueToExtendedCratePort, SignalEvaluatorPort};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;

const MAX_TYPE_SIGNALS_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUSTDOC_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_CATALOGUE_BYTES: usize = 16 * 1024 * 1024;

/// Error returned when a layer's type signals cannot be evaluated safely.
#[derive(Debug)]
pub struct EvaluateSignalsError(pub String);

impl std::fmt::Display for EvaluateSignalsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// Returns the active track's signal-artifact directory.
pub(crate) fn type_signals_track_dir(items_dir: &Path, track_id: &TrackId) -> PathBuf {
    items_dir.join(track_id.as_ref())
}

/// Rejects a symlinked trust anchor.
pub(crate) fn reject_symlinked_type_signals_anchor(path: &Path, label: &str) -> Result<(), String> {
    crate::track::symlink_guard::reject_symlinks_up_to_root(path).map_err(|error| {
        format!("symlink guard: refusing to use {label} '{}': {error}", path.display())
    })
}

/// Evaluates and writes type signals for one TDDD-enabled layer.
///
/// # Errors
///
/// Returns an error when required files cannot be read, rustdoc cannot be
/// obtained, or the implementation input identity is unavailable.
pub fn execute_type_signals_for_layer(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
) -> Result<ExitCode, EvaluateSignalsError> {
    let exporter = RustdocSchemaExporter::new(workspace_root.to_path_buf());
    execute_with_dependencies(items_dir, track_id, workspace_root, binding, features, &exporter)
}

#[cfg(feature = "test-helpers")]
pub fn execute_type_signals_for_layer_with_launch_observer(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    observer: &RustdocLaunchObserver,
) -> Result<ExitCode, EvaluateSignalsError> {
    execute_with_dependencies(items_dir, track_id, workspace_root, binding, features, observer)
}

fn execute_with_dependencies(
    items_dir: &Path,
    track_id: &TrackId,
    workspace_root: &Path,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    rustdoc: &impl RustdocJsonPathProvider,
) -> Result<ExitCode, EvaluateSignalsError> {
    reject_symlinked_type_signals_anchor(workspace_root, "workspace_root")
        .map_err(EvaluateSignalsError)?;
    reject_symlinked_type_signals_anchor(items_dir, "items_dir").map_err(EvaluateSignalsError)?;
    let canonical_items = items_dir.canonicalize().map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot canonicalize items_dir '{}': {error}",
            items_dir.display()
        ))
    })?;
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        EvaluateSignalsError(format!(
            "cannot canonicalize workspace_root '{}': {error}",
            workspace_root.display()
        ))
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(EvaluateSignalsError(format!(
            "security: items_dir '{}' resolves outside workspace_root '{}'",
            items_dir.display(),
            workspace_root.display()
        )));
    }

    let track_dir = type_signals_track_dir(&canonical_items, track_id);
    reject_type_signals_path(&track_dir, &canonical_items, "track directory")?;
    let catalogue_path = track_dir.join(binding.catalogue_file());
    reject_type_signals_path(&catalogue_path, &canonical_items, "catalogue")?;
    let baseline_path = track_dir.join(binding.baseline_file());
    reject_type_signals_path(&baseline_path, &canonical_items, "baseline")?;
    let signal_path = track_dir.join(binding.signal_file());
    reject_type_signals_path(&signal_path, &canonical_items, "signal artifact")?;
    let catalogue_bytes = inputs::read_bytes_file_limited(&catalogue_path, MAX_CATALOGUE_BYTES)
        .map_err(|error| {
            EvaluateSignalsError(format!(
                "cannot read catalogue '{}': {error}",
                catalogue_path.display()
            ))
        })?;
    let target_crate = match binding.targets() {
        [target] => target.as_str(),
        _ => {
            return Err(EvaluateSignalsError(
                "type-signal layers require exactly one rustdoc target".to_owned(),
            ));
        }
    };
    let declaration_hash = type_signals_codec::declaration_hash(&catalogue_bytes);
    let implementation_hash = hash_workspace_inputs(&canonical_workspace, target_crate, features);
    let recorded = read_utf8_file_limited(&signal_path, MAX_TYPE_SIGNALS_BYTES)
        .ok()
        .and_then(|text| type_signals_codec::decode(&text).ok());
    match reuse_decision_for_recorded_document(
        recorded.as_ref(),
        &declaration_hash,
        implementation_hash.as_ref().ok(),
    ) {
        TypeSignalsReuseDecision::SkipEvaluation => return Ok(ExitCode::SUCCESS),
        TypeSignalsReuseDecision::ReevaluateWithoutExtraction => {
            if let Some(content) = existing_rustdoc_content(rustdoc, target_crate) {
                return evaluate_and_write(
                    &catalogue_bytes,
                    &catalogue_path,
                    &track_dir,
                    &canonical_workspace,
                    &canonical_items,
                    target_crate,
                    binding,
                    features,
                    content,
                    declaration_hash,
                    implementation_hash,
                );
            }
        }
        TypeSignalsReuseDecision::ReextractAndEvaluate => {}
    }
    let target_crate_name = CrateName::new(target_crate).map_err(|error| {
        EvaluateSignalsError(format!("invalid rustdoc target crate '{target_crate}': {error}"))
    })?;
    let json_path =
        rustdoc.export_rustdoc_json_path(&target_crate_name, features).map_err(|error| {
            EvaluateSignalsError(format!("rustdoc export failed for '{target_crate}': {error}"))
        })?;
    let content = read_utf8_file_limited(&json_path, MAX_RUSTDOC_JSON_BYTES).map_err(|error| {
        EvaluateSignalsError(format!("cannot read rustdoc JSON '{}': {error}", json_path.display()))
    })?;
    // The persisted hash must describe the inputs rustdoc actually extracted.
    // When the pre-launch hash was available, require the post-extraction hash
    // to match it — inputs changing DURING extraction would otherwise pair the
    // old rustdoc output with the new hash and let a later run skip wrongly.
    // Only a transiently unavailable pre-launch hash is re-established after
    // extraction, so a repaired input can be recorded with the fresh artifact.
    let implementation_hash = match implementation_hash {
        Ok(initial) => {
            let current = hash_workspace_inputs(&canonical_workspace, target_crate, features)?;
            if current != initial {
                return Err(EvaluateSignalsError(format!(
                    "implementation inputs for '{target_crate}' changed during rustdoc \
                     extraction; re-run the evaluation"
                )));
            }
            Ok(current)
        }
        Err(_) => hash_workspace_inputs(&canonical_workspace, target_crate, features),
    };
    evaluate_and_write(
        &catalogue_bytes,
        &catalogue_path,
        &track_dir,
        &canonical_workspace,
        &canonical_items,
        target_crate,
        binding,
        features,
        content,
        declaration_hash,
        implementation_hash,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_and_write(
    catalogue_bytes: &[u8],
    catalogue_path: &Path,
    track_dir: &Path,
    workspace_root: &Path,
    trusted_items_root: &Path,
    target_crate: &str,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    rustdoc_json: String,
    declaration_hash: domain::CatalogueDeclarationHash,
    implementation_hash: Result<domain::ImplementationInputHash, EvaluateSignalsError>,
) -> Result<ExitCode, EvaluateSignalsError> {
    let name = catalogue_path
        .file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix("-types"))
        .unwrap_or(target_crate);
    let catalogue = CatalogueDocumentCodec::decode(
        std::str::from_utf8(catalogue_bytes)
            .map_err(|error| EvaluateSignalsError(format!("catalogue is not UTF-8: {error}")))?,
        name,
    )
    .map_err(|error| EvaluateSignalsError(format!("cannot decode catalogue: {error}")))?;
    let mut kinds = BTreeMap::new();
    for (name, entry) in catalogue.types() {
        kinds
            .entry(name.as_str().to_owned())
            .or_insert_with(Vec::new)
            .push(data_role_kind_tag(entry.role(), entry.kind()));
    }
    for (name, entry) in catalogue.traits() {
        kinds
            .entry(name.as_str().to_owned())
            .or_insert_with(Vec::new)
            .push(contract_role_kind_tag(entry.role()));
    }
    for (name, entry) in catalogue.functions() {
        kinds
            .entry(name.to_string())
            .or_insert_with(Vec::new)
            .push(function_role_kind_tag(entry.role()));
    }
    let extended = CatalogueToExtendedCrateCodec::new()
        .encode(catalogue)
        .map_err(|error| EvaluateSignalsError(format!("cannot convert catalogue: {error}")))?;
    let baseline_path = track_dir.join(binding.baseline_file());
    reject_type_signals_path(&baseline_path, trusted_items_root, "baseline")?;
    let baseline = BaselineRustdocCodec::from_json(
        &read_utf8_file_limited(&baseline_path, MAX_RUSTDOC_JSON_BYTES)
            .map_err(|error| EvaluateSignalsError(format!("cannot read baseline: {error}")))?,
    )
    .map_err(|error| EvaluateSignalsError(format!("cannot decode baseline: {error}")))?;
    let current = BaselineRustdocCodec::from_json(&rustdoc_json)
        .map_err(|error| EvaluateSignalsError(format!("cannot decode rustdoc JSON: {error}")))?;
    let report = SignalEvaluatorV2::with_workspace_root(workspace_root.to_path_buf())
        .evaluate(extended, baseline, current)
        .map_err(|error| EvaluateSignalsError(format!("signal evaluation failed: {error:?}")))?;
    let implementation_hash = implementation_hash?;
    reject_type_signals_path(catalogue_path, trusted_items_root, "catalogue")?;
    verify_evaluation_inputs_unchanged(
        workspace_root,
        target_crate,
        features,
        catalogue_path,
        &declaration_hash,
        &implementation_hash,
    )?;
    let generated_at = Timestamp::new(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .map_err(|error| EvaluateSignalsError(format!("cannot create timestamp: {error}")))?;
    let document = TypeSignalsDocument::new(
        generated_at,
        declaration_hash,
        implementation_hash,
        build_type_signals_from_report(report.iter(), &kinds),
    );
    let encoded = type_signals_codec::encode(&document)
        .map_err(|error| EvaluateSignalsError(format!("cannot encode type signals: {error}")))?;
    let signal_path = track_dir.join(binding.signal_file());
    reject_type_signals_path(&signal_path, trusted_items_root, "signal artifact")?;
    atomic_write_file(&signal_path, format!("{encoded}\n").as_bytes())
        .map_err(|error| EvaluateSignalsError(format!("cannot write type signals: {error}")))?;
    Ok(ExitCode::SUCCESS)
}

fn reject_type_signals_path(
    path: &Path,
    trusted_items_root: &Path,
    label: &str,
) -> Result<(), EvaluateSignalsError> {
    reject_symlinks_below(path, trusted_items_root).map_err(|error| {
        EvaluateSignalsError(format!(
            "symlink guard rejected {label} '{}': {error}",
            path.display()
        ))
    })?;
    Ok(())
}

#[cfg(all(test, feature = "test-helpers"))]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::verify::tddd_layers::parse_tddd_layers;

    fn rustdoc_json() -> String {
        format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        )
    }

    fn setup_workspace() -> (tempfile::TempDir, PathBuf, TrackId, TdddLayerBinding, PathBuf) {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let track_id = TrackId::try_new("feature-input-track").unwrap();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        std::fs::create_dir_all(root.join("libs/infrastructure/src")).unwrap();
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/infrastructure\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("Cargo.lock"),
            "version = 4\n\n[[package]]\nname = \"infrastructure\"\nversion = \"0.1.0\"\n",
        )
        .unwrap();
        std::fs::write(
            root.join("libs/infrastructure/Cargo.toml"),
            "[package]\nname = \"infrastructure\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[features]\nsemantic-dup = []\n",
        )
        .unwrap();
        std::fs::write(root.join("libs/infrastructure/src/lib.rs"), "pub struct Fixture;\n")
            .unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            "{\n  \"schema_version\": 5,\n  \"crate_name\": \"infrastructure\",\n  \"layer\": \"infrastructure\",\n  \"types\": {},\n  \"traits\": {},\n  \"functions\": {}\n}\n",
        )
        .unwrap();
        let rustdoc_path = root.join("infrastructure-rustdoc.json");
        let json = rustdoc_json();
        std::fs::write(&rustdoc_path, &json).unwrap();
        std::fs::write(track_dir.join("infrastructure-types-baseline.json"), json).unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [{
                "crate": "infrastructure",
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "infrastructure-types.json",
                    "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                }
            }]
        }"#;
        let binding = parse_tddd_layers(rules).unwrap().pop().unwrap();

        (workspace, items_dir, track_id, binding, rustdoc_path)
    }

    fn nightly_toolchain_available() -> bool {
        std::process::Command::new("rustup")
            .args(["run", "nightly", "rustc", "-Vv"])
            .status()
            .is_ok_and(|status| status.success())
    }

    #[test]
    fn test_execute_type_signals_feature_selection_change_reextracts_and_reaches_rustdoc() {
        if !nightly_toolchain_available() {
            eprintln!(
                "skipping feature-selection evaluator test: nightly toolchain is unavailable"
            );
            return;
        }
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let declared_feature = CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            std::slice::from_ref(&declared_feature),
            &observer,
        )
        .unwrap();
        let signal_path =
            items_dir.join(track_id.as_ref()).join("infrastructure-type-signals.json");
        let first =
            type_signals_codec::decode(&std::fs::read_to_string(&signal_path).unwrap()).unwrap();

        execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            &[],
            &observer,
        )
        .unwrap();
        let second =
            type_signals_codec::decode(&std::fs::read_to_string(signal_path).unwrap()).unwrap();

        assert_eq!(
            observer.feature_selections_for("infrastructure"),
            vec![vec!["semantic-dup".to_owned()], Vec::new()],
            "each measured rustdoc extraction must observe its declared feature selection"
        );
        assert_eq!(observer.launches_for("infrastructure"), 2);
        assert_ne!(
            first.implementation_input_hash(),
            second.implementation_input_hash(),
            "changing the declaration-derived feature selection must not reuse a stale artifact"
        );
    }
}
