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
use domain::tddd::catalogue_v2::CrateName;
use domain::tddd::type_signals_doc::{BaselineHash, TypeSignalsCacheKey, TypeSignalsDocument};
use domain::{FreeText, Timestamp, TrackId};
use freshness::{RustdocJsonPathProvider, decide_reuse_for_recorded_document};
use inputs::{
    read_head_commit, read_utf8_file_limited, verify_evaluation_inputs_unchanged, worktree_is_clean,
};
use signal_builder::build_type_signals_from_report;
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
    let head_commit = read_head_commit(&canonical_workspace)?;
    let current_key = TypeSignalsCacheKey::new(
        declaration_hash.clone(),
        head_commit.clone(),
        baseline_hash.clone(),
    );
    let recorded = read_utf8_file_limited(&signal_path, MAX_TYPE_SIGNALS_BYTES)
        .ok()
        .and_then(|text| type_signals_codec::decode(&text).ok());
    let reuse_decision = decide_reuse_for_recorded_document(
        recorded.as_ref(),
        &current_key,
        worktree_is_clean(&canonical_workspace)?,
    );
    match reuse_decision {
        domain::TypeSignalsReuseDecision::SkipEvaluation => {
            verify_evaluation_inputs_unchanged(
                &canonical_workspace,
                &catalogue_path,
                &baseline_path,
                &current_key,
            )?;
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
        target_crate,
        binding,
        content,
        declaration_hash,
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
    target_crate: &str,
    binding: &TdddLayerBinding,
    rustdoc_json: String,
    declaration_hash: domain::CatalogueDeclarationHash,
    head_commit: domain::CommitHash,
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
        catalogue_path,
        baseline_path,
        &TypeSignalsCacheKey::new(
            declaration_hash.clone(),
            head_commit.clone(),
            baseline_hash.clone(),
        ),
    )?;
    let generated_at = Timestamp::new(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string())
        .map_err(|error| {
            EvaluateSignalsError::evaluation(format!("cannot create timestamp: {error}"))
        })?;
    let document = TypeSignalsDocument::new(
        generated_at,
        TypeSignalsCacheKey::new(declaration_hash, head_commit, baseline_hash),
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
#[allow(clippy::unwrap_used, clippy::panic)]
mod tests {
    use super::*;
    use crate::verify::tddd_layers::parse_tddd_layers;
    use usecase::merge_gate::{BlobFetchResult, TrackBlobReader};

    fn rustdoc_json() -> String {
        format!(
            r#"{{"root":0,"crate_version":null,"includes_private":false,"index":{{}},"paths":{{}},"external_crates":{{}},"format_version":{},"target":{{"triple":"","target_features":[]}}}}"#,
            rustdoc_types::FORMAT_VERSION
        )
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
