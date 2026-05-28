//! `track tddd` subcommands — CliApp impl methods.
//!
//! Each method accepts `Option<String>` for `track_id` and resolves it internally:
//! - **WRITE operations** call `super::resolve_track_id_for_write` (branch guard enforced).
//! - **READ operations** call `super::resolve_track_id` or `super::resolve_track_id_from_root`.

use std::path::PathBuf;
use std::sync::Arc;

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Evaluate domain type signals via rustdoc schema export.
    ///
    /// WRITE operation: the current branch must match `track/<track_id>`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_type_signals(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::git_cli::{GitRepository as _, SystemGitRepo};
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use infrastructure::tddd::type_signals_executor_adapter::TypeSignalsExecutorAdapter;
        use usecase::type_signals::{
            TypeSignalsInteractor, TypeSignalsRequest, TypeSignalsService,
        };

        let items_dir = workspace_root.join("track").join("items");
        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)?;

        // Resolve the current git branch for the CN-07 guard (TypeSignalsInteractor requires it).
        let branch = SystemGitRepo::discover_from(&workspace_root)
            .map_err(|e| format!("cannot discover git repo: {e}"))?
            .current_branch()
            .map_err(|e| format!("cannot read current branch: {e}"))?
            .ok_or_else(|| {
                "cannot read current branch: git rev-parse --abbrev-ref HEAD returned non-zero"
                    .to_owned()
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
                lenient: false,
            })
            .map_err(|e| e.to_string())?;

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
    ) -> Result<CommandOutcome, String> {
        Err("sotp track type-graph is removed in T008. \
             Use `sotp track catalogue-impl-signals` instead."
            .to_owned())
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
    ) -> Result<CommandOutcome, String> {
        use infrastructure::tddd::baseline_graph_loader_adapter::BaselineGraphLoaderAdapter;
        use infrastructure::tddd::baseline_graph_renderer_adapter::BaselineGraphRendererAdapter;
        use infrastructure::tddd::baseline_graph_writer_adapter::BaselineGraphWriterAdapter;
        use usecase::baseline_graph_workflow::{
            RenderBaselineGraph, RenderBaselineGraphCommand, RenderBaselineGraphInteractor,
        };
        use usecase::{LayerId, TrackId};

        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)?;

        let typed_track_id = TrackId::try_new(resolved_id.clone())
            .map_err(|e| format!("invalid track ID '{resolved_id}': {e}"))?;

        let layer_filter_parsed: Option<Vec<LayerId>> =
            layers.as_deref().map(parse_layer_filter_ids).transpose()?;

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
        let out =
            renderer_ref.execute(&cmd).map_err(|e| format!("baseline-graph render failed: {e}"))?;

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
    ) -> Result<CommandOutcome, String> {
        use infrastructure::tddd::contract_map_adapter::{FsCatalogueLoader, FsContractMapWriter};
        use infrastructure::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter;
        use usecase::contract_map_workflow::{
            RenderContractMap, RenderContractMapCommand, RenderContractMapInteractor,
        };
        use usecase::{LayerId, TrackId};

        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)?;

        let typed_track_id = TrackId::try_new(resolved_id.clone())
            .map_err(|e| format!("invalid track ID '{resolved_id}': {e}"))?;

        let layer_filter_parsed: Option<Vec<LayerId>> =
            layers.as_deref().map(parse_layer_filter_ids).transpose()?;

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
        let out =
            renderer_ref.execute(&cmd).map_err(|e| format!("contract-map render failed: {e}"))?;

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
    ) -> Result<CommandOutcome, String> {
        use infrastructure::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
        use infrastructure::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};
        use usecase::TrackId;

        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)?;

        // Validate track_id format (CN-01 / AC-03). Must happen before any filesystem access.
        TrackId::try_new(&resolved_id)
            .map_err(|e| format!("invalid track ID '{resolved_id}': {e}"))?;

        // Security: verify the items_dir root itself is not a symlink.
        match items_dir.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "symlink guard: refusing to follow symlink at items_dir: {}",
                    items_dir.display()
                ));
            }
            Ok(_) => {}
            Err(e) => {
                return Err(format!(
                    "symlink guard: cannot stat items_dir {}: {e}",
                    items_dir.display()
                ));
            }
        }

        // Security: verify the track directory itself is not a symlink.
        let track_dir = items_dir.join(&resolved_id);
        match track_dir.symlink_metadata() {
            Ok(meta) if meta.file_type().is_symlink() => {
                return Err(format!(
                    "symlink guard: refusing to follow symlink at track directory: {}",
                    track_dir.display()
                ));
            }
            Ok(_) => {}
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => {
                return Err(format!(
                    "symlink guard: cannot stat track directory {}: {e}",
                    track_dir.display()
                ));
            }
        }

        // Resolve layers (fail-closed).
        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings = load_tddd_layers(&rules_path, &workspace_root).map_err(|e| match e {
            LoadTdddLayersError::Io { path, source } => {
                format!("{}: {source}", path.display())
            }
            LoadTdddLayersError::Parse(err) => {
                format!("{}: {err}", rules_path.display())
            }
        })?;

        let bindings = if let Some(filter) = layer.as_deref() {
            let Some(binding) = bindings.iter().find(|b| b.layer_id() == filter) else {
                return Err(format!(
                    "layer '{filter}' is not tddd.enabled in architecture-rules.json"
                ));
            };
            vec![binding.clone()]
        } else {
            bindings
        };

        if bindings.is_empty() {
            return Err(
                "no tddd.enabled layers found in architecture-rules.json; nothing to evaluate"
                    .to_owned(),
            );
        }

        let writer = FsCatalogueSpecSignalsStore::new(items_dir.clone());

        for binding in &bindings {
            if !binding.catalogue_spec_signal_enabled() {
                continue;
            }
            infrastructure::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
                &items_dir,
                &track_dir,
                &resolved_id,
                binding,
                &writer,
            )?;
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
    ) -> Result<CommandOutcome, String> {
        let resolved_id = super::resolve_track_id(track_id, &items_dir)?;

        super::validate_track_id_str(&resolved_id)?;

        let hashes = infrastructure::track::spec_element_hash::compute_spec_element_hashes(
            items_dir,
            &resolved_id,
            anchor.as_deref(),
        )
        .map_err(|e| e.0)?;

        let output = match anchor {
            Some(ref anchor_id) => {
                if let Some(hash) = hashes.get(anchor_id) {
                    hash.clone()
                } else {
                    return Err(format!("anchor '{anchor_id}' not found in spec.json"));
                }
            }
            None => serde_json::to_string_pretty(&hashes)
                .map_err(|e| format!("JSON encode error: {e}"))?,
        };

        Ok(CommandOutcome::success(Some(output)))
    }

    /// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
    ///
    /// WRITE operation: the current branch must match `track/<track_id>`.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_baseline_capture(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        source_workspace: Option<PathBuf>,
        layer: Option<String>,
        force: bool,
    ) -> Result<CommandOutcome, String> {
        use infrastructure::FsSymlinkGuard;
        use infrastructure::tddd::rustdoc_baseline_capture_adapter::RustdocBaselineCaptureAdapter;
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use usecase::baseline_capture::{
            BaselineCaptureInteractor, BaselineCaptureRequest, BaselineCaptureService,
        };

        let items_dir = workspace_root.join("track").join("items");
        let resolved_id = super::resolve_track_id_for_write(track_id, &items_dir)?;

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
                force,
            })
            .map_err(|e| e.to_string())?;

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
    ) -> Result<CommandOutcome, String> {
        use infrastructure::tddd::contract_map_adapter::FsCatalogueLoader;
        use infrastructure::tddd::in_memory_catalogue_linter::InMemoryCatalogueLinter;
        use usecase::catalogue_lint_workflow::{
            LintRuleKind, LintRuleSpec, RunCatalogueLint, RunCatalogueLintCommand,
            RunCatalogueLintInteractor,
        };

        let resolved_id = super::resolve_track_id_from_root(track_id, &workspace_root)?;

        let rules = vec![
            LintRuleSpec {
                kind: LintRuleKind::FieldEmpty,
                target_kind: "value_object".to_owned(),
                target_field: Some("expected_methods".to_owned()),
                permitted_layers: vec![],
            },
            LintRuleSpec {
                kind: LintRuleKind::KindLayerConstraint,
                target_kind: "domain_service".to_owned(),
                target_field: None,
                permitted_layers: vec!["domain".to_owned(), "usecase".to_owned()],
            },
        ];

        let items_dir = workspace_root.join("track/items");
        let rules_path = workspace_root.join("architecture-rules.json");
        let loader = FsCatalogueLoader::new(items_dir, rules_path, workspace_root.clone());
        let linter = InMemoryCatalogueLinter::new();
        let interactor = RunCatalogueLintInteractor::new(loader, linter);

        let runner: &dyn RunCatalogueLint = &interactor;
        let violations = runner
            .execute(RunCatalogueLintCommand { track_id: resolved_id, layer_id, rules })
            .map_err(|e| format!("catalogue lint failed: {e}"))?;

        let mut stdout_lines = Vec::new();
        for v in &violations {
            stdout_lines.push(format!(
                "{:?} on {}: {}",
                v.rule_kind(),
                v.entry_name(),
                v.message()
            ));
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
    ) -> Result<CommandOutcome, String> {
        use infrastructure::FsSymlinkGuard;
        use infrastructure::tddd::catalogue_to_extended_crate_codec::CatalogueToExtendedCrateCodec;
        use infrastructure::tddd::rustdoc_crate_adapter::RustdocCrateAdapter;
        use infrastructure::tddd::signal_evaluator_v2::SignalEvaluatorV2;
        use infrastructure::tddd::tddd_catalogue_document_loader::FsCatalogueDocumentLoader;
        use infrastructure::tddd::tddd_layer_bindings_adapter::FsTdddLayerBindingsAdapter;
        use usecase::catalogue_impl_signals::{
            CatalogueImplSignalsInteractor, CatalogueImplSignalsService,
        };

        let resolved_id = super::resolve_track_id_from_root(track_id, &workspace_root)?;

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

        let report =
            interactor.run(resolved_id, workspace_root, layer).map_err(|e| e.to_string())?;

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
