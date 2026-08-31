//! Assembly of authoritative rustdoc contexts for type-signal encoding.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::{Arc, Mutex, MutexGuard};

use domain::tddd::catalogue_v2::{AttestedCatalogueDocument, CrateName, RustdocCratePortError};
use domain::tddd::type_signals_doc::{
    BaselineHash, CapturedRustdocJson, RustdocExecutionIdentity, RustdocSnapshot,
    TypeSignalsCacheKey, TypeSignalsDocument,
};
use domain::tddd::{AuthoritativeRustdocContext, CargoFeatureName, LayerId};
use domain::{CatalogueDeclarationHash, CommitHash, Timestamp};

use super::freshness::RustdocProvider;
use super::{
    EvaluateSignalsError, EvaluationObservers, MAX_RUSTDOC_JSON_BYTES, TdddLayerBinding,
    contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag, validate_evaluator_binding,
};
use crate::tddd::baseline_rustdoc_codec::BaselineRustdocCodec;
use crate::tddd::catalogue_document_codec::{CatalogueDocumentCodec, derive_filename_stem};
use crate::tddd::catalogue_to_extended_crate_codec::{
    CatalogueToExtendedCrateCodec, normalized_paths_for_doc,
    resolution_paths_for_catalogue_with_contexts,
};
use crate::tddd::signal_evaluator_v2::SignalEvaluatorV2;
use crate::tddd::type_signals_codec;
use crate::tddd::{CatalogueToExtendedCratePort, SignalEvaluatorPort};
use crate::track::atomic_write::atomic_write_file;
use crate::track::symlink_guard::reject_symlinks_below;

#[path = "resolution_fingerprint.rs"]
mod resolution_fingerprint;
pub(crate) use resolution_fingerprint::{
    encode_snapshot_bytes, read_configured_catalogue, read_workspace_file,
    resolution_input_fingerprint,
};

/// Conservative upper bound for the number of rustdoc exports in one context
/// assembly. Each export is independently time-bounded by the schema exporter;
/// exceeding this count fails closed instead of allowing unbounded aggregate
/// process and I/O work from configuration.
pub(super) const MAX_RUSTDOC_CONTEXT_EXPORTS: usize = 64;

pub(super) fn map_rustdoc_capture_error(
    error: RustdocCratePortError,
    context: impl Into<String>,
) -> EvaluateSignalsError {
    let context = context.into();
    match error {
        RustdocCratePortError::AuthoritativeInput { reason, .. } => {
            EvaluateSignalsError::authoritative_input(format!("{context}: {reason}"))
        }
        other @ (RustdocCratePortError::NotFound { .. }
        | RustdocCratePortError::Io { .. }
        | RustdocCratePortError::ParseFailed { .. }
        | RustdocCratePortError::CaptureFailed { .. }) => {
            EvaluateSignalsError::evaluation(format!("{context}: {other}"))
        }
    }
}

#[derive(Debug)]
pub(super) struct AssembledRustdocContexts {
    pub(super) contexts: BTreeMap<LayerId, AuthoritativeRustdocContext>,
    pub(super) baseline_snapshots: BTreeMap<LayerId, BaselineSnapshot>,
}

#[derive(Debug)]
pub(super) struct LoadedTrackCatalogues {
    pub(super) bindings: Vec<TdddLayerBinding>,
    pub(super) catalogues: BTreeMap<LayerId, AttestedCatalogueDocument>,
    pub(super) baselines: BTreeMap<LayerId, CapturedRustdocJson>,
    pub(super) baseline_paths: BTreeMap<LayerId, PathBuf>,
    pub(super) resolution_fingerprint: Option<CatalogueDeclarationHash>,
}

#[derive(Debug, Clone)]
pub(super) struct BaselineSnapshot {
    pub(super) path: PathBuf,
    pub(super) hash: BaselineHash,
}

