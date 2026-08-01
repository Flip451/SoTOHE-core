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
use domain::tddd::type_signals_doc::{
    BaselineHash, TypeSignalsCacheKey, TypeSignalsDocument, TypeSignalsReuseDecision,
};
use domain::{FreeText, Timestamp, TrackId};
use freshness::{RustdocJsonPathProvider, reuse_decision_for_recorded_document};
use inputs::{hash_workspace_inputs, read_utf8_file_limited, verify_evaluation_inputs_unchanged};
use signal_builder::build_type_signals_from_report;
use signal_tags::{contract_role_kind_tag, data_role_kind_tag, function_role_kind_tag};

#[cfg(feature = "test-helpers")]
pub use freshness::RustdocLaunchObserver;

#[cfg(test)]
pub(crate) fn with_process_environment_lock<T>(action: impl FnOnce() -> T) -> T {
    let _environment_guard = match build_inputs::PROCESS_ENVIRONMENT_LOCK.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    action()
}

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
    let catalogue_bytes = inputs::read_bytes_file_limited(&catalogue_path, MAX_CATALOGUE_BYTES)
        .map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!(
                "cannot read catalogue '{}': {error}",
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
    let implementation_hash = hash_workspace_inputs(&canonical_workspace, target_crate, features)?;
    let current_key = TypeSignalsCacheKey::new(
        declaration_hash.clone(),
        implementation_hash.clone(),
        baseline_hash.clone(),
    );
    let recorded = read_utf8_file_limited(&signal_path, MAX_TYPE_SIGNALS_BYTES)
        .ok()
        .and_then(|text| type_signals_codec::decode(&text).ok());
    match reuse_decision_for_recorded_document(recorded.as_ref(), &current_key) {
        TypeSignalsReuseDecision::SkipEvaluation => {
            verify_evaluation_inputs_unchanged(
                &canonical_workspace,
                target_crate,
                features,
                &catalogue_path,
                &baseline_path,
                &current_key,
            )?;
            return Ok(ExitCode::SUCCESS);
        }
        // Cargo's shared rustdoc output path is not keyed by the cache identity
        // or feature selection. Re-extract rather than trusting an unrelated
        // producer's valid JSON document.
        TypeSignalsReuseDecision::ReevaluateWithoutExtraction => {}
        TypeSignalsReuseDecision::ReextractAndEvaluate => {}
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
    let current_implementation_hash =
        hash_workspace_inputs(&canonical_workspace, target_crate, features)?;
    if current_implementation_hash != implementation_hash {
        return Err(EvaluateSignalsError::authoritative_input(format!(
            "implementation inputs for '{target_crate}' changed during rustdoc extraction; re-run the evaluation"
        )));
    }
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
    target_crate: &str,
    binding: &TdddLayerBinding,
    features: &[CargoFeatureName],
    rustdoc_json: String,
    declaration_hash: domain::CatalogueDeclarationHash,
    implementation_hash: domain::ImplementationInputHash,
    baseline_path: &Path,
    baseline_json: &str,
    baseline_hash: BaselineHash,
) -> Result<ExitCode, EvaluateSignalsError> {
    let name = catalogue_path
        .file_stem()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_suffix("-types"))
        .unwrap_or(target_crate);
    let catalogue = CatalogueDocumentCodec::decode(
        std::str::from_utf8(catalogue_bytes).map_err(|error| {
            EvaluateSignalsError::authoritative_input(format!("catalogue is not UTF-8: {error}"))
        })?,
        name,
    )
    .map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("cannot decode catalogue: {error}"))
    })?;
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
    let extended = CatalogueToExtendedCrateCodec::new().encode(catalogue).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("cannot convert catalogue: {error}"))
    })?;
    reject_type_signals_path(baseline_path, trusted_items_root, "baseline")?;
    let baseline = BaselineRustdocCodec::from_json(baseline_json).map_err(|error| {
        EvaluateSignalsError::authoritative_input(format!("cannot decode baseline: {error}"))
    })?;
    let current = BaselineRustdocCodec::from_json(&rustdoc_json).map_err(|error| {
        EvaluateSignalsError::evaluation(format!("cannot decode rustdoc JSON: {error}"))
    })?;
    let report = SignalEvaluatorV2::with_workspace_root(workspace_root.to_path_buf())
        .evaluate(extended, baseline, current)
        .map_err(|error| {
            EvaluateSignalsError::evaluation(format!("signal evaluation failed: {error:?}"))
        })?;
    reject_type_signals_path(catalogue_path, trusted_items_root, "catalogue")?;
    verify_evaluation_inputs_unchanged(
        workspace_root,
        target_crate,
        features,
        catalogue_path,
        baseline_path,
        &TypeSignalsCacheKey::new(
            declaration_hash.clone(),
            implementation_hash.clone(),
            baseline_hash.clone(),
        ),
    )?;
    let generated_at = Timestamp::new(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .map_err(|error| {
            EvaluateSignalsError::evaluation(format!("cannot create timestamp: {error}"))
        })?;
    let document = TypeSignalsDocument::new(
        generated_at,
        TypeSignalsCacheKey::new(declaration_hash, implementation_hash, baseline_hash),
        build_type_signals_from_report(report.iter(), &kinds),
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
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::verify::tddd_layers::parse_tddd_layers;

    #[cfg(unix)]
    fn with_fake_rustup<T>(action: impl FnOnce() -> T) -> T {
        use std::os::unix::fs::PermissionsExt;

        with_process_environment_lock(|| {
            let fake_bin = tempfile::tempdir().unwrap();
            let rustup = fake_bin.path().join("rustup");
            std::fs::write(&rustup, "#!/bin/sh\nprintf 'test-nightly\\n'\n").unwrap();
            std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
            temp_env::with_var("PATH", Some(fake_bin.path().as_os_str()), action)
        })
    }

    #[cfg(unix)]
    fn with_fake_rustup_mutating_file_on_second_call<T>(
        mutation_path: &Path,
        mutation: &str,
        action: impl FnOnce() -> T,
    ) -> T {
        use std::os::unix::fs::PermissionsExt;

        with_process_environment_lock(|| {
            let fake_bin = tempfile::tempdir().unwrap();
            let counter_path = fake_bin.path().join("rustup-count");
            let rustup = fake_bin.path().join("rustup");
            std::fs::write(
                &rustup,
                format!(
                    "#!/bin/sh\ncount=0\nif [ -r '{counter}' ]; then IFS= read -r count < '{counter}'; fi\ncount=$((count + 1))\nprintf '%s' \"$count\" > '{counter}'\nif [ \"$count\" -eq 2 ]; then printf '%s' '{mutation}' >> '{path}'; fi\nprintf 'test-nightly\\n'\n",
                    counter = counter_path.display(),
                    mutation = mutation,
                    path = mutation_path.display(),
                ),
            )
            .unwrap();
            std::fs::set_permissions(&rustup, std::fs::Permissions::from_mode(0o755)).unwrap();
            temp_env::with_var("PATH", Some(fake_bin.path().as_os_str()), action)
        })
    }

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

    fn signal_path(items_dir: &Path, track_id: &TrackId) -> PathBuf {
        items_dir.join(track_id.as_ref()).join("infrastructure-type-signals.json")
    }

    fn current_document(
        workspace_root: &Path,
        items_dir: &Path,
        track_id: &TrackId,
    ) -> TypeSignalsDocument {
        let track_dir = items_dir.join(track_id.as_ref());
        let catalogue = std::fs::read(track_dir.join("infrastructure-types.json")).unwrap();
        let (_, baseline_hash) =
            read_actual_baseline(&track_dir.join("infrastructure-types-baseline.json")).unwrap();
        let implementation_hash = with_fake_rustup(|| {
            hash_workspace_inputs(workspace_root, "infrastructure", &[]).unwrap()
        });

        TypeSignalsDocument::new(
            Timestamp::new("2026-07-31T00:00:00Z").unwrap(),
            TypeSignalsCacheKey::new(
                type_signals_codec::declaration_hash(&catalogue),
                implementation_hash,
                baseline_hash,
            ),
            vec![],
        )
    }

    fn write_current_cache(workspace_root: &Path, items_dir: &Path, track_id: &TrackId) -> String {
        let encoded =
            type_signals_codec::encode(&current_document(workspace_root, items_dir, track_id))
                .unwrap();
        std::fs::write(signal_path(items_dir, track_id), &encoded).unwrap();
        encoded
    }

    fn execute_with_observer(
        items_dir: &Path,
        track_id: &TrackId,
        workspace_root: &Path,
        binding: &TdddLayerBinding,
        observer: &RustdocLaunchObserver,
    ) -> Result<ExitCode, EvaluateSignalsError> {
        with_fake_rustup(|| {
            execute_type_signals_for_layer_with_launch_observer(
                items_dir,
                track_id,
                workspace_root,
                binding,
                &[],
                observer,
            )
        })
    }

    #[test]
    fn test_execute_type_signals_feature_selection_change_reextracts_and_reaches_rustdoc() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let declared_feature = CargoFeatureName::try_new("semantic-dup".to_owned()).unwrap();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        with_fake_rustup(|| {
            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                std::slice::from_ref(&declared_feature),
                &observer,
            )
        })
        .unwrap();
        let signal_path =
            items_dir.join(track_id.as_ref()).join("infrastructure-type-signals.json");
        let first =
            type_signals_codec::decode(&std::fs::read_to_string(&signal_path).unwrap()).unwrap();

        execute_with_observer(&items_dir, &track_id, workspace.path(), &binding, &observer)
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
            first.cache_key().implementation_input_hash(),
            second.cache_key().implementation_input_hash(),
            "changing the declaration-derived feature selection must not reuse a stale artifact"
        );
    }

    #[test]
    fn test_execute_type_signals_all_three_hash_cache_hit_skips_evaluation() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let original = write_current_cache(workspace.path(), &items_dir, &track_id);
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        assert_eq!(
            execute_with_observer(&items_dir, &track_id, workspace.path(), &binding, &observer)
                .unwrap(),
            ExitCode::SUCCESS
        );
        assert_eq!(observer.launches(), 0, "a full three-hash cache hit must not extract rustdoc");
        assert_eq!(std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_cache_hit_input_mutation_fails_closed() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let original = write_current_cache(workspace.path(), &items_dir, &track_id);
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        let source_path = workspace.path().join("libs/infrastructure/src/lib.rs");
        let error = with_fake_rustup_mutating_file_on_second_call(
            &source_path,
            "\\npub struct ChangedDuringCacheCheck;\\n",
            || {
                execute_type_signals_for_layer_with_launch_observer(
                    &items_dir,
                    &track_id,
                    workspace.path(),
                    &binding,
                    &[],
                    &observer,
                )
                .unwrap_err()
            },
        );

        assert!(
            error
                .to_string()
                .contains("implementation inputs changed during type-signal evaluation"),
            "got: {error}"
        );
        assert_eq!(observer.launches(), 0, "a changed cache-hit input must not be reused");
        assert_eq!(std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(), original);
    }

    #[cfg(unix)]
    #[test]
    fn test_execute_type_signals_cache_hit_catalogue_mutation_fails_closed() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let original = write_current_cache(workspace.path(), &items_dir, &track_id);
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);
        let catalogue_path = items_dir.join(track_id.as_ref()).join("infrastructure-types.json");

        let error = with_fake_rustup_mutating_file_on_second_call(&catalogue_path, "\\n", || {
            execute_type_signals_for_layer_with_launch_observer(
                &items_dir,
                &track_id,
                workspace.path(),
                &binding,
                &[],
                &observer,
            )
            .unwrap_err()
        });

        assert!(
            error.to_string().contains("catalogue changed during type-signal evaluation"),
            "got: {error}"
        );
        assert_eq!(observer.launches(), 0, "a changed cache-hit catalogue must not be reused");
        assert_eq!(std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(), original);
    }

    #[test]
    fn test_execute_type_signals_each_hash_mismatch_is_a_cache_miss() {
        for mismatch in ["declaration", "implementation", "baseline"] {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let original = write_current_cache(workspace.path(), &items_dir, &track_id);
            let track_dir = items_dir.join(track_id.as_ref());
            if mismatch == "declaration" {
                let catalogue_path = track_dir.join("infrastructure-types.json");
                let catalogue = std::fs::read_to_string(&catalogue_path).unwrap();
                std::fs::write(catalogue_path, format!("{catalogue}\n")).unwrap();
            } else if mismatch == "implementation" {
                std::fs::write(
                    workspace.path().join("libs/infrastructure/src/lib.rs"),
                    "pub struct ChangedFixture;\n",
                )
                .unwrap();
            } else {
                assert_eq!(mismatch, "baseline");
                std::fs::write(
                    track_dir.join("infrastructure-types-baseline.json"),
                    format!("{}\n", rustdoc_json()),
                )
                .unwrap();
            }
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            assert_eq!(
                execute_with_observer(&items_dir, &track_id, workspace.path(), &binding, &observer)
                    .unwrap(),
                ExitCode::SUCCESS,
                "{mismatch} mismatch must evaluate rather than reuse the old cache"
            );
            assert_eq!(
                observer.launches(),
                1,
                "{mismatch} mismatch must obtain a fresh rustdoc artifact"
            );
            let replacement = std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap();
            let replacement = type_signals_codec::decode(&replacement).unwrap();
            assert_ne!(
                type_signals_codec::encode(&replacement).unwrap(),
                original,
                "{mismatch} mismatch must replace the stale cache"
            );
            assert_eq!(
                replacement.cache_key(),
                current_document(workspace.path(), &items_dir, &track_id).cache_key(),
                "{mismatch} replacement must use all current authoritative hashes"
            );
        }
    }

    #[test]
    fn test_execute_type_signals_missing_malformed_or_schema_incompatible_cache_replaces_it() {
        for invalid_cache in ["missing", "malformed", "schema-incompatible"] {
            let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
            let path = signal_path(&items_dir, &track_id);
            if invalid_cache == "malformed" {
                std::fs::write(&path, "{ malformed cache }").unwrap();
            } else if invalid_cache == "schema-incompatible" {
                let current = type_signals_codec::encode(&current_document(
                    workspace.path(),
                    &items_dir,
                    &track_id,
                ))
                .unwrap();
                let marker = format!("\"schema_version\": {}", domain::TYPE_SIGNALS_SCHEMA_VERSION);
                std::fs::write(&path, current.replacen(&marker, "\"schema_version\": 3", 1))
                    .unwrap();
            } else {
                assert_eq!(invalid_cache, "missing");
            }
            let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

            assert_eq!(
                execute_with_observer(&items_dir, &track_id, workspace.path(), &binding, &observer)
                    .unwrap(),
                ExitCode::SUCCESS,
                "{invalid_cache} cache must be reevaluated"
            );
            assert_eq!(observer.launches(), 1, "{invalid_cache} cache must reextract rustdoc");
            assert_eq!(
                type_signals_codec::decode(&std::fs::read_to_string(&path).unwrap())
                    .unwrap()
                    .cache_key(),
                current_document(workspace.path(), &items_dir, &track_id).cache_key()
            );
        }
    }

    #[test]
    fn test_execute_type_signals_evaluation_failure_fails_closed_without_replacing_cache() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let original = write_current_cache(workspace.path(), &items_dir, &track_id);
        let catalogue_path = items_dir.join(track_id.as_ref()).join("infrastructure-types.json");
        let catalogue = std::fs::read_to_string(&catalogue_path).unwrap();
        std::fs::write(&catalogue_path, format!("{catalogue}\n")).unwrap();
        std::fs::write(&rustdoc_path, "{ malformed rustdoc }").unwrap();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        let error =
            execute_with_observer(&items_dir, &track_id, workspace.path(), &binding, &observer)
                .unwrap_err();
        assert!(error.to_string().contains("cannot decode rustdoc JSON"));
        assert_eq!(observer.launches(), 1);
        assert_eq!(std::fs::read_to_string(signal_path(&items_dir, &track_id)).unwrap(), original);
    }

    #[test]
    fn test_execute_type_signals_successful_replacement_uses_atomic_write_path() {
        let (workspace, items_dir, track_id, binding, rustdoc_path) = setup_workspace();
        let observer = RustdocLaunchObserver::using_json_path(rustdoc_path);

        execute_with_observer(&items_dir, &track_id, workspace.path(), &binding, &observer)
            .unwrap();

        let path = signal_path(&items_dir, &track_id);
        assert!(path.exists());
        let persisted =
            type_signals_codec::decode(&std::fs::read_to_string(path).unwrap()).unwrap();
        let expected = current_document(workspace.path(), &items_dir, &track_id);
        assert_eq!(
            persisted.cache_key(),
            expected.cache_key(),
            "the replacement must persist all three current authoritative hashes"
        );
        assert_eq!(
            persisted.signals(),
            expected.signals(),
            "the replacement must persist the evaluator's expected signal set"
        );
        assert!(
            std::fs::read_dir(items_dir.join(track_id.as_ref())).unwrap().all(|entry| !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .starts_with(".tmp-")),
            "the atomic writer must leave no temporary replacement file"
        );
    }
}
