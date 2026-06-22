//! `track tddd` subcommands — `TrackCompositionRoot` impl methods.
//!
//! Each method accepts `Option<String>` for `track_id` and resolves it internally:
//! - **WRITE operations** call `super::resolve_track_id_for_write` (branch guard enforced).
//! - **READ operations** call `super::resolve_track_id` or `super::resolve_track_id_from_root`.

use std::path::PathBuf;
use std::sync::Arc;

use crate::CommandOutcome;
use crate::error::CompositionError;
use crate::track::composition_root::TrackCompositionRoot;

impl TrackCompositionRoot {
    /// Evaluate domain type signals via rustdoc schema export.
    ///
    /// WRITE operation: the current branch must match `track/<track_id>`.
    ///
    /// Absent catalogue files are always skipped unconditionally (no gate-vs-direct
    /// distinction). Present catalogues are always evaluated strictly.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_type_signals(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
        use usecase::type_signals::{
            TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService,
        };

        let items_dir = workspace_root.join("track").join("items");
        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)
            .map_err(CompositionError::AdapterInit)?;

        // Resolve the current git branch for the CN-07 guard (TypeSignalsInteractor requires it).
        let branch = SystemGitRepo::discover_from(&workspace_root)
            .map_err(|e| CompositionError::AdapterInit(format!("cannot discover git repo: {e}")))?
            .current_branch()
            .map_err(|e| {
                CompositionError::Infrastructure(format!("cannot read current branch: {e}"))
            })?
            .ok_or_else(|| {
                CompositionError::Infrastructure(
                    "cannot read current branch: git rev-parse --abbrev-ref HEAD returned non-zero"
                        .to_owned(),
                )
            })?;

        let layer_bindings = Arc::new(FsTdddLayerBindingsAdapter::new());
        let executor = Arc::new(TypeSignalsExecutorAdapter::new());
        let interactor = TypeSignalsInteractor::new(layer_bindings, executor);

        interactor
            .run(TypeSignalsRequest {
                items_dir,
                track_id: resolved_id,
                branch,
                workspace_root,
                layer,
            })
            .map_err(|e| CompositionError::Usecase(e.to_string()))?;

        Ok(CommandOutcome::success(None))
    }

    /// Render a mermaid type graph from rustdoc schema export.
    ///
    /// T008: This command is removed. Use `sotp track catalogue-impl-signals` instead.
    ///
    /// # Errors
    /// Always returns `Err` explaining the command is removed.
    pub fn track_type_graph(
        &self,
        _items_dir: PathBuf,
        _track_id: Option<String>,
        _workspace_root: PathBuf,
        _layer: Option<String>,
        _cluster_depth: usize,
        _edges: String,
    ) -> Result<CommandOutcome, CompositionError> {
        Err(CompositionError::WiringFailed(
            "sotp track type-graph is removed in T008. \
             Use `sotp track catalogue-impl-signals` instead."
                .to_owned(),
        ))
    }

    /// Render the rustdoc-input baseline graph (Reality View) for a track.
    ///
    /// WRITE operation: the current branch must match `track/<track_id>`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_baseline_graph(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layers: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::tddd::baseline_graph_loader_adapter::BaselineGraphLoaderAdapter;
        use infrastructure::tddd::baseline_graph_renderer_adapter::BaselineGraphRendererAdapter;
        use infrastructure::tddd::baseline_graph_writer_adapter::BaselineGraphWriterAdapter;
        use usecase::baseline_graph_workflow::{
            RenderBaselineGraph, RenderBaselineGraphCommand, RenderBaselineGraphInteractor,
        };
        use usecase::{LayerId, TrackId};

        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)
            .map_err(CompositionError::AdapterInit)?;

        let typed_track_id = TrackId::try_new(resolved_id.clone()).map_err(|e| {
            CompositionError::WiringFailed(format!("invalid track ID '{resolved_id}': {e}"))
        })?;

        let layer_filter_parsed: Option<Vec<LayerId>> = layers
            .as_deref()
            .map(parse_layer_filter_ids)
            .transpose()
            .map_err(CompositionError::WiringFailed)?;

        let rules_path = workspace_root.join("architecture-rules.json");
        let loader =
            BaselineGraphLoaderAdapter::new(items_dir.clone(), rules_path, workspace_root.clone());
        let writer = BaselineGraphWriterAdapter::new(items_dir.clone(), workspace_root.clone());
        let style_config_path = workspace_root.join(".harness/config/baseline-graph-style.toml");
        let renderer = BaselineGraphRendererAdapter::new(style_config_path);

        let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);
        let renderer_ref: &dyn RenderBaselineGraph = &interactor;
        let cmd = RenderBaselineGraphCommand {
            track_id: typed_track_id,
            layer_filter: layer_filter_parsed,
        };
        let out = renderer_ref
            .execute(&cmd)
            .map_err(|e| CompositionError::Usecase(format!("baseline-graph render failed: {e}")))?;

        let msg = format!(
            "[OK] baseline-graph: wrote depth-1 overview + depth-2 cluster files for track '{}' \
             (layers={}, files={})",
            resolved_id, out.rendered_layer_count, out.written_file_count,
        );
        Ok(CommandOutcome::success(Some(msg)))
    }

    /// Render the catalogue-input contract map for a track.
    ///
    /// WRITE operation: the current branch must match `track/<track_id>`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_contract_map(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layers: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::tddd::contract_map_adapter::{FsCatalogueLoader, FsContractMapWriter};
        use infrastructure::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter;
        use usecase::contract_map_workflow::{
            RenderContractMap, RenderContractMapCommand, RenderContractMapInteractor,
        };
        use usecase::{LayerId, TrackId};

        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)
            .map_err(CompositionError::AdapterInit)?;

        let typed_track_id = TrackId::try_new(resolved_id.clone()).map_err(|e| {
            CompositionError::WiringFailed(format!("invalid track ID '{resolved_id}': {e}"))
        })?;

        let layer_filter_parsed: Option<Vec<LayerId>> = layers
            .as_deref()
            .map(parse_layer_filter_ids)
            .transpose()
            .map_err(CompositionError::WiringFailed)?;

        let rules_path = workspace_root.join("architecture-rules.json");
        let loader = FsCatalogueLoader::new(items_dir.clone(), rules_path, workspace_root.clone());
        let writer = FsContractMapWriter::new(items_dir.clone(), workspace_root.clone());
        let style_config_path = workspace_root.join(".harness/config/contract-map-style.toml");
        let renderer = ContractMapRendererAdapter::new(style_config_path);

        let interactor = RenderContractMapInteractor::new(loader, renderer, writer);
        let renderer_ref: &dyn RenderContractMap = &interactor;
        let cmd = RenderContractMapCommand {
            track_id: typed_track_id,
            layer_filter: layer_filter_parsed,
        };
        let out = renderer_ref
            .execute(&cmd)
            .map_err(|e| CompositionError::Usecase(format!("contract-map render failed: {e}")))?;

        let msg = format!(
            "[OK] contract-map: wrote track/items/{resolved_id}/contract-map.md \
             (layers={}, entries={})",
            out.rendered_layer_count, out.total_entry_count,
        );
        Ok(CommandOutcome::success(Some(msg)))
    }

    /// Regenerate catalogue-spec-signals.json for each catalogue-spec-enabled layer.
    ///
    /// WRITE operation: the current branch must match `track/<track_id>`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_catalogue_spec_signals(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
        use infrastructure::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};
        use usecase::TrackId;

        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)
            .map_err(CompositionError::AdapterInit)?;

        // Validate track_id format (CN-01 / AC-03). Must happen before any filesystem access.
        TrackId::try_new(&resolved_id).map_err(|e| {
            CompositionError::WiringFailed(format!("invalid track ID '{resolved_id}': {e}"))
        })?;

        // Security: verify the items_dir root itself is not a symlink.
        match items_dir.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(CompositionError::WiringFailed(format!(
                    "symlink guard: refusing to follow symlink at items_dir: {}",
                    items_dir.display()
                )));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(CompositionError::Infrastructure(format!(
                    "symlink guard: cannot stat items_dir {}: {e}",
                    items_dir.display()
                )));
            }
        }

        // Security: verify the track directory itself is not a symlink.
        let track_dir = items_dir.join(&resolved_id);
        match track_dir.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(CompositionError::WiringFailed(format!(
                    "symlink guard: refusing to follow symlink at track directory: {}",
                    track_dir.display()
                )));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(CompositionError::Infrastructure(format!(
                    "symlink guard: cannot stat track directory {}: {e}",
                    track_dir.display()
                )));
            }
        }

        // Resolve layers (fail-closed).
        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings = load_tddd_layers(&rules_path, &workspace_root).map_err(|e| match e {
            LoadTdddLayersError::Io { path, source } => {
                CompositionError::ConfigLoad(format!("{}: {source}", path.display()))
            }
            LoadTdddLayersError::Parse(err) => {
                CompositionError::ConfigLoad(format!("{}: {err}", rules_path.display()))
            }
        })?;

        let bindings = if let Some(filter) = layer.as_deref() {
            let Some(binding) = bindings.iter().find(|b| b.layer_id() == filter) else {
                return Err(CompositionError::WiringFailed(format!(
                    "layer '{filter}' is not tddd.enabled in architecture-rules.json"
                )));
            };
            vec![binding.clone()]
        } else {
            bindings
        };

        if bindings.is_empty() {
            return Err(CompositionError::WiringFailed(
                "no tddd.enabled layers found in architecture-rules.json; nothing to evaluate"
                    .to_owned(),
            ));
        }

        let writer = FsCatalogueSpecSignalsStore::new(items_dir.clone());

        for binding in &bindings {
            if !binding.catalogue_spec_signal_enabled() {
                continue;
            }

            // Absent catalogue file for a layer must be silently skipped
            // (AC-01/AC-02). The `sotp signal calc-impl-catalog` step already skips
            // absent catalogues unconditionally; this step does the same so that
            // the full `track-active-gate` chain (type-signals → catalogue-spec-signals
            // → views sync) succeeds at Phase 0/1 before any catalogue exists.
            // When a catalogue IS present it is evaluated normally: a red signal
            // still blocks (AC-03/CN-02 — no fail-open on present catalogues).
            let catalogue_path = track_dir.join(binding.catalogue_file());
            match catalogue_path.symlink_metadata() {
                Ok(meta) if meta.file_type().is_file() => {
                    // Catalogue present and is a regular file — proceed to refresh.
                }
                Ok(_) => {
                    // Non-file entry (symlink, directory, etc.) — let refresh_one_layer
                    // handle it so the caller sees the same error as CI.
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    // Catalogue absent — remove any stale signals file so that the
                    // later `signal check-catalog-spec` does not find a signals
                    // file without its backing catalogue (which would be an error).
                    // If the signals file is also absent that is fine — nothing to do.
                    let stale_signals_path = track_dir
                        .join(format!("{}-catalogue-spec-signals.json", binding.layer_id()));
                    match std::fs::remove_file(&stale_signals_path) {
                        Ok(()) => {}
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                        Err(e) => {
                            return Err(CompositionError::Infrastructure(format!(
                                "failed to remove stale signals file '{}': {e}",
                                stale_signals_path.display(),
                            )));
                        }
                    }
                    continue;
                }
                Err(e) => {
                    return Err(CompositionError::Infrastructure(format!(
                        "cannot stat catalogue '{}' for layer '{}': {e}",
                        catalogue_path.display(),
                        binding.layer_id(),
                    )));
                }
            }

            infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
                &items_dir,
                &track_dir,
                &resolved_id,
                binding,
                &writer,
            )
            .map_err(CompositionError::Infrastructure)?;
        }

        Ok(CommandOutcome::success(None))
    }

    /// Emit canonical SHA-256 hashes for spec.json elements.
    ///
    /// READ operation.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_spec_element_hash(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        anchor: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        let resolved_id = super::resolve_track_id(track_id, &items_dir)
            .map_err(CompositionError::WiringFailed)?;

        super::validate_track_id_str(&resolved_id).map_err(CompositionError::WiringFailed)?;

        let hashes = infrastructure::track::spec_element_hash::compute_spec_element_hashes(
            items_dir,
            &resolved_id,
            anchor.as_deref(),
        )
        .map_err(|e| CompositionError::Infrastructure(e.0))?;

        let output = match anchor {
            Some(ref anchor_id) => {
                if let Some(hash) = hashes.get(anchor_id) {
                    hash.clone()
                } else {
                    return Err(CompositionError::WiringFailed(format!(
                        "anchor '{anchor_id}' not found in spec.json"
                    )));
                }
            }
            None => serde_json::to_string_pretty(&hashes)
                .map_err(|e| CompositionError::Infrastructure(format!("JSON encode error: {e}")))?,
        };

        Ok(CommandOutcome::success(Some(output)))
    }

    /// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
    ///
    /// WRITE operation: the current branch must match `track/<track_id>`.
    ///
    /// The operation is always idempotent: if the baseline file already exists it
    /// is kept as-is. To re-capture, delete the baseline file first.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_baseline_capture(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        source_workspace: Option<PathBuf>,
        layer: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::FsSymlinkGuard;
        use infrastructure::tddd::rustdoc_baseline_capture_adapter::RustdocBaselineCaptureAdapter;
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use usecase::baseline_capture::{
            BaselineCaptureInteractor, BaselineCaptureRequest, BaselineCaptureService,
        };

        let items_dir = workspace_root.join("track").join("items");
        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)
            .map_err(CompositionError::AdapterInit)?;

        let symlink_guard = Arc::new(FsSymlinkGuard::new());
        let layer_bindings = Arc::new(FsTdddLayerBindingsAdapter::new());
        let capture = Arc::new(RustdocBaselineCaptureAdapter::new());

        let interactor = BaselineCaptureInteractor::new(symlink_guard, layer_bindings, capture);

        interactor
            .run(BaselineCaptureRequest {
                track_id: resolved_id,
                workspace_root,
                source_workspace,
                layer,
            })
            .map_err(|e| CompositionError::Usecase(e.to_string()))?;

        Ok(CommandOutcome::success(None))
    }

    /// Run catalogue lint rules against a layer catalogue.
    ///
    /// READ operation.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_lint(
        &self,
        track_id: Option<String>,
        layer_id: String,
        workspace_root: PathBuf,
        rules_file: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::tddd::contract_map_adapter::FsCatalogueLoader;
        use infrastructure::tddd::fs_lint_config_loader::FsLintConfigLoader;
        use usecase::catalogue_lint_workflow::{
            RunCatalogueLint, RunCatalogueLintCommand, RunCatalogueLintError,
            RunCatalogueLintInteractor,
        };

        let resolved_id = super::resolve_track_id_from_root(track_id, &workspace_root)
            .map_err(CompositionError::WiringFailed)?;

        // Resolve the config file path: --rules-file overrides the default location.
        let config_path = rules_file
            .unwrap_or_else(|| workspace_root.join(".harness/catalogue-lint/config.json"));

        let items_dir = workspace_root.join("track/items");
        let rules_path = workspace_root.join("architecture-rules.json");
        let loader = FsCatalogueLoader::new(items_dir, rules_path, workspace_root.clone());
        let config_loader = FsLintConfigLoader::new(config_path);
        let interactor = RunCatalogueLintInteractor::new(loader, config_loader);

        let runner: &dyn RunCatalogueLint = &interactor;
        let result = runner.execute(RunCatalogueLintCommand {
            track_id: resolved_id,
            layer_id,
            rules: vec![],
        });

        let violations = match result {
            Ok(v) => v,
            Err(RunCatalogueLintError::ConfigMissing { path }) => {
                let msg = format!(
                    "lint config not found at {}. \
                     Copy `.harness/catalogue-lint/presets/ddd-strict.json` to that location \
                     to enable linting.",
                    path.display()
                );
                return Ok(CommandOutcome { stdout: None, stderr: Some(msg), exit_code: 1 });
            }
            Err(e) => {
                return Err(CompositionError::Usecase(format!("catalogue lint failed: {e}")));
            }
        };

        let mut stdout_lines = Vec::new();
        for v in &violations {
            stdout_lines.push(format!("{} on {}: {}", v.rule_kind(), v.entry_name(), v.message()));
        }
        let count = violations.len();
        let stderr_msg = format!("Found {count} violation(s)");

        if count > 0 {
            Ok(CommandOutcome {
                stdout: Some(stdout_lines.join("\n")),
                stderr: Some(stderr_msg),
                exit_code: 1,
            })
        } else {
            Ok(CommandOutcome { stdout: None, stderr: Some(stderr_msg), exit_code: 0 })
        }
    }

    /// Diagnose SoT Chain ③ (catalogue ↔ implementation) for a track.
    ///
    /// READ operation.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_catalogue_impl_signals(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::FsSymlinkGuard;
        use infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;
        use infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter;
        use infrastructure::tddd::signal_evaluator_v2::SignalEvaluatorV2;
        use infrastructure::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader;
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use usecase::catalogue_impl_signals::{
            CatalogueImplSignalsInteractor, CatalogueImplSignalsService,
        };

        let resolved_id = super::resolve_track_id_from_root(track_id, &workspace_root)
            .map_err(CompositionError::WiringFailed)?;

        let catalogue_loader = Arc::new(FsCatalogueDocumentLoader::new());
        let ext_crate_codec = Arc::new(CatalogueToExtendedCrateCodec::new());
        let evaluator = Arc::new(SignalEvaluatorV2::new());
        let rustdoc_crate_port = Arc::new(RustdocCrateAdapter::new(workspace_root.clone()));
        let layer_bindings_port = Arc::new(FsTdddLayerBindingsAdapter::new());
        let symlink_guard = Arc::new(FsSymlinkGuard::new());

        let interactor = CatalogueImplSignalsInteractor::new(
            catalogue_loader,
            ext_crate_codec,
            evaluator,
            rustdoc_crate_port,
            layer_bindings_port,
            symlink_guard,
        );

        let report = interactor
            .run(resolved_id, workspace_root, layer)
            .map_err(|e| CompositionError::Usecase(e.to_string()))?;

        let exit_code = if report.any_red { 1 } else { 0 };
        Ok(CommandOutcome { stdout: Some(report.text), stderr: None, exit_code })
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Parses a `--layers` CSV value into validated [`LayerId`] values (CN-12).
///
/// # Errors
/// Returns `Err` if any token is not a valid `LayerId`.
fn parse_layer_filter_ids(raw: &str) -> Result<Vec<usecase::LayerId>, String> {
    let mut layers = Vec::new();
    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id = usecase::LayerId::try_new(trimmed)
            .map_err(|e| format!("invalid layer id '{trimmed}': {e}"))?;
        layers.push(id);
    }
    Ok(layers)
}