/// A run-local immutable context cache used to share a complete context
/// assembly across the per-layer executor calls that make up one type-signals
/// run. The mutex also serializes callers using the same executor; the rustdoc
/// provider adds a filesystem lock for callers in other processes.
#[derive(Debug, Default)]
pub(crate) struct RustdocContextCache {
    state: Mutex<Option<RustdocContextCacheEntry>>,
}

#[derive(Debug)]
struct RustdocContextCacheEntry {
    pub(super) key: RustdocContextCacheKey,
    assembled: Arc<AssembledRustdocContexts>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct RustdocContextCacheKey {
    pub(super) workspace_root: PathBuf,
    pub(super) track_dir: PathBuf,
    pub(super) resolution_fingerprint: CatalogueDeclarationHash,
    pub(super) current_implementation_fingerprint: String,
    pub(super) head_commit: CommitHash,
    pub(super) feature_selections: BTreeMap<LayerId, Vec<CargoFeatureName>>,
    pub(super) rustdoc_execution_identities: BTreeMap<LayerId, RustdocExecutionIdentity>,
}

impl RustdocContextCache {
    fn lock(
        &self,
    ) -> Result<MutexGuard<'_, Option<RustdocContextCacheEntry>>, EvaluateSignalsError> {
        self.state.lock().map_err(|_| {
            EvaluateSignalsError::authoritative_input(
                "rustdoc context cache lock was poisoned; refusing to reuse an incomplete snapshot",
            )
        })
    }

    /// Returns a shared immutable snapshot without extending the mutex guard's
    /// lifetime into rustdoc export or evaluation I/O.
    pub(super) fn get(
        &self,
        key: &RustdocContextCacheKey,
    ) -> Result<Option<Arc<AssembledRustdocContexts>>, EvaluateSignalsError> {
        let state = self.lock()?;
        Ok(state
            .as_ref()
            .filter(|entry| entry.key == *key)
            .map(|entry| Arc::clone(&entry.assembled)))
    }

    /// Publishes a completed snapshot, or returns the snapshot another caller
    /// published for the same key while this caller was doing its I/O.
    pub(super) fn insert_or_get(
        &self,
        key: RustdocContextCacheKey,
        assembled: AssembledRustdocContexts,
    ) -> Result<Arc<AssembledRustdocContexts>, EvaluateSignalsError> {
        let shared = Arc::new(assembled);
        let mut state = self.lock()?;
        if let Some(entry) = state.as_ref().filter(|entry| entry.key == key) {
            return Ok(Arc::clone(&entry.assembled));
        }
        *state = Some(RustdocContextCacheEntry { key, assembled: shared.clone() });
        Ok(shared)
    }
}

#[cfg(test)]
pub(super) fn load_track_catalogues(
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
        super::reject_type_signals_path(&path, trusted_items_root, "catalogue")?;
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
    Ok(LoadedTrackCatalogues {
        bindings,
        catalogues,
        baselines: BTreeMap::new(),
        baseline_paths: BTreeMap::new(),
        resolution_fingerprint: None,
    })
}

