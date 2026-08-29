//! Per-layer type-signal evaluation with conservative rustdoc reuse.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

#[path = "type_signals_evaluator/freshness.rs"]
pub(crate) mod freshness;
#[path = "type_signals_evaluator/inputs.rs"]
pub(crate) mod inputs;
#[path = "type_signals_evaluator/signal_builder.rs"]
mod signal_builder;
#[path = "type_signals_evaluator/signal_tags.rs"]
pub(crate) mod signal_tags;

use domain::tddd::CargoFeatureName;
use domain::tddd::LayerId;
use domain::tddd::catalogue_v2::{AttestedCatalogueDocument, CrateName};
use domain::tddd::type_signals_doc::{BaselineHash, TypeSignalsCacheKey, TypeSignalsDocument};
use domain::{FreeText, Timestamp, TrackId};
use freshness::{RustdocJsonPathProvider, decide_reuse_for_recorded_document};
use inputs::{
    read_head_commit, read_utf8_file_limited, verify_evaluation_inputs_unchanged, worktree_is_clean,
};
use signal_builder::{build_type_signal_identity_index, build_type_signals_from_report};
use signal_tags::{contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag};

#[cfg(feature = "test-helpers")]
pub use freshness::RustdocLaunchObserver;

#[cfg(test)]
static PROCESS_ENVIRONMENT_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

#[cfg(test)]
pub(crate) fn with_process_environment_lock<T>(action: impl FnOnce() -> T) -> T {
    let _environment_guard = match PROCESS_ENVIRONMENT_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    action()
}

use crate::schema_export::RustdocSchemaExporter;
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::catalogue_document_codec::{CatalogueDocumentCodec, derive_filename_stem};
use crate::tddd::catalogue_to_extended_crate_codec::{
    CatalogueToExtendedCrateCodec, normalized_paths_for_doc, resolution_paths_for_catalogue,
};
#[cfg(all(test, feature = "test-helpers"))]
use crate::tddd::catalogue_to_extended_crate_codec::{
    encode_document, resolution_paths_for_document,
};
use crate::tddd::signal_evaluator_v2::SignalEvaluatorV2;
use crate::tddd::type_signals_codec;
use crate::tddd::{CatalogueToExtendedCratePort, SignalEvaluatorPort};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::TdddLayerBinding;

const MAX_TYPE_SIGNALS_BYTES: usize = 16 * 1024 * 1024;
const MAX_RUSTDOC_JSON_BYTES: usize = 64 * 1024 * 1024;
const MAX_CATALOGUE_BYTES: usize = 16 * 1024 * 1024;

#[derive(Debug)]
struct LoadedTrackCatalogues {
    bindings: Vec<TdddLayerBinding>,
    catalogues: BTreeMap<LayerId, AttestedCatalogueDocument>,
}

/// Error returned when a layer's type signals cannot be evaluated safely.
#[derive(Debug)]
pub enum EvaluateSignalsError {
    /// An authoritative input could not be acquired or verified.
    AuthoritativeInput(FreeText),
    /// Rustdoc extraction or signal evaluation could not complete.
    Evaluation(FreeText),
    /// A freshly evaluated cache document could not be persisted.
    CacheWrite(FreeText),
}

impl EvaluateSignalsError {
    fn authoritative_input(message: impl Into<String>) -> Self {
        Self::AuthoritativeInput(FreeText::new(message))
    }

    fn evaluation(message: impl Into<String>) -> Self {
        Self::Evaluation(FreeText::new(message))
    }

    fn cache_write(message: impl Into<String>) -> Self {
        Self::CacheWrite(FreeText::new(message))
    }
}

impl std::fmt::Display for EvaluateSignalsError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::AuthoritativeInput(message)
            | Self::Evaluation(message)
            | Self::CacheWrite(message) => formatter.write_str(message.as_str()),
        }
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

fn load_track_catalogues(
    workspace_root: &Path,
    track_dir: &Path,
    trusted_items_root: &Path,
) -> Result<LoadedTrackCatalogues, EvaluateSignalsError> {
    let bindings = crate::verify::tddd_layers::load_tddd_layers_from_workspace(workspace_root)
        .map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot load TDDD layer bindings: {error}"
            ))
        })?;
    let mut catalogues = BTreeMap::new();
    for binding in &bindings {
        let path = track_dir.join(binding.catalogue_file());
        reject_type_signals_path(&path, trusted_items_root, "catalogue")?;
        let Some(bytes) = read_configured_catalogue(&path, trusted_items_root)? else {
            continue;
        };
        let name = derive_filename_stem(&path);
        let document = AttestedCatalogueDocument::attest(&bytes, |source| {
            let text = std::str::from_utf8(source).map_err(|error| error.to_string())?;
            CatalogueDocumentCodec::decode(text, &name).map_err(|error| error.to_string())
        })
        .map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot decode catalogue '{}': {error}",
                path.display()
            ))
        })?;
        let expected_layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "invalid layer id '{}': {error}",
                binding.layer_id()
            ))
        })?;
        if document.document().layer() != &expected_layer {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "catalogue '{}' declares layer '{}' but architecture binding selects '{}'",
                path.display(),
                document.document().layer(),
                expected_layer
            )));
        }
        if catalogues.insert(expected_layer.clone(), document).is_some() {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "duplicate effective TDDD catalogue layer '{}'",
                expected_layer
            )));
        }
    }
    Ok(LoadedTrackCatalogues { bindings, catalogues })
}

/// Hashes architecture-rules.json and every configured catalogue file.
pub(super) fn resolution_input_fingerprint(
    workspace_root: &Path,
    track_dir: &Path,
    trusted_items_root: &Path,
) -> Result<domain::CatalogueDeclarationHash, EvaluateSignalsError> {
    let mut input = Vec::new();
    let rules_path = workspace_root.join("architecture-rules.json");
    match crate::track::symlink_guard::reject_symlinks_below(&rules_path, workspace_root) {
        Ok(true) => {}
        Ok(false) => {
            return Err(EvaluateSignalsError::authoritative_input(
                "architecture-rules.json not found".to_owned(),
            ));
        }
        Err(error) => {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "symlink guard rejected architecture-rules.json '{}': {error}",
                rules_path.display()
            )));
        }
    }
    let rules = read_workspace_file(&rules_path, workspace_root, "architecture-rules.json")?
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(
                "architecture-rules.json not found".to_owned(),
            )
        })?;
    encode_present_bytes(&mut input, &rules);
    let bindings = crate::verify::tddd_layers::load_tddd_layers_from_workspace(workspace_root)
        .map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot load TDDD layer bindings: {error}"
            ))
        })?;
    for binding in bindings {
        input.extend_from_slice(binding.layer_id().as_bytes());
        let path = track_dir.join(binding.catalogue_file());
        reject_type_signals_path(&path, trusted_items_root, "catalogue")?;
        match read_configured_catalogue(&path, trusted_items_root)? {
            Some(bytes) => encode_present_bytes(&mut input, &bytes),
            None => input.push(0),
        }
    }
    Ok(type_signals_codec::declaration_hash(&input))
}

fn encode_present_bytes(input: &mut Vec<u8>, bytes: &[u8]) {
    input.push(1);
    input.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    input.extend_from_slice(bytes);
}

/// Reads one configured track catalogue only when it is present.
///
/// The caller must already have applied the symlink guard. The open itself
/// uses the shared no-follow catalogue primitive so a concurrent replacement
/// cannot turn a validated leaf into a symlink or FIFO.
fn read_configured_catalogue(
    path: &Path,
    trusted_items_root: &Path,
) -> Result<Option<Vec<u8>>, EvaluateSignalsError> {
    read_workspace_file(path, trusted_items_root, "catalogue")
}

fn read_workspace_file(
    path: &Path,
    trusted_root: &Path,
    label: &str,
) -> Result<Option<Vec<u8>>, EvaluateSignalsError> {
    crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
        path,
        Some(trusted_root),
        MAX_CATALOGUE_BYTES as u64,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read {label} '{}': {error}",
            path.display()
        ))
    })
}