// Restored from baseline 883cb682 (apps/cli/src/commands/track/tddd/contract_map.rs).
// These `parse_layer_filter_ids` tests were dropped during the cli-composition
// migration. The parser behavior (CSV split, trim, skip empty, CN-12 LayerId
// validation) is unchanged, so the coverage is restored here.
#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::missing_panics_doc
)]
mod tests {
    use super::*;
    use crate::track::composition_root::TrackCompositionRoot;

    #[test]
    fn test_parse_layer_filter_ids_single_value_succeeds() {
        let layers = parse_layer_filter_ids("domain").unwrap();
        assert_eq!(layers.len(), 1);
        assert_eq!(layers[0].as_ref(), "domain");
    }

    #[test]
    fn test_parse_layer_filter_ids_multiple_values_preserves_order() {
        let layers = parse_layer_filter_ids("infrastructure,usecase,domain").unwrap();
        assert_eq!(layers.len(), 3);
        assert_eq!(layers[0].as_ref(), "infrastructure");
        assert_eq!(layers[1].as_ref(), "usecase");
        assert_eq!(layers[2].as_ref(), "domain");
    }

    #[test]
    fn test_parse_layer_filter_ids_trims_whitespace_and_skips_empty() {
        let layers = parse_layer_filter_ids(" domain ,, usecase , ").unwrap();
        assert_eq!(layers.len(), 2);
        assert_eq!(layers[0].as_ref(), "domain");
        assert_eq!(layers[1].as_ref(), "usecase");
    }