/// Captures the complete catalogue/baseline/rules resolution input set from
/// one rules snapshot. The returned maps and fingerprint are all derived from
/// those same bytes; callers must not reload the paths to reconstruct them.
pub(super) fn load_authoritative_inputs(
    workspace_root: &Path,
    track_dir: &Path,
    trusted_items_root: &Path,
    rustdoc: &impl RustdocProvider,
) -> Result<LoadedTrackCatalogues, EvaluateSignalsError> {
    let rules_path = workspace_root.join("architecture-rules.json");
    let rules = read_workspace_file(&rules_path, workspace_root, "architecture-rules.json")?
        .ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(
                "architecture-rules.json not found".to_owned(),
            )
        })?;
    let rules_text = std::str::from_utf8(&rules).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "architecture-rules.json is not UTF-8: {error}"
        ))
    })?;
    let bindings = crate::verify::tddd_layers::parse_tddd_layers(rules_text).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot parse architecture-rules.json: {error}"
        ))
    })?;
    if bindings.len() > MAX_RUSTDOC_CONTEXT_EXPORTS {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "configured TDDD layer count {} exceeds the rustdoc context export budget of {}",
            bindings.len(),
            MAX_RUSTDOC_CONTEXT_EXPORTS
        )));
    }

    let mut fingerprint_input = Vec::new();
    encode_snapshot_bytes(&mut fingerprint_input, &rules_path, Some(&rules))?;
    for file_name in
        [super::TDDD_FEATURE_DECLARATION_FILE, super::TDDD_FEATURE_DECLARATION_SNAPSHOT_FILE]
    {
        let path = track_dir.join(file_name);
        super::reject_type_signals_path(&path, trusted_items_root, file_name)?;
        match read_workspace_file(&path, trusted_items_root, file_name)? {
            Some(bytes) => encode_snapshot_bytes(&mut fingerprint_input, &path, Some(&bytes))?,
            None => encode_snapshot_bytes(&mut fingerprint_input, &path, None)?,
        }
    }

    let mut catalogues = BTreeMap::new();
    let mut baselines = BTreeMap::new();
    let mut baseline_paths = BTreeMap::new();
    for binding in &bindings {
        let layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "invalid layer id '{}': {error}",
                binding.layer_id()
            ))
        })?;
        fingerprint_input.extend_from_slice(layer.as_ref().as_bytes());

        let catalogue_path = track_dir.join(binding.catalogue_file());
        super::reject_type_signals_path(&catalogue_path, trusted_items_root, "catalogue")?;
        match read_configured_catalogue(&catalogue_path, trusted_items_root)? {
            Some(bytes) => {
                encode_snapshot_bytes(&mut fingerprint_input, &catalogue_path, Some(&bytes))?;
                let name = derive_filename_stem(&catalogue_path);
                let document = AttestedCatalogueDocument::attest(&bytes, |source| {
                    let text = std::str::from_utf8(source).map_err(|error| error.to_string())?;
                    CatalogueDocumentCodec::decode(text, &name).map_err(|error| error.to_string())
                })
                .map_err(|error| {
                    EvaluateSignalsError::authoritative_input(format!(
                        "cannot decode catalogue '{}': {error}",
                        catalogue_path.display()
                    ))
                })?;
                if document.document().layer() != &layer {
                    return Err(EvaluateSignalsError::authoritative_input(format!(
                        "catalogue '{}' declares layer '{}' but architecture binding selects '{}'",
                        catalogue_path.display(),
                        document.document().layer(),
                        layer
                    )));
                }
                if catalogues.insert(layer.clone(), document).is_some() {
                    return Err(EvaluateSignalsError::authoritative_input(format!(
                        "duplicate effective TDDD catalogue layer '{layer}'"
                    )));
                }
            }
            None => encode_snapshot_bytes(&mut fingerprint_input, &catalogue_path, None)?,
        }

        let baseline_path = track_dir.join(binding.baseline_file());
        super::reject_type_signals_path(&baseline_path, trusted_items_root, "baseline")?;
        let baseline = rustdoc.load_from_path(&baseline_path).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot load baseline for layer '{layer}': {error}"
            ))
        })?;
        encode_snapshot_bytes(
            &mut fingerprint_input,
            &baseline_path,
            Some(baseline.json_hash().as_digest().as_str().as_bytes()),
        )?;
        baselines.insert(layer.clone(), baseline);
        baseline_paths.insert(layer, baseline_path);
    }
    Ok(LoadedTrackCatalogues {
        bindings,
        catalogues,
        baselines,
        baseline_paths,
        resolution_fingerprint: Some(type_signals_codec::declaration_hash(&fingerprint_input)),
    })
}