/// Evaluates and writes type signals for one TDDD-enabled layer.
///
/// # Errors
///
/// Returns an error when required files cannot be read or rustdoc cannot be
/// obtained safely.
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
pub(crate) fn execute_type_signals_for_layer_with_launch_observer(
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
        .map_err(EvaluateSignalsError::authoritative_input)?;
    reject_symlinked_type_signals_anchor(items_dir, "items_dir")
        .map_err(EvaluateSignalsError::authoritative_input)?;
    let canonical_items = items_dir.canonicalize().map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot canonicalize items_dir '{}': {error}",
            items_dir.display()
        ))
    })?;
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot canonicalize workspace_root '{}': {error}",
            workspace_root.display()
        ))
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(EvaluateSignalsError::authoritative_input(format!(
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
    let catalogue_bytes = read_configured_catalogue(&catalogue_path, &canonical_items)?
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot read catalogue '{}': file not found",
                catalogue_path.display()
            ))
        })?;
    let target_crate = match binding.targets() {
        [target] => target.as_str(),
        _ => {
            return Err(EvaluateSignalsError::authoritative_input(
                "type-signal layers require exactly one rustdoc target".to_owned(),
            ));
        }
    };
    let declaration_hash = type_signals_codec::declaration_hash(&catalogue_bytes);
    let (baseline_json, baseline_hash) = read_actual_baseline(&baseline_path)?;
    let head_commit = read_head_commit(&canonical_workspace)?;
    let current_key = TypeSignalsCacheKey::new(
        declaration_hash.clone(),
        head_commit.clone(),
        baseline_hash.clone(),
    );
    let recorded = read_utf8_file_limited(&signal_path, MAX_TYPE_SIGNALS_BYTES)
        .ok()
        .and_then(|text| type_signals_codec::decode(&text).ok());
    let cache_decision_start_resolution =
        resolution_input_fingerprint(&canonical_workspace, &track_dir, &canonical_items)?;
    let configured_layers = crate::verify::tddd_layers::load_tddd_layers_from_workspace(
        &canonical_workspace,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot load TDDD layer bindings: {error}"
        ))
    })?;
    validate_evaluator_binding(&configured_layers, binding)?;
    let reuse_decision = if configured_layers.len() > 1 {
        domain::TypeSignalsReuseDecision::ReextractAndEvaluate
    } else {
        decide_reuse_for_recorded_document(
            recorded.as_ref(),
            &current_key,
            worktree_is_clean(&canonical_workspace)?,
        )
    };
    match reuse_decision {
        domain::TypeSignalsReuseDecision::SkipEvaluation => {
            verify_evaluation_inputs_unchanged(
                &canonical_workspace,
                &catalogue_path,
                &baseline_path,
                &current_key,
            )?;
            let cache_decision_end_resolution =
                resolution_input_fingerprint(&canonical_workspace, &track_dir, &canonical_items)?;
            if cache_decision_start_resolution != cache_decision_end_resolution {
                return Err(EvaluateSignalsError::authoritative_input(
                    "architecture-rules or track catalogues changed during type-signal cache decision"
                        .to_owned(),
                ));
            }
            return Ok(ExitCode::SUCCESS);
        }
        // Cargo's shared rustdoc output path is not keyed by the cache identity
        // or feature selection. Re-extract rather than trusting an unrelated
        // producer's valid JSON document.
        domain::TypeSignalsReuseDecision::ReevaluateWithoutExtraction => {}
        domain::TypeSignalsReuseDecision::ReextractAndEvaluate => {}
    }
    let target_crate_name = CrateName::new(target_crate).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "invalid rustdoc target crate '{target_crate}': {error}"
        ))
    })?;
    let json_path =
        rustdoc.export_rustdoc_json_path(&target_crate_name, features).map_err(|error| {
            EvaluateSignalsError::evaluation(format!(
                "rustdoc export failed for '{target_crate}': {error}"
            ))
        })?;
    let content = read_utf8_file_limited(&json_path, MAX_RUSTDOC_JSON_BYTES).map_err(|error| {
        EvaluateSignalsError::evaluation(format!(
            "cannot read rustdoc JSON '{}': {error}",
            json_path.display()
        ))
    })?;
    evaluate_and_write(
        &catalogue_bytes,
        &catalogue_path,
        &track_dir,
        &canonical_workspace,
        &canonical_items,
        binding,
        content,
        head_commit,
        &baseline_path,
        &baseline_json,
        baseline_hash,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluate_and_write(
    catalogue_bytes: &[u8],
    catalogue_path: &Path,
    track_dir: &Path,
    workspace_root: &Path,
    trusted_items_root: &Path,
    binding: &TdddLayerBinding,
    rustdoc_json: String,
    head_commit: domain::CommitHash,
    baseline_path: &Path,
    baseline_json: &str,
    baseline_hash: BaselineHash,
) -> Result<ExitCode, EvaluateSignalsError> {
    reject_type_signals_path(baseline_path, trusted_items_root, "baseline")?;
    let baseline = BaselineRustdocCodec::from_json(baseline_json).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("cannot decode baseline: {error}"))
    })?;
    let current = BaselineRustdocCodec::from_json(&rustdoc_json).map_err(|error| {
        EvaluateSignalsError::evaluation(format!("cannot decode rustdoc JSON: {error}"))
    })?;
    let target_layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
    })?;
    let start_resolution =
        resolution_input_fingerprint(workspace_root, track_dir, trusted_items_root)?;
    let LoadedTrackCatalogues { bindings, catalogues } =
        load_track_catalogues(workspace_root, track_dir, trusted_items_root)?;
    validate_evaluator_binding(&bindings, binding)?;
    let target_catalogue = catalogues.get(&target_layer).ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "target catalogue for layer '{}' is not present in the current resolution set",
            target_layer
        ))
    })?;
    let supplied_declaration_hash = type_signals_codec::declaration_hash(catalogue_bytes);
    if target_catalogue.declaration_hash() != &supplied_declaration_hash {
        return Err(EvaluateSignalsError::authoritative_input(
            "target catalogue bytes changed between the initial load and the attested resolution read"
                .to_owned(),
        ));
    }
    let declaration_hash = target_catalogue.declaration_hash().clone();
    let catalogue = target_catalogue.document().clone();
    let track_catalogues = catalogues
        .into_iter()
        .map(|(layer, attested)| (layer, attested.into_document()))
        .collect::<BTreeMap<_, _>>();
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
    let identity_paths =
        resolution_paths_for_catalogue(&target_layer, &track_catalogues, &baseline, &current)
            .map_err(|error| EvaluateSignalsError::evaluation(error.to_string()))?;
    let identity_index = build_type_signal_identity_index(&catalogue, &identity_paths)
        .map_err(EvaluateSignalsError::evaluation)?;
    let canonical_baseline_paths = normalized_paths_for_doc(&baseline, catalogue.crate_name());
    let canonical_current_paths = normalized_paths_for_doc(&current, catalogue.crate_name());
    let canonical_baseline = crate_with_canonical_paths(&baseline, canonical_baseline_paths);
    let canonical_current = crate_with_canonical_paths(&current, canonical_current_paths);
    let extended = CatalogueToExtendedCrateCodec::new()
        .encode(&target_layer, &track_catalogues, &baseline, &current)
        .map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!("cannot convert catalogue: {error}"))
        })?;
    let report = SignalEvaluatorV2::with_workspace_root(workspace_root.to_path_buf())
        .evaluate(extended, canonical_baseline, canonical_current)
        .map_err(|error| {
            EvaluateSignalsError::evaluation(format!("signal evaluation failed: {error:?}"))
        })?;
    reject_type_signals_path(catalogue_path, trusted_items_root, "catalogue")?;
    verify_evaluation_inputs_unchanged(
        workspace_root,
        catalogue_path,
        baseline_path,
        &TypeSignalsCacheKey::new(
            declaration_hash.clone(),
            head_commit.clone(),
            baseline_hash.clone(),
        ),
    )?;
    let end_resolution =
        resolution_input_fingerprint(workspace_root, track_dir, trusted_items_root)?;
    if start_resolution != end_resolution {
        return Err(EvaluateSignalsError::authoritative_input(
            "architecture-rules or track catalogues changed during type-signal evaluation"
                .to_owned(),
        ));
    }
    let generated_at = Timestamp::new(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .map_err(|error| {
            EvaluateSignalsError::evaluation(format!("cannot create timestamp: {error}"))
        })?;
    let document = TypeSignalsDocument::new(
        generated_at,
        TypeSignalsCacheKey::new(declaration_hash, head_commit, baseline_hash),
        build_type_signals_from_report(report.iter(), &kinds, &identity_index),
    );
    let encoded = type_signals_codec::encode(&document).map_err(|error| {
        EvaluateSignalsError::cache_write(format!("cannot encode type signals: {error}"))
    })?;
    let signal_path = track_dir.join(binding.signal_file());
    reject_type_signals_path(&signal_path, trusted_items_root, "signal artifact")?;
    atomic_write_file(&signal_path, format!("{encoded}\n").as_bytes()).map_err(|error| {
        EvaluateSignalsError::cache_write(format!("cannot write type signals: {error}"))
    })?;
    Ok(ExitCode::SUCCESS)
}