    #[test]
    fn test_parse_layer_filter_ids_invalid_value_returns_error() {
        // A value with an internal space is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("domain core");
        assert!(result.is_err(), "layer id with space must be rejected");

        // A value starting with a digit is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("1layer");
        assert!(result.is_err(), "layer id starting with digit must be rejected");
    }

    // ── T003: catalogue-spec-signals absent-catalogue skip path ─────────────

    /// Helper: create a minimal git repo on `track/<track_id>` branch.
    fn init_git_repo_on_track_branch(root: &std::path::Path, track_id: &str) {
        let branch_name = format!("track/{track_id}");
        let run_git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .expect("git command failed to spawn");
            assert!(status.success(), "git {} exited with status {status}", args.join(" "));
        };
        run_git(&["init", "-q"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "Test"]);
        run_git(&["config", "commit.gpgsign", "false"]);
        run_git(&["commit", "--allow-empty", "-q", "-m", "init", "--no-gpg-sign"]);
        run_git(&["branch", "-m", &branch_name]);
    }

    fn minimal_active_metadata_json(track_id: &str) -> String {
        format!(
            r#"{{
  "schema_version": 5,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test Track",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z"
}}
"#
        )
    }

    fn minimal_impl_plan_json() -> &'static str {
        r#"{"schema_version":1,"tasks":[],"plan":{"summary":[],"sections":[]}}"#
    }

    fn setup_catalogue_spec_signal_track() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_on_track_branch(dir.path(), "test-track");

        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();

        let rules_json = r#"{
          "layers": [{
            "crate": "domain",
            "tddd": {
              "enabled": true,
              "catalogue_file": "domain-types.json",
              "catalogue_spec_signal": { "enabled": true }
            }
          }]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        (dir, items_dir, track_dir)
    }

    /// AC-01/AC-02: `catalogue-spec-signals` gate at Phase 0 (no catalogue file) succeeds.
    ///
    /// The gate (`track-active-gate`) calls `sotp signal calc-catalog-spec` after
    /// `sotp signal calc-impl-catalog`. When no catalogue exists, both commands
    /// must exit zero so the full gate chain succeeds at Phase 0/1.
    #[test]
    fn test_track_catalogue_spec_signals_absent_catalogue_returns_ok() {
        let (dir, items_dir, _track_dir) = setup_catalogue_spec_signal_track();

        let app = TrackCompositionRoot::new();
        let result = app.track_catalogue_spec_signals(
            items_dir,
            Some("test-track".to_owned()),
            dir.path().to_path_buf(),
            None,
        );
        assert!(
            result.is_ok(),
            "absent catalogue in catalogue-spec-signals must return Ok (Phase 0 skip), \
             got: {result:?}"
        );
    }

    #[test]
    fn test_track_catalogue_spec_signals_missing_track_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_on_track_branch(dir.path(), "test-track");

        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let rules_json = r#"{
          "layers": [{
            "crate": "domain",
            "tddd": {
              "enabled": true,
              "catalogue_file": "domain-types.json",
              "catalogue_spec_signal": { "enabled": true }
            }
          }]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        let app = TrackCompositionRoot::new();
        let result = app.track_catalogue_spec_signals(
            items_dir,
            Some("test-track".to_owned()),
            dir.path().to_path_buf(),
            None,
        );

        assert!(
            result.is_err(),
            "missing track directory must not be hidden by absent-catalogue leniency"
        );
    }

    /// CN-02/AC-03: `catalogue-spec-signals` with a PRESENT catalogue does NOT silently
    /// skip — it evaluates normally. The absent-catalogue skip must only apply when the
    /// file is genuinely absent (no fail-open on present catalogues).
    #[test]
    fn test_track_catalogue_spec_signals_present_catalogue_is_evaluated_not_skipped() {
        let (dir, items_dir, track_dir) = setup_catalogue_spec_signal_track();

        // Write a minimal v5 catalogue with a Red-signal entry.
        let v5_catalogue = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "RedType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        std::fs::write(track_dir.join("domain-types.json"), v5_catalogue).unwrap();

        let app = TrackCompositionRoot::new();
        let result = app.track_catalogue_spec_signals(
            items_dir,
            Some("test-track".to_owned()),
            dir.path().to_path_buf(),
            None,
        );

        // A present catalogue with a red signal must still be evaluated (not silently
        // skipped). The catalogue-spec-signals refresher writes the signals file — it
        // does NOT block on red signals itself (blocking is the gate's job). The
        // command must succeed (Ok) because signal computation is a regen, not a gate.
        // This test confirms the absent-catalogue skip does NOT fire when catalogue IS present.
        assert!(result.is_ok(), "present catalogue must be evaluated (not skipped): {result:?}");

        // Verify the signal file was written (catalogue was processed, not skipped).
        let signals_path = track_dir.join("domain-catalogue-spec-signals.json");
        assert!(
            signals_path.exists(),
            "signals file must be written when catalogue IS present (not silently skipped)"
        );
    }

    /// T003: stale signals file is removed when catalogue is absent.
    ///
    /// If a catalogue was removed/renamed but a previously-generated
    /// `<layer>-catalogue-spec-signals.json` is still present, the
    /// absent-catalogue arm must delete it so that the later
    /// `signal check-catalog-spec` does not find signals without
    /// a backing catalogue (which would be an error).
    #[test]
    fn test_track_catalogue_spec_signals_absent_catalogue_removes_stale_signals_file() {
        let (dir, items_dir, track_dir) = setup_catalogue_spec_signal_track();

        // Write a stale signals file (catalogue was removed but signals remained).
        let stale_signals_path = track_dir.join("domain-catalogue-spec-signals.json");
        std::fs::write(&stale_signals_path, r#"{"stale": true}"#).unwrap();
        assert!(stale_signals_path.exists(), "pre-condition: stale signals file must exist");

        let app = TrackCompositionRoot::new();
        let result = app.track_catalogue_spec_signals(
            items_dir,
            Some("test-track".to_owned()),
            dir.path().to_path_buf(),
            None,
        );

        assert!(
            result.is_ok(),
            "absent catalogue must return Ok even with a stale signals file, got: {result:?}"
        );
        assert!(
            !stale_signals_path.exists(),
            "stale signals file must be removed when catalogue is absent"
        );
    }
}