pub(super) fn crate_with_canonical_paths(
    krate: &rustdoc_types::Crate,
    paths: HashMap<rustdoc_types::Id, rustdoc_types::ItemSummary>,
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

pub(super) fn reject_type_signals_path(
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

pub(super) fn read_actual_baseline(
    path: &Path,
    trusted_items_root: &Path,
) -> Result<(String, BaselineHash), EvaluateSignalsError> {
    let bytes = crate::tddd::tddd_catalogue_document_loader::read_optional_regular_file_bytes(
        path,
        Some(trusted_items_root),
        MAX_RUSTDOC_JSON_BYTES as u64,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read baseline '{}': {error}",
            path.display()
        ))
    })?
    .ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot read baseline '{}': file not found",
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

/// Assembles contexts from the already-captured authoritative input snapshot.
/// No catalogue, baseline, or current rustdoc path is read by this function.
pub(super) fn assemble_rustdoc_contexts_from_snapshot(
    configured_layers: &[TdddLayerBinding],
    target_layer: &LayerId,
    loaded: &LoadedTrackCatalogues,
    target_current: &RustdocSnapshot,
    feature_selections: &BTreeMap<LayerId, Vec<CargoFeatureName>>,
    rustdoc: &impl RustdocProvider,
) -> Result<AssembledRustdocContexts, EvaluateSignalsError> {
    if configured_layers.len() > MAX_RUSTDOC_CONTEXT_EXPORTS {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "configured TDDD layer count {} exceeds the rustdoc context export budget of {}",
            configured_layers.len(),
            MAX_RUSTDOC_CONTEXT_EXPORTS
        )));
    }
    let mut contexts = BTreeMap::new();
    let mut baseline_snapshots = BTreeMap::new();
    for configured_binding in configured_layers {
        let layer =
            LayerId::try_new(configured_binding.layer_id().to_owned()).map_err(|error| {
                EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
            })?;
        let baseline = loaded.baselines.get(&layer).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "baseline snapshot for configured layer '{layer}' is unavailable"
            ))
        })?;
        let selected_features = feature_selections.get(&layer).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "feature declaration has no selection for configured layer '{layer}'"
            ))
        })?;
        let current = if &layer == target_layer {
            target_current.clone()
        } else {
            let target = match configured_binding.targets() {
                [target] => target.as_str(),
                _ => {
                    return Err(EvaluateSignalsError::authoritative_input(format!(
                        "type-signal layer '{layer}' requires exactly one rustdoc target"
                    )));
                }
            };
            let target_crate = CrateName::new(target).map_err(|error| {
                EvaluateSignalsError::authoritative_input(format!(
                    "invalid rustdoc target crate '{target}' for layer '{layer}': {error}"
                ))
            })?;
            let snapshot =
                rustdoc.capture_current(&target_crate, selected_features).map_err(|error| {
                    map_rustdoc_capture_error(
                        error,
                        format!("rustdoc export failed for layer '{layer}' ('{target}')"),
                    )
                })?;
            if snapshot.execution_identity().crate_name() != &target_crate
                || snapshot.execution_identity().features() != selected_features
            {
                return Err(EvaluateSignalsError::authoritative_input(format!(
                    "rustdoc snapshot identity does not match layer '{layer}' selection"
                )));
            }
            snapshot
        };
        contexts.insert(
            layer.clone(),
            AuthoritativeRustdocContext::new(layer.clone(), baseline.crate_data().clone(), current),
        );
        let baseline_path = loaded.baseline_paths.get(&layer).ok_or_else(|| {
            EvaluateSignalsError::authoritative_input(format!(
                "baseline path for configured layer '{layer}' is unavailable"
            ))
        })?;
        baseline_snapshots.insert(
            layer,
            BaselineSnapshot {
                path: baseline_path.clone(),
                hash: BaselineHash::new(baseline.json_hash().as_digest().clone()),
            },
        );
    }
    Ok(AssembledRustdocContexts { contexts, baseline_snapshots })
}