fn evaluator_bindings_match(left: &TdddLayerBinding, right: &TdddLayerBinding) -> bool {
    left.layer_id() == right.layer_id()
        && left.catalogue_file() == right.catalogue_file()
        && left.targets() == right.targets()
}

fn validate_evaluator_binding(
    configured_bindings: &[TdddLayerBinding],
    binding: &TdddLayerBinding,
) -> Result<(), EvaluateSignalsError> {
    let configured_binding = configured_bindings
        .iter()
        .find(|candidate| candidate.layer_id() == binding.layer_id())
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "type-signal layer '{}' is not configured in architecture-rules.json",
                binding.layer_id()
            ))
        })?;
    if !evaluator_bindings_match(configured_binding, binding) {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "type-signal binding for layer '{}' does not match architecture-rules.json",
            binding.layer_id()
        )));
    }
    Ok(())
}

fn crate_with_canonical_paths(
    krate: &rustdoc_types::Crate,
    paths: std::collections::HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>,
) -> rustdoc_types::Crate {
    let mut canonical = krate.clone();
    canonical.paths = paths;
    if let Some(root) = canonical.index.get_mut(&canonical.root) {
        if let Some(root_name) = canonical
            .paths
            .values()
            .find(|summary| summary.crate_id == 0)
            .and_then(|summary| summary.path.first())
        {
            root.name = Some(root_name.clone());
        }
    }
    canonical
}

fn reject_type_signals_path(
    path: &Path,
    trusted_items_root: &Path,
    label: &str,
) -> Result<(), EvaluateSignalsError> {
    reject_symlinks_below(path, trusted_items_root).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "symlink guard rejected {label} '{}': {error}",
            path.display()
        ))
    })?;
    Ok(())
}

fn read_actual_baseline(path: &Path) -> Result<(String, BaselineHash), EvaluateSignalsError> {
    let bytes = inputs::read_bytes_file_limited(path, MAX_RUSTDOC_JSON_BYTES).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read baseline '{}': {error}",
            path.display()
        ))
    })?;
    let baseline_json = String::from_utf8(bytes).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "baseline '{}' is not UTF-8: {error}",
            path.display()
        ))
    })?;
    BaselineRustdocCodec::from_json(&baseline_json).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot decode baseline '{}': {error}",
            path.display()
        ))
    })?;
    let hash = type_signals_codec::baseline_hash(baseline_json.as_bytes());
    Ok((baseline_json, hash))
}