/// Computes the current implementation snapshot used to identify a run-local
/// rustdoc context cache entry.
///
/// The rustdoc graph is derived from workspace source, manifests, build inputs,
/// toolchain configuration, and selected Cargo environment values, not only
/// from the catalogue and baseline inputs. Reusing a context after one of those
/// inputs changes would therefore make a dirty worktree appear to have the old
/// current implementation. The fingerprint helper returns a bounded,
/// fail-closed snapshot rather than relying on timestamps or path-only status.
pub(super) fn current_implementation_fingerprint(
    workspace_root: &Path,
) -> Result<String, EvaluateSignalsError> {
    super::freshness::rustdoc_input_fingerprint(workspace_root).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!(
            "cannot fingerprint semantic rustdoc inputs: {error}"
        ))
    })
}

/// Recomputes the current implementation snapshot and rejects persistence when
/// source files changed during rustdoc extraction or signal evaluation.
pub(super) fn verify_current_implementation_unchanged(
    workspace_root: &Path,
    initial_fingerprint: &str,
) -> Result<(), EvaluateSignalsError> {
    let current = current_implementation_fingerprint(workspace_root)?;
    if current != initial_fingerprint {
        return Err(EvaluateSignalsError::authoritative_input(
            "workspace Rust implementation changed during type-signal evaluation",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn evaluate_and_write_with_contexts(
    catalogue_bytes: &[u8],
    catalogue_path: &Path,
    track_dir: &Path,
    workspace_root: &Path,
    trusted_items_root: &Path,
    binding: &TdddLayerBinding,
    rustdoc_contexts: &BTreeMap<LayerId, AuthoritativeRustdocContext>,
    _baseline_snapshots: &BTreeMap<LayerId, BaselineSnapshot>,
    loaded_catalogues: &LoadedTrackCatalogues,
    start_resolution: CatalogueDeclarationHash,
    start_implementation: String,
    head_commit: CommitHash,
    baseline_path: &Path,
    baseline_hash: BaselineHash,
    initial_cache_key: TypeSignalsCacheKey,
    observers: EvaluationObservers<'_>,
) -> Result<ExitCode, EvaluateSignalsError> {
    super::reject_type_signals_path(baseline_path, trusted_items_root, "baseline")?;
    let target_layer = LayerId::try_new(binding.layer_id().to_owned()).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("invalid layer id: {error}"))
    })?;
    let target_context = rustdoc_contexts.get(&target_layer).ok_or_else(|| {
        EvaluateSignalsError::authoritative_input(format!(
            "target layer '{target_layer}' has no authoritative rustdoc context"
        ))
    })?;
    if let Some(snapshot_fingerprint) = loaded_catalogues.resolution_fingerprint.as_ref() {
        if snapshot_fingerprint != &start_resolution {
            return Err(EvaluateSignalsError::authoritative_input(
                "resolution snapshot fingerprint does not match the evaluation start".to_owned(),
            ));
        }
    }
    if let Some(snapshot) = _baseline_snapshots.get(&target_layer) {
        if snapshot.path != baseline_path || snapshot.hash != baseline_hash {
            return Err(EvaluateSignalsError::authoritative_input(
                "target baseline snapshot does not match the evaluation input".to_owned(),
            ));
        }
    }
    let baseline = target_context.baseline();
    let current = target_context.current();
    validate_evaluator_binding(&loaded_catalogues.bindings, binding)?;
    let target_catalogue = loaded_catalogues.catalogues.get(&target_layer).ok_or_else(|| {
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
    let track_catalogues = loaded_catalogues
        .catalogues
        .clone()
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
    let identity_paths = resolution_paths_for_catalogue_with_contexts(
        &target_layer,
        &track_catalogues,
        rustdoc_contexts,
    )
    .map_err(|error| EvaluateSignalsError::evaluation(error.to_string()))?;
    if let Some(observe) = observers.resolution_paths {
        observe(&identity_paths);
    }
    let identity_index = super::build_type_signal_identity_index(&catalogue, &identity_paths)
        .map_err(EvaluateSignalsError::evaluation)?;
    let canonical_baseline_paths = normalized_paths_for_doc(baseline, catalogue.crate_name());
    let canonical_current_paths = normalized_paths_for_doc(current, catalogue.crate_name());
    let canonical_baseline = crate_with_canonical_paths(baseline, canonical_baseline_paths);
    let canonical_current = crate_with_canonical_paths(current, canonical_current_paths);
    let extended = CatalogueToExtendedCrateCodec::new()
        .encode(&target_layer, &track_catalogues, rustdoc_contexts)
        .map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!("cannot convert catalogue: {error}"))
        })?;
    if let Some(observe) = observers.encoded_crate {
        observe(&extended);
    }
    let report = SignalEvaluatorV2::with_workspace_root(workspace_root.to_path_buf())
        .evaluate(extended, canonical_baseline, canonical_current)
        .map_err(|error| {
            EvaluateSignalsError::evaluation(format!("signal evaluation failed: {error:?}"))
        })?;
    let end_resolution =
        resolution_input_fingerprint(workspace_root, track_dir, trusted_items_root)?;
    if end_resolution != start_resolution {
        return Err(EvaluateSignalsError::authoritative_input(
            "architecture-rules, track catalogues, rustdoc baselines, or feature declarations changed during type-signal evaluation"
                .to_owned(),
        ));
    }
    super::reject_type_signals_path(catalogue_path, trusted_items_root, "catalogue")?;
    if declaration_hash != *initial_cache_key.declaration_hash()
        || baseline_hash != *initial_cache_key.baseline_hash()
        || head_commit != *initial_cache_key.head_commit()
    {
        return Err(EvaluateSignalsError::authoritative_input(
            "type-signal cache identity changed during evaluation".to_owned(),
        ));
    }
    verify_current_implementation_unchanged(workspace_root, &start_implementation)?;
    let generated_at = Timestamp::new(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .map_err(|error| {
            EvaluateSignalsError::evaluation(format!("cannot create timestamp: {error}"))
        })?;
    let document = TypeSignalsDocument::new(
        generated_at,
        initial_cache_key,
        super::build_type_signals_from_report(report.iter(), &kinds, &identity_index),
    );
    let encoded = type_signals_codec::encode_with_workspace(&document, Some(workspace_root))
        .map_err(|error| {
            EvaluateSignalsError::cache_write(format!("cannot encode type signals: {error}"))
        })?;
    let signal_path = track_dir.join(binding.signal_file());
    super::reject_type_signals_path(&signal_path, trusted_items_root, "signal artifact")?;
    atomic_write_file(&signal_path, format!("{encoded}\n").as_bytes()).map_err(|error| {
        EvaluateSignalsError::cache_write(format!("cannot write type signals: {error}"))
    })?;
    Ok(ExitCode::SUCCESS)
}

/// Re-reads every baseline used to assemble the rustdoc contexts and rejects
/// persistence if any authoritative baseline changed in the meantime.
#[cfg(all(test, feature = "test-helpers"))]
pub(super) fn verify_baseline_snapshots_unchanged(
    snapshots: &BTreeMap<LayerId, BaselineSnapshot>,
    trusted_items_root: &Path,
) -> Result<(), EvaluateSignalsError> {
    for (layer, snapshot) in snapshots {
        reject_type_signals_path(&snapshot.path, trusted_items_root, "baseline")?;
        let (_, current_hash) = read_actual_baseline(&snapshot.path, trusted_items_root)?;
        if current_hash != snapshot.hash {
            return Err(EvaluateSignalsError::authoritative_input(format!(
                "baseline for layer '{layer}' changed during type-signal evaluation"
            )));
        }
    }
    Ok(())
}