#[cfg(test)]
#[cfg(feature = "test-helpers")]
#[allow(clippy::expect_used, clippy::indexing_slicing, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::tddd::ThreeWaySignal;
    use crate::verify::tddd_layers::parse_tddd_layers;
    use domain::FreeText;
    use domain::tddd::catalogue_v2::CatalogueDocument;
    use usecase::merge_gate::{BlobFetchResult, TrackBlobReader};

    fn rustdoc_json() -> String {
        format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        )
    }

    fn rustdoc_crate_with_paths(
        paths: HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>,
    ) -> rustdoc_types::Crate {
        rustdoc_types::Crate {
            root: rustdoc_types::Id(0),
            crate_version: None,
            includes_private: false,
            index: HashMap::new(),
            paths,
            external_crates: HashMap::new(),
            target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
            format_version: rustdoc_types::FORMAT_VERSION,
        }
    }

    #[test]
    fn test_type_signal_identity_index_uses_catalogue_adds_from_shared_resolution_set() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::identifiers::CatalogueItemNamespace;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{CatalogueDocument, CatalogueEntryKey};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("AddedOnlyInCatalogue".to_owned()).unwrap(),
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

        let baseline = rustdoc_crate_with_paths(HashMap::new());
        let current = baseline.clone();
        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &current)
            .expect("the shared resolution set must include catalogue add declarations");
        let index = build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("type-signal identity indexing must consume catalogue additions");

        let kinds = BTreeMap::from([("AddedOnlyInCatalogue".to_owned(), vec!["struct"])]);
        let signals = [ThreeWaySignal::catalogue_item(
            FreeText::new("domain::AddedOnlyInCatalogue"),
            CatalogueItemNamespace::Type,
            domain::tddd::signal_evaluator::region::SignalRegion::SIntersectC_Match_Add,
        )];
        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "AddedOnlyInCatalogue");
    }

    #[test]
    fn test_type_signal_identity_index_resolves_add_impl_owner_from_shared_resolution_set() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::CatalogueEntryKey;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::{TraitEntry, TypeEntry};
        use domain::tddd::catalogue_v2::roles::{ContractRole, DataRole, ItemAction};
        use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
        use domain::tddd::catalogue_v2::{CrateName, TypeRef};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Owner".to_owned()).unwrap(),
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
        catalogue.insert_trait(
            CatalogueEntryKey::try_new("NewTrait".to_owned()).unwrap(),
            TraitEntry::new(
                ItemAction::Add,
                ContractRole::SpecificationPort,
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            ItemAction::Add,
            TypeRef::new("NewTrait".to_owned()).unwrap(),
            TypeRef::new("Owner".to_owned()).unwrap(),
            vec![],
            vec![],
        ));

        let baseline = rustdoc_crate_with_paths(HashMap::new());
        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &baseline)
            .expect("the shared resolution set must include both add declarations");
        let index = build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("an add impl owner must resolve through the shared set");
        let kinds = BTreeMap::from([("Owner".to_owned(), vec!["struct"])]);
        let signals = [ThreeWaySignal::label(
            FreeText::new("Owner: NewTrait"),
            domain::tddd::signal_evaluator::region::SignalRegion::SIntersectC_Match_Add,
        )];

        let built = build_type_signals_from_report(signals.iter(), &kinds, &index);
        assert_eq!(built.len(), 1);
        assert_eq!(built[0].type_name(), "Owner");
        assert_eq!(built[0].found_items(), &["NewTrait"]);
    }

    #[test]
    fn test_type_signal_identity_index_rejects_impl_owner_absent_from_shared_resolution_set() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::traits::TraitImplDeclV2;
        use domain::tddd::catalogue_v2::{CrateName, TypeRef};

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        catalogue.push_trait_impl(TraitImplDeclV2::from_parts(
            domain::tddd::catalogue_v2::ItemAction::Add,
            TypeRef::new("MissingTrait").unwrap(),
            TypeRef::new("MissingOwner").unwrap(),
            vec![],
            vec![],
        ));

        let empty = rustdoc_crate_with_paths(HashMap::new());
        let error = build_type_signal_identity_index(&catalogue, &empty.paths)
            .expect_err("an impl owner absent from both authorities must fail closed");
        assert!(error.contains("MissingOwner"));
    }

    #[test]
    fn test_execute_type_signals_evaluates_shared_catalogue_identity_and_function_path() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::entries::FunctionEntry;
        use domain::tddd::catalogue_v2::roles::FunctionRole;
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CatalogueEntryKey, CrateName, FunctionName, FunctionPath, ModulePath,
        };

        let make_rustdoc = |root_name: &str, include_items: bool, include_removed: bool| {
            let root_id = rustdoc_types::Id(0);
            let type_id = rustdoc_types::Id(1);
            let function_id = rustdoc_types::Id(2);
            let removed_id = rustdoc_types::Id(3);
            let mut index = HashMap::new();
            let mut paths = HashMap::new();
            let mut root_items = Vec::new();

            if include_items {
                index.insert(
                    type_id,
                    rustdoc_types::Item {
                        id: type_id,
                        crate_id: 0,
                        name: Some("Added".to_owned()),
                        span: None,
                        visibility: rustdoc_types::Visibility::Public,
                        docs: None,
                        links: HashMap::new(),
                        attrs: vec![],
                        deprecation: None,
                        inner: rustdoc_types::ItemEnum::Struct(rustdoc_types::Struct {
                            kind: rustdoc_types::StructKind::Unit,
                            generics: rustdoc_types::Generics {
                                params: vec![],
                                where_predicates: vec![],
                            },
                            impls: vec![],
                        }),
                    },
                );
                paths.insert(
                    type_id,
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: vec![
                            root_name.to_owned(),
                            "generated".to_owned(),
                            "Added".to_owned(),
                        ],
                        kind: rustdoc_types::ItemKind::Struct,
                    },
                );
                index.insert(
                    function_id,
                    rustdoc_types::Item {
                        id: function_id,
                        crate_id: 0,
                        name: Some("run".to_owned()),
                        span: None,
                        visibility: rustdoc_types::Visibility::Public,
                        docs: None,
                        links: HashMap::new(),
                        attrs: vec![],
                        deprecation: None,
                        inner: rustdoc_types::ItemEnum::Function(rustdoc_types::Function {
                            sig: rustdoc_types::FunctionSignature {
                                inputs: vec![],
                                output: None,
                                is_c_variadic: false,
                            },
                            generics: rustdoc_types::Generics {
                                params: vec![],
                                where_predicates: vec![],
                            },
                            has_body: true,
                            header: rustdoc_types::FunctionHeader {
                                is_async: false,
                                is_const: false,
                                is_unsafe: false,
                                abi: rustdoc_types::Abi::Rust,
                            },
                        }),
                    },
                );
                paths.insert(
                    function_id,
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: vec![root_name.to_owned(), "commands".to_owned(), "run".to_owned()],
                        kind: rustdoc_types::ItemKind::Function,
                    },
                );
                root_items.extend([type_id, function_id]);
            }

            if include_removed {
                index.insert(
                    removed_id,
                    rustdoc_types::Item {
                        id: removed_id,
                        crate_id: 0,
                        name: Some("Removed".to_owned()),
                        span: None,
                        visibility: rustdoc_types::Visibility::Public,
                        docs: None,
                        links: HashMap::new(),
                        attrs: vec![],
                        deprecation: None,
                        inner: rustdoc_types::ItemEnum::Struct(rustdoc_types::Struct {
                            kind: rustdoc_types::StructKind::Unit,
                            generics: rustdoc_types::Generics {
                                params: vec![],
                                where_predicates: vec![],
                            },
                            impls: vec![],
                        }),
                    },
                );
                paths.insert(
                    removed_id,
                    rustdoc_types::ItemSummary {
                        crate_id: 0,
                        path: vec![root_name.to_owned(), "Removed".to_owned()],
                        kind: rustdoc_types::ItemKind::Struct,
                    },
                );
                root_items.push(removed_id);
            }

            index.insert(
                root_id,
                rustdoc_types::Item {
                    id: root_id,
                    crate_id: 0,
                    name: Some(root_name.to_owned()),
                    span: None,
                    visibility: rustdoc_types::Visibility::Public,
                    docs: None,
                    links: HashMap::new(),
                    attrs: vec![],
                    deprecation: None,
                    inner: rustdoc_types::ItemEnum::Module(rustdoc_types::Module {
                        is_crate: true,
                        items: root_items,
                        is_stripped: false,
                    }),
                },
            );

            rustdoc_types::Crate {
                root: root_id,
                crate_version: None,
                includes_private: false,
                index,
                paths,
                external_crates: HashMap::new(),
                target: rustdoc_types::Target { triple: String::new(), target_features: vec![] },
                format_version: rustdoc_types::FORMAT_VERSION,
            }
        };

        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        crate::verify::test_support::git_init(root);
        let track_id = TrackId::try_new("shared-identity-track").unwrap();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nmembers = [\"libs/infrastructure\"]\nresolver = \"2\"\n",
        )
        .unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        std::fs::create_dir_all(root.join("libs/infrastructure/src")).unwrap();
        std::fs::write(
            root.join("libs/infrastructure/Cargo.toml"),
            "[package]\nname = \"infrastructure\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .unwrap();
        std::fs::write(root.join("libs/infrastructure/src/lib.rs"), "pub struct Fixture;\n")
            .unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [{
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "infrastructure-types.json",
                    "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                }
            }]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        crate::verify::test_support::run_git(root, &["add", "."]);
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);
        let binding = parse_tddd_layers(rules).unwrap().pop().unwrap();

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Added".to_owned()).unwrap(),
            domain::tddd::catalogue_v2::entries::TypeEntry::new(
                domain::tddd::catalogue_v2::ItemAction::Add,
                domain::tddd::catalogue_v2::roles::DataRole::value_object(),
                domain::tddd::catalogue_v2::composite::TypeKindV2::Struct(
                    domain::tddd::catalogue_v2::composite::StructKind::new(
                        domain::tddd::catalogue_v2::composite::StructShape::Unit,
                        None,
                    ),
                ),
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            ),
        );
        catalogue.push_deletion(domain::tddd::catalogue_v2::DeletionRecord::Type {
            name: CatalogueEntryKey::try_new("Removed".to_owned()).unwrap(),
            spec_refs: vec![],
            informal_grounds: vec![],
        });
        catalogue.insert_function(
            FunctionPath::new(
                CrateName::new("infrastructure").unwrap(),
                ModulePath::from_segments(vec!["commands".to_owned()]).unwrap(),
                FunctionName::new("run").unwrap(),
            ),
            FunctionEntry::new(
                domain::tddd::catalogue_v2::ItemAction::Add,
                FunctionRole::FreeFunction,
                vec![],
                domain::tddd::catalogue_v2::TypeRef::new("()").unwrap(),
                false,
                vec![],
                vec![],
                None,
                vec![],
                vec![],
            ),
        );

        let catalogue_json = CatalogueDocumentCodec::encode(&catalogue).unwrap();
        let baseline = make_rustdoc("sotp", false, true);
        let current = make_rustdoc("sotp", true, false);
        let baseline_json = serde_json::to_string(&baseline).unwrap();
        let current_json = serde_json::to_string(&current).unwrap();
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let baseline_path = track_dir.join(binding.baseline_file());
        std::fs::write(&catalogue_path, &catalogue_json).unwrap();
        std::fs::write(&baseline_path, &baseline_json).unwrap();

        let result = evaluate_and_write(
            catalogue_json.as_bytes(),
            &catalogue_path,
            &track_dir,
            root,
            &items_dir.canonicalize().unwrap(),
            &binding,
            current_json,
            read_head_commit(root).unwrap(),
            &baseline_path,
            &baseline_json,
            type_signals_codec::baseline_hash(baseline_json.as_bytes()),
        )
        .unwrap();
        assert_eq!(result, ExitCode::SUCCESS);

        let persisted = type_signals_codec::decode(
            &std::fs::read_to_string(track_dir.join(binding.signal_file())).unwrap(),
        )
        .unwrap();
        assert!(
            persisted.signals().iter().any(|signal| signal.type_name() == "Added"),
            "catalogue additions must reach the production type-signal pipeline"
        );
        assert!(
            persisted
                .signals()
                .iter()
                .any(|signal| signal.type_name() == "infrastructure::commands::run"),
            "function identity must retain the canonical package path"
        );
        assert!(
            persisted.signals().iter().any(|signal| signal.type_name() == "Removed"),
            "deletion handling must retain the baseline identity through Phase 1"
        );
    }

    #[test]
    fn test_evaluate_and_write_loads_all_track_catalogues_for_encode() {
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{
            CatalogueDocument, CatalogueEntryKey, CrateName, FieldDecl, FieldName, ModulePath,
            TypeRef,
        };

        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        crate::verify::test_support::git_init(root);
        let track_id = TrackId::try_new("cross-crate-handoff-track").unwrap();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join(track_id.as_ref());
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(root.join("Cargo.toml"), "[workspace]\nmembers = []\nresolver = \"2\"\n")
            .unwrap();
        std::fs::write(root.join("Cargo.lock"), "version = 4\n").unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "domain-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
                    }
                },
                {
                    "crate": "infrastructure",
                    "path": "libs/infrastructure",
                    "may_depend_on": ["domain"],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "infrastructure-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                    }
                }
            ]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        crate::verify::test_support::run_git(root, &["add", "."]);
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);
        let binding = parse_tddd_layers(rules)
            .unwrap()
            .into_iter()
            .find(|layer| layer.layer_id() == "infrastructure")
            .unwrap();

        let mut target = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        target.insert_type(
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

        let mut declaring = CatalogueDocument::new(
            5,
            CrateName::new("domain").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        declaring.insert_type(
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

        let catalogue_json = CatalogueDocumentCodec::encode(&target).unwrap();
        let declaring_json = CatalogueDocumentCodec::encode(&declaring).unwrap();
        let baseline_json = rustdoc_json();
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let baseline_path = track_dir.join(binding.baseline_file());
        std::fs::write(&catalogue_path, &catalogue_json).unwrap();
        std::fs::write(&baseline_path, &baseline_json).unwrap();
        std::fs::write(track_dir.join("domain-types.json"), declaring_json).unwrap();

        let trusted = items_dir.canonicalize().unwrap();
        let loaded = load_track_catalogues(root, &track_dir, &trusted)
            .expect("architecture-rules must select every TDDD-enabled catalogue");
        let loaded_documents = loaded
            .catalogues
            .iter()
            .map(|(layer, attested)| (layer.clone(), attested.document().clone()))
            .collect::<BTreeMap<_, _>>();
        let empty = rustdoc_crate_with_paths(HashMap::new());
        let encoded = CatalogueToExtendedCrateCodec::new()
            .encode(&LayerId::try_new("infrastructure").unwrap(), &loaded_documents, &empty, &empty)
            .expect("other-layer add declarations must join the evaluator encode map");
        let user_id = encoded
            .krate()
            .paths
            .iter()
            .find(|(_, summary)| summary.path == ["domain", "model", "UserId"])
            .map(|(id, summary)| (*id, summary.clone()))
            .expect("domain::model::UserId must be a declaring-crate external item");
        assert_ne!(user_id.1.crate_id, 0);
        assert_eq!(encoded.krate().external_crates[&user_id.1.crate_id].name, "domain");
        assert!(
            !encoded
                .krate()
                .paths
                .values()
                .any(|summary| summary.path == ["infrastructure", "model", "UserId"]),
            "the target catalogue must not duplicate the declaring-layer item"
        );

        let result = evaluate_and_write(
            catalogue_json.as_bytes(),
            &catalogue_path,
            &track_dir,
            root,
            &items_dir.canonicalize().unwrap(),
            &binding,
            baseline_json.clone(),
            read_head_commit(root).unwrap(),
            &baseline_path,
            &baseline_json,
            type_signals_codec::baseline_hash(baseline_json.as_bytes()),
        )
        .expect("evaluator encode must consume every track catalogue");
        assert_eq!(result, ExitCode::SUCCESS);

        let persisted = type_signals_codec::decode(
            &std::fs::read_to_string(track_dir.join(binding.signal_file())).unwrap(),
        )
        .unwrap();
        assert!(
            persisted.signals().iter().any(|signal| signal.type_name() == "Handler"),
            "other-layer add declarations must reach encode through the evaluator handoff"
        );
    }

    #[test]
    fn test_evaluate_and_write_rejects_stale_target_catalogue_input() {
        let (workspace, items_dir, track_id, binding, _rustdoc_path) = setup_workspace();
        let root = workspace.path();
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue_path = track_dir.join(binding.catalogue_file());
        let baseline_path = track_dir.join(binding.baseline_file());
        let initial_catalogue = std::fs::read(&catalogue_path).unwrap();
        let changed_catalogue =
            format!("{} \n", std::str::from_utf8(&initial_catalogue).unwrap().trim_end());
        std::fs::write(&catalogue_path, changed_catalogue).unwrap();
        let baseline_json = std::fs::read_to_string(&baseline_path).unwrap();

        let error = evaluate_and_write(
            &initial_catalogue,
            &catalogue_path,
            &track_dir,
            root,
            &items_dir.canonicalize().unwrap(),
            &binding,
            rustdoc_json(),
            read_head_commit(root).unwrap(),
            &baseline_path,
            &baseline_json,
            type_signals_codec::baseline_hash(baseline_json.as_bytes()),
        )
        .expect_err("a stale caller catalogue must not mix with the attested target document");

        assert!(error.to_string().contains("target catalogue bytes changed"), "got: {error}");
    }

    #[test]
    fn test_load_track_catalogues_treats_enabled_layer_without_file_as_empty() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join("missing-layer-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            root.join("architecture-rules.json"),
            r#"{
            "version": 2,
            "layers": [
                {
                    "crate": "domain",
                    "path": "libs/domain",
                    "may_depend_on": [],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "domain-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["domain"] }
                    }
                },
                {
                    "crate": "infrastructure",
                    "path": "libs/infrastructure",
                    "may_depend_on": ["domain"],
                    "tddd": {
                        "enabled": true,
                        "catalogue_file": "infrastructure-types.json",
                        "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                    }
                }
            ]
        }"#,
        )
        .unwrap();
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            CatalogueDocumentCodec::encode(&CatalogueDocument::new(
                5,
                CrateName::new("infrastructure").unwrap(),
                LayerId::try_new("infrastructure").unwrap(),
            ))
            .unwrap(),
        )
        .unwrap();

        let loaded = load_track_catalogues(root, &track_dir, &items_dir.canonicalize().unwrap())
            .expect("missing enabled catalogues are empty, not errors");
        assert!(loaded.catalogues.contains_key(&LayerId::try_new("infrastructure").unwrap()));
        assert!(
            !loaded.catalogues.contains_key(&LayerId::try_new("domain").unwrap()),
            "a TDDD-enabled layer without a catalogue file must contribute no declarations"
        );
    }

    #[test]
    fn test_load_track_catalogues_uses_filename_stem_for_custom_catalogue_file() {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join("custom-catalogue-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [{
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "custom.json",
                    "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                }
            }]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        let document = CatalogueDocument::new(
            5,
            CrateName::new("custom").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        std::fs::write(
            track_dir.join("custom.json"),
            CatalogueDocumentCodec::encode(&document).unwrap(),
        )
        .unwrap();

        let loaded =
            load_track_catalogues(root, &track_dir, &items_dir.canonicalize().unwrap()).unwrap();
        assert_eq!(
            loaded
                .catalogues
                .get(&LayerId::try_new("infrastructure").unwrap())
                .unwrap()
                .document()
                .crate_name()
                .as_str(),
            "custom"
        );
    }

    #[test]
    fn test_load_track_catalogues_mismatched_layer_is_rejected() {
        use domain::tddd::catalogue_v2::{CatalogueDocument, CrateName};

        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let mismatched = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("domain").unwrap(),
        );
        std::fs::write(
            track_dir.join("infrastructure-types.json"),
            CatalogueDocumentCodec::encode(&mismatched).unwrap(),
        )
        .unwrap();

        let error =
            load_track_catalogues(workspace.path(), &track_dir, &items_dir.canonicalize().unwrap())
                .expect_err("a catalogue layer must match its architecture binding");
        let message = error.to_string();
        assert!(message.contains("declares layer 'domain'"), "got: {message}");
        assert!(message.contains("binding selects 'infrastructure'"), "got: {message}");
    }

    #[cfg(unix)]
    #[test]
    fn test_load_track_catalogues_dangling_symlink_is_rejected() {
        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue_path = track_dir.join("infrastructure-types.json");
        std::fs::remove_file(&catalogue_path).unwrap();
        std::os::unix::fs::symlink(track_dir.join("missing-types.json"), &catalogue_path).unwrap();

        let error =
            load_track_catalogues(workspace.path(), &track_dir, &items_dir.canonicalize().unwrap())
                .expect_err("a dangling catalogue symlink must not be treated as absent");
        assert!(error.to_string().contains("symlink guard rejected catalogue"));
    }

    #[cfg(unix)]
    #[test]
    fn test_load_track_catalogues_rejects_fifo_before_opening() {
        use std::time::Instant;

        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue_path = track_dir.join("infrastructure-types.json");
        std::fs::remove_file(&catalogue_path).unwrap();
        let status = std::process::Command::new("mkfifo").arg(&catalogue_path).status().unwrap();
        assert!(status.success(), "mkfifo must create the FIFO fixture");

        let started = Instant::now();
        let error =
            load_track_catalogues(workspace.path(), &track_dir, &items_dir.canonicalize().unwrap())
                .expect_err("a configured FIFO must fail closed");
        assert!(
            started.elapsed() < std::time::Duration::from_secs(1),
            "FIFO rejection must not block"
        );
        assert!(error.to_string().contains("not a regular file"), "got: {error}");
    }

    #[test]
    fn test_resolution_input_fingerprint_distinguishes_missing_from_empty_catalogue() {
        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let trusted = items_dir.canonicalize().unwrap();
        let present = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        std::fs::remove_file(track_dir.join("infrastructure-types.json")).unwrap();
        let missing = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        std::fs::write(track_dir.join("infrastructure-types.json"), []).unwrap();
        let empty = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted).unwrap();
        assert_ne!(present, missing, "a missing catalogue must not hash as a present catalogue");
        assert_ne!(missing, empty, "a 0-byte catalogue must not hash as a missing catalogue");
        assert_ne!(present, empty);
    }

    #[cfg(unix)]
    #[test]
    fn test_resolution_input_fingerprint_rejects_symlinked_architecture_rules() {
        let (workspace, items_dir, track_id, _binding, _rustdoc_path) = setup_workspace();
        let track_dir = items_dir.join(track_id.as_ref());
        let trusted = items_dir.canonicalize().unwrap();
        let rules_path = workspace.path().join("architecture-rules.json");
        let real_rules = workspace.path().join("architecture-rules.real.json");
        std::fs::rename(&rules_path, &real_rules).unwrap();
        std::os::unix::fs::symlink(&real_rules, &rules_path).unwrap();

        let error = resolution_input_fingerprint(workspace.path(), &track_dir, &trusted)
            .expect_err("a symlinked architecture-rules.json must fail closed");
        assert!(
            error.to_string().contains("symlink guard rejected architecture-rules.json"),
            "got: {error}"
        );
    }

    #[test]
    fn test_execute_type_signals_resolves_mutual_add_and_modify_references_in_declaration_order() {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::TypeEntry;
        use domain::tddd::catalogue_v2::roles::{DataRole, ItemAction};
        use domain::tddd::catalogue_v2::{
            CatalogueEntryKey, CrateName, FieldDecl, FieldName, ModulePath, TypeRef,
        };

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("infrastructure").unwrap(),
            LayerId::try_new("infrastructure").unwrap(),
        );
        // Insert the modify declaration before the add declarations. The production
        // pre-pass must still resolve every reference independently of declaration order.
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Holder".to_owned()).unwrap(),
            TypeEntry::new(
                ItemAction::Modify,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(
                    StructShape::Plain {
                        fields: vec![FieldDecl::new(
                            FieldName::new("first").unwrap(),
                            TypeRef::new("First").unwrap(),
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
        let first = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("second").unwrap(),
                        TypeRef::new("Second").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            None,
            None,
            vec![],
            vec![],
        );
        catalogue.insert_type(CatalogueEntryKey::try_new("First".to_owned()).unwrap(), first);
        let second = TypeEntry::new(
            ItemAction::Add,
            DataRole::value_object(),
            TypeKindV2::Struct(StructKind::new(
                StructShape::Plain {
                    fields: vec![FieldDecl::new(
                        FieldName::new("first").unwrap(),
                        TypeRef::new("First").unwrap(),
                    )],
                    has_stripped_fields: false,
                },
                None,
            )),
            vec![],
            vec![],
            vec![],
            None,
            None,
            vec![],
            vec![],
        );
        catalogue.insert_type(CatalogueEntryKey::try_new("Second".to_owned()).unwrap(), second);

        let baseline = rustdoc_crate_with_paths(HashMap::from([(
            rustdoc_types::Id(1),
            rustdoc_types::ItemSummary {
                crate_id: 0,
                path: vec!["infrastructure".to_owned(), "Holder".to_owned()],
                kind: rustdoc_types::ItemKind::Struct,
            },
        )]));
        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &baseline)
            .expect("the shared set must include both add declarations and the modify baseline");
        let identity_index = build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("all declared identities must resolve through the shared set");
        let _ = identity_index;
        let encoded = encode_document(catalogue, &baseline, &baseline)
            .expect("mutual add and modify-to-add references must encode");

        let id_for = |path: &[&str]| {
            let expected = path.iter().map(|segment| (*segment).to_owned()).collect::<Vec<_>>();
            encoded
                .krate()
                .paths
                .iter()
                .find(|(_, summary)| summary.path == expected)
                .map(|(id, _)| *id)
                .expect("encoded identity must have an authoritative path")
        };
        let first_id = id_for(&["infrastructure", "First"]);
        let second_id = id_for(&["infrastructure", "Second"]);
        let holder_id = id_for(&["infrastructure", "Holder"]);
        let field_target = |owner_id| {
            let rustdoc_types::ItemEnum::Struct(owner) = &encoded.krate().index[&owner_id].inner
            else {
                panic!("expected a struct owner")
            };
            let rustdoc_types::StructKind::Plain { fields, .. } = &owner.kind else {
                panic!("expected a plain struct owner")
            };
            let field_id = fields.first().expect("expected one reference-bearing field");
            let rustdoc_types::ItemEnum::StructField(rustdoc_types::Type::ResolvedPath(path)) =
                &encoded.krate().index[field_id].inner
            else {
                panic!("expected a resolved field reference")
            };
            path.id
        };

        assert_eq!(field_target(first_id), second_id);
        assert_eq!(field_target(second_id), first_id);
        assert_eq!(field_target(holder_id), first_id);
    }

    #[test]
    fn test_execute_type_signals_resolves_all_type_actions_and_modify_function_under_bin_root_alias()
     {
        use domain::tddd::LayerId;
        use domain::tddd::catalogue_v2::CatalogueDocument;
        use domain::tddd::catalogue_v2::composite::{StructKind, StructShape, TypeKindV2};
        use domain::tddd::catalogue_v2::entries::{FunctionEntry, TypeEntry};
        use domain::tddd::catalogue_v2::roles::{DataRole, FunctionRole, ItemAction};
        use domain::tddd::catalogue_v2::{
            CatalogueEntryKey, CrateName, FunctionName, FunctionPath, ModulePath, TypeRef,
        };

        let rooted_crate = |paths: Vec<(u32, Vec<&str>, rustdoc_types::ItemKind)>| {
            let path_map = paths
                .into_iter()
                .map(|(id, path, kind)| {
                    (
                        rustdoc_types::Id(id),
                        rustdoc_types::ItemSummary {
                            crate_id: 0,
                            path: path.into_iter().map(str::to_owned).collect(),
                            kind,
                        },
                    )
                })
                .collect::<HashMap<_, _>>();
            let mut krate = rustdoc_crate_with_paths(path_map);
            krate.index.insert(
                rustdoc_types::Id(0),
                rustdoc_types::Item {
                    id: rustdoc_types::Id(0),
                    crate_id: 0,
                    name: Some("sotp".to_owned()),
                    span: None,
                    visibility: rustdoc_types::Visibility::Public,
                    docs: None,
                    links: HashMap::new(),
                    attrs: vec![],
                    deprecation: None,
                    inner: rustdoc_types::ItemEnum::Module(rustdoc_types::Module {
                        is_crate: true,
                        items: vec![],
                        is_stripped: false,
                    }),
                },
            );
            krate
        };
        let type_entry = |action| {
            TypeEntry::new(
                action,
                DataRole::value_object(),
                TypeKindV2::Struct(StructKind::new(StructShape::Unit, None)),
                vec![],
                vec![],
                vec![],
                None,
                None,
                vec![],
                vec![],
            )
        };

        let mut catalogue = CatalogueDocument::new(
            5,
            CrateName::new("cli").unwrap(),
            LayerId::try_new("cli").unwrap(),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Added".to_owned()).unwrap(),
            type_entry(ItemAction::Add),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Modified".to_owned()).unwrap(),
            type_entry(ItemAction::Modify),
        );
        catalogue.insert_type(
            CatalogueEntryKey::try_new("Referenced".to_owned()).unwrap(),
            type_entry(ItemAction::Reference),
        );
        catalogue.insert_function(
            FunctionPath::new(
                CrateName::new("cli").unwrap(),
                ModulePath::from_segments(vec!["commands".to_owned()]).unwrap(),
                FunctionName::new("run").unwrap(),
            ),
            FunctionEntry::new(
                ItemAction::Modify,
                FunctionRole::FreeFunction,
                vec![],
                TypeRef::new("()").unwrap(),
                false,
                vec![],
                vec![],
                None,
                vec![],
                vec![],
            ),
        );

        let baseline = rooted_crate(vec![
            (1, vec!["sotp", "commands", "Modified"], rustdoc_types::ItemKind::Struct),
            (2, vec!["sotp", "commands", "Referenced"], rustdoc_types::ItemKind::Struct),
            (3, vec!["sotp", "commands", "run"], rustdoc_types::ItemKind::Function),
        ]);
        let current = rooted_crate(vec![
            (1, vec!["sotp", "commands", "Modified"], rustdoc_types::ItemKind::Struct),
            (2, vec!["sotp", "commands", "Referenced"], rustdoc_types::ItemKind::Struct),
            (3, vec!["sotp", "commands", "run"], rustdoc_types::ItemKind::Function),
            (4, vec!["sotp", "generated", "Added"], rustdoc_types::ItemKind::Struct),
        ]);
        let package = CrateName::new("cli").unwrap();
        let normalized_current = normalized_paths_for_doc(&current, &package);
        for expected in [
            vec!["cli", "commands", "Modified"],
            vec!["cli", "commands", "Referenced"],
            vec!["cli", "commands", "run"],
            vec!["cli", "generated", "Added"],
        ] {
            assert!(
                normalized_current.values().any(|summary| summary.path == expected),
                "root alias must be normalized once for {expected:?}"
            );
        }

        let resolution_paths = resolution_paths_for_document(&catalogue, &baseline, &current)
            .expect("all type actions must share one normalized resolution set");
        build_type_signal_identity_index(&catalogue, &resolution_paths)
            .expect("type-signal indexing must use the same normalized identities");
        let encoded = encode_document(catalogue, &baseline, &current)
            .expect("add, modify, reference, and function identities must encode under alias");

        let id_for = |path: &[&str]| {
            let expected = path.iter().map(|segment| (*segment).to_owned()).collect::<Vec<_>>();
            encoded
                .krate()
                .paths
                .iter()
                .find(|(_, summary)| summary.path == expected)
                .map(|(id, _)| *id)
                .expect("encoded path must be present")
        };
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "generated", "Added"])),
            Some(ItemAction::Add)
        );
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "commands", "Modified"])),
            Some(ItemAction::Modify)
        );
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "commands", "Referenced"])),
            Some(ItemAction::Reference)
        );
        assert_eq!(
            encoded.action_for(&id_for(&["cli", "commands", "run"])),
            Some(ItemAction::Modify)
        );
    }

    fn setup_workspace() -> (tempfile::TempDir, PathBuf, TrackId, TdddLayerBinding, PathBuf) {
        let workspace = tempfile::tempdir().unwrap();
        let root = workspace.path();
        crate::verify::test_support::git_init(root);
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
            "[package]\nname = \"infrastructure\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
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
        std::fs::write(
            root.join(".gitignore"),
            "track/items/feature-input-track/infrastructure-type-signals.json\n",
        )
        .unwrap();
        let rules = r#"{
            "version": 2,
            "layers": [{
                "crate": "infrastructure",
                "path": "libs/infrastructure",
                "may_depend_on": [],
                "tddd": {
                    "enabled": true,
                    "catalogue_file": "infrastructure-types.json",
                    "schema_export": { "method": "rustdoc", "targets": ["infrastructure"] }
                }
            }]
        }"#;
        std::fs::write(root.join("architecture-rules.json"), rules).unwrap();
        crate::verify::test_support::run_git(root, &["add", "."]);
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "fixture"]);
        let binding = parse_tddd_layers(rules).unwrap().pop().unwrap();

        (workspace, items_dir, track_id, binding, rustdoc_path)
    }

    fn signal_path(items_dir: &Path, track_id: &TrackId) -> PathBuf {
        items_dir.join(track_id.as_ref()).join("infrastructure-type-signals.json")
    }

    fn cache_document(
        items_dir: &Path,
        track_id: &TrackId,
        head_commit: domain::CommitHash,
    ) -> TypeSignalsDocument {
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue = std::fs::read(track_dir.join("infrastructure-types.json")).unwrap();
        let (_, baseline_hash) =
            read_actual_baseline(&track_dir.join("infrastructure-types-baseline.json")).unwrap();
        TypeSignalsDocument::new(
            Timestamp::new("2026-07-31T00:00:00Z").unwrap(),
            TypeSignalsCacheKey::new(
                type_signals_codec::declaration_hash(&catalogue),
                head_commit,
                baseline_hash,
            ),
            vec![],
        )
    }

    fn current_cache_document(
        workspace_root: &Path,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> TypeSignalsDocument {
        cache_document(items_dir, track_id, read_head_commit(workspace_root).unwrap())
    }

    fn write_cache(
        items_dir: &Path,
        track_id: &TrackId,
        head_commit: domain::CommitHash,
    ) -> String {
        let document = cache_document(items_dir, track_id, head_commit);
        let encoded = type_signals_codec::encode(&document).unwrap();
        std::fs::write(signal_path(items_dir, track_id), &encoded).unwrap();
        encoded
    }

    #[test]
    fn test_execute_type_signals_clean_worktree_and_matching_head_reuses_cache() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let original =
            write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        assert_eq!(
            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(observer.launches(), 0);
        assert_eq!(std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(), original);
    }

    #[test]
    fn test_execute_type_signals_dirty_worktree_recalculates_even_with_matching_cache() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let head_commit = read_head_commit(workspace.path()).unwrap();
        write_cache(&items_dir, &track_id, head_commit);
        std::fs::write(
            workspace.path().join("libs/infrastructure/src/lib.rs"),
            "pub struct ChangedFixture;\n",
        )
        .unwrap();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            &[],
            &observer,
        )
        .unwrap();

        assert_eq!(observer.launches(), 1);
        let persisted = type_signals_codec::decode(
            &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            persisted.cache_key(),
            current_cache_document(workspace.path(), &items_dir, &track_id).cache_key()
        );
    }

    #[test]
    fn test_execute_type_signals_clean_worktree_with_different_head_recalculates() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        write_cache(&items_dir, &track_id, domain::CommitHash::try_new("b".repeat(40)).unwrap());
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            &[],
            &observer,
        )
        .unwrap();

        assert_eq!(observer.launches(), 1);
        let persisted = type_signals_codec::decode(
            &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
        )
        .unwrap();
        assert_eq!(
            persisted.cache_key(),
            current_cache_document(workspace.path(), &items_dir, &track_id).cache_key()
        );
    }

    #[test]
    fn test_execute_type_signals_missing_or_invalid_cache_is_replaced_atomically() {
        let invalid_cache_variants = [
            ("missing", None),
            ("malformed", Some("not-json".to_owned())),
            (
                "schema-mismatch",
                Some(
                    serde_json::json!({
                        "schema_version": 999,
                        "generated_at": "2026-07-31T00:00:00Z",
                        "declaration_hash": "a".repeat(64),
                        "head_commit": "b".repeat(40),
                        "baseline_hash": "a".repeat(64),
                        "signals": []
                    })
                    .to_string(),
                ),
            ),
            (
                "missing-required-field",
                Some(
                    serde_json::json!({
                        "schema_version": 4,
                        "generated_at": "2026-07-31T00:00:00Z",
                        "declaration_hash": "a".repeat(64),
                        "baseline_hash": "a".repeat(64),
                        "signals": []
                    })
                    .to_string(),
                ),
            ),
            (
                "invalid-value",
                Some(
                    serde_json::json!({
                        "schema_version": 4,
                        "generated_at": "2026-07-31T00:00:00Z",
                        "declaration_hash": "a".repeat(64),
                        "head_commit": "not-a-commit",
                        "baseline_hash": "a".repeat(64),
                        "signals": []
                    })
                    .to_string(),
                ),
            ),
        ];

        for (label, invalid_cache) in invalid_cache_variants {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            if let Some(invalid_cache) = invalid_cache {
                std::fs::write(signal_path(&items_dir, &track_id), invalid_cache).unwrap();
            }
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .unwrap_or_else(|error| panic!("{label} cache must be treated as a miss: {error}"));

            assert_eq!(observer.launches(), 1, "{label} cache must trigger evaluation");
            let persisted = type_signals_codec::decode(
                &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
            )
            .unwrap();
            assert_eq!(
                persisted.cache_key(),
                current_cache_document(workspace.path(), &items_dir, &track_id).cache_key(),
                "{label} cache must be atomically replaced with current identities"
            );
        }
    }

    #[test]
    fn test_execute_type_signals_cache_write_failure_preserves_prior_cache() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let original = write_cache(
            &items_dir,
            &track_id,
            domain::CommitHash::try_new("b".repeat(40)).unwrap(),
        );
        let signal_path = signal_path(&items_dir, &track_id);
        let temporary_path = signal_path.parent().unwrap().join(format!(
            ".tmp-{}-{}",
            signal_path.file_name().unwrap().to_string_lossy(),
            std::process::id()
        ));
        std::fs::create_dir(&temporary_path).unwrap();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        let error = match execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            &[],
            &observer,
        ) {
            Ok(status) => panic!("a failed atomic cache write returned {status:?}"),
            Err(error) => error,
        };

        assert!(
            matches!(&error, EvaluateSignalsError::CacheWrite(_)),
            "signal write failure must be reported as a cache-write error: {error:?}"
        );
        assert_eq!(
            std::fs::read(&signal_path).unwrap(),
            original.as_bytes(),
            "failed replacement must preserve the prior cache bytes"
        );
    }

    #[test]
    fn test_execute_type_signals_cache_replacement_is_read_by_track_blob_reader() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            &[],
            &observer,
        )
        .unwrap();

        assert_eq!(observer.launches(), 1, "a missing cache must trigger evaluation");
        let persisted = type_signals_codec::decode(
            &std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(),
        )
        .unwrap();

        let root = workspace.path();
        crate::verify::test_support::run_git(root, &["branch", "-M", "main"]);
        crate::verify::test_support::run_git(
            root,
            &["remote", "add", "origin", root.to_str().unwrap()],
        );
        crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);
        crate::verify::test_support::run_git(
            root,
            &["add", "-f", "track/items/feature-input-track/infrastructure-type-signals.json"],
        );
        crate::verify::test_support::run_git(root, &["commit", "--quiet", "-m", "replace cache"]);
        crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);

        let reader =
            crate::verify::merge_gate_adapter::GitShowTrackBlobReader::new(root.to_path_buf());
        match reader.read_type_signals("main", track_id.as_ref(), "infrastructure") {
            BlobFetchResult::Found(document) => {
                assert_eq!(document.cache_key(), persisted.cache_key());
                assert_eq!(document.signals(), persisted.signals());
            }
            other => {
                panic!("the replaced cache must be readable through TrackBlobReader: {other:?}")
            }
        }
    }

    #[test]
    fn test_unreadable_type_signals_reader_forces_cache_miss_and_reevaluation() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let root = workspace.path();
        let signal_file = signal_path(&items_dir, &track_id);
        let relative_signal_file =
            "track/items/feature-input-track/infrastructure-type-signals.json";

        // Start with a cache that would otherwise be eligible for reuse, then
        // publish an unreadable replacement to the branch the blob reader sees.
        write_cache(&items_dir, &track_id, read_head_commit(root).unwrap());
        crate::verify::test_support::run_git(root, &["branch", "-M", "main"]);
        crate::verify::test_support::run_git(
            root,
            &["remote", "add", "origin", root.to_str().unwrap()],
        );
        crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);
        std::fs::write(&signal_file, b"not valid json").unwrap();
        crate::verify::test_support::run_git(root, &["add", "-f", relative_signal_file]);
        crate::verify::test_support::run_git(
            root,
            &["commit", "--quiet", "-m", "unreadable cache"],
        );
        crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);

        let reader =
            crate::verify::merge_gate_adapter::GitShowTrackBlobReader::new(root.to_path_buf());
        match reader.read_type_signals("main", track_id.as_ref(), "infrastructure") {
            BlobFetchResult::FetchError(message) => {
                assert!(message.contains("decode error"), "reader diagnostic: {message}");
            }
            other => {
                panic!("an existing malformed cache must be reported as unreadable: {other:?}")
            }
        }

        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
        execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            root,
            &binding,
            &[],
            &observer,
        )
        .unwrap();

        assert_eq!(observer.launches(), 1, "unreadable cache must force reevaluation");
        let replacement =
            type_signals_codec::decode(&std::fs::read_to_string(&signal_file).unwrap()).unwrap();
        assert_eq!(
            replacement.cache_key(),
            current_cache_document(root, &items_dir, &track_id).cache_key()
        );

        crate::verify::test_support::run_git(root, &["add", "-f", relative_signal_file]);
        crate::verify::test_support::run_git(
            root,
            &["commit", "--quiet", "-m", "replace unreadable cache"],
        );
        crate::verify::test_support::run_git(root, &["fetch", "--quiet", "origin"]);
        assert!(matches!(
            reader.read_type_signals("main", track_id.as_ref(), "infrastructure"),
            BlobFetchResult::Found(_)
        ));
    }

    #[test]
    fn test_execute_type_signals_unreadable_authority_does_not_reuse_cache() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let original =
            write_cache(&items_dir, &track_id, read_head_commit(workspace.path()).unwrap());
        std::fs::remove_file(items_dir.join(track_id.as_ref()).join("infrastructure-types.json"))
            .unwrap();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        let result = execute_type_signals_for_layer_with_launch_observer(
            &items_dir,
            &track_id,
            workspace.path(),
            &binding,
            &[],
            &observer,
        );

        assert!(matches!(result, Err(EvaluateSignalsError::AuthoritativeInput(_))));
        assert_eq!(observer.launches(), 0, "unreadable authority must not reuse the cache");
        assert_eq!(std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(), original);
    }
}
