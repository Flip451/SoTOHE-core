//! `TrackCompositionRoot::catalogue_lint_check_active_track` — split out of
//! `tddd.rs` to stay under `architecture-rules.json`'s `module_limits.max_lines`
//! (see `libs/domain/src/tddd/catalogue_linter.rs` for the same `#[path]`
//! extraction pattern used for `catalogue_linter_helpers.rs` /
//! `catalogue_linter_eval.rs` / `catalogue_linter_eval_primitives.rs`).
//!
//! Declared by `tddd.rs` via `#[path = "tddd_catalogue_lint.rs"] mod tddd_catalogue_lint;`.

use std::path::PathBuf;

use crate::CommandOutcome;
use crate::error::CompositionError;
use crate::track::composition_root::TrackCompositionRoot;

impl TrackCompositionRoot {
    /// Run the catalogue-lint ruleset across every `tddd.enabled` layer of the
    /// active track and aggregate violations (ADR
    /// `knowledge/adr/2026-07-01-0004-catalogue-primitive-obsession-guard.md`
    /// §D5: blocking from day one, no warn→block staged migration).
    ///
    /// READ operation: `track_id` resolution reuses `resolve_track_id_from_root`
    /// — the same mechanism [`Self::track_lint`] already uses (CN-07: no new
    /// track-scoping logic).
    ///
    /// Layers are enumerated via [`infrastructure::verify::tddd_layers::load_tddd_layers`]
    /// (the same helper [`Self::track_catalogue_spec_signals`] uses) — no
    /// hard-coded layer names.
    ///
    /// [`domain::tddd::catalogue_ports::CatalogueLoader::load_all`] requires
    /// every `tddd.enabled` layer's catalogue file to be present (all-or-nothing;
    /// see `infrastructure::tddd::catalogue_bulk_loader`'s fail-closed
    /// `CatalogueNotFound`). A track that has not finished Phase 2 (`type-design`)
    /// for every layer legitimately lacks some catalogue files, so this method
    /// skips the whole gate (exit 0) until every layer has one, mirroring
    /// [`Self::track_catalogue_spec_signals`]'s absent-catalogue tolerance.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails (e.g.
    /// `architecture-rules.json` missing/invalid, symlink guard rejection).
    pub fn catalogue_lint_check_active_track(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        rules_file: Option<PathBuf>,
    ) -> Result<CommandOutcome, CompositionError> {
        use infrastructure::tddd::contract_map_adapter::FsCatalogueLoader;
        use infrastructure::tddd::fs_lint_config_loader::FsLintConfigLoader;
        use infrastructure::tddd::syn_primitive_occurrence_scanner::SynPrimitiveOccurrenceScanner;
        use infrastructure::verify::tddd_layers::{LoadTdddLayersError, load_tddd_layers};
        use usecase::catalogue_lint_workflow::{
            RunCatalogueLint, RunCatalogueLintCommand, RunCatalogueLintError,
            RunCatalogueLintInteractor,
        };

        let resolved_id = crate::track::resolve_track_id_from_root(track_id, &workspace_root)?;

        // Resolve layers (fail-closed) — same helper track_catalogue_spec_signals
        // uses, so layer names are never hard-coded here.
        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings = load_tddd_layers(&rules_path, &workspace_root).map_err(|e| match e {
            LoadTdddLayersError::Io { path, source } => {
                CompositionError::ConfigLoad(format!("{}: {source}", path.display()))
            }
            LoadTdddLayersError::Parse(err) => {
                CompositionError::ConfigLoad(format!("{}: {err}", rules_path.display()))
            }
        })?;

        if bindings.is_empty() {
            return Err(CompositionError::WiringFailed(
                "no tddd.enabled layers found in architecture-rules.json; nothing to lint"
                    .to_owned(),
            ));
        }

        let items_dir = workspace_root.join("track/items");
        let track_dir = items_dir.join(&resolved_id);

        // Pre-flight: CatalogueLoader::load_all requires every tddd.enabled
        // layer's catalogue file to be present at once (all-or-nothing). Skip
        // the whole gate gracefully until every layer has one, rather than
        // surfacing a hard CatalogueNotFound error for an in-progress track.
        for binding in &bindings {
            let catalogue_path = track_dir.join(binding.catalogue_file());
            match catalogue_path.symlink_metadata() {
                Ok(meta) if meta.file_type().is_file() => {
                    // Present — keep checking the remaining layers.
                }
                Ok(_) => {
                    // Non-file entry (symlink/dir) — let the loader below
                    // surface a precise error instead of masking it here.
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                    let msg = format!(
                        "catalogue-lint skipped: layer '{}' has no catalogue file yet at {} \
                         (tolerated before/during Phase 2 type-design)",
                        binding.layer_id(),
                        catalogue_path.display(),
                    );
                    return Ok(CommandOutcome { stdout: None, stderr: Some(msg), exit_code: 0 });
                }
                Err(e) => {
                    return Err(CompositionError::Infrastructure(format!(
                        "cannot stat catalogue '{}' for layer '{}': {e}",
                        catalogue_path.display(),
                        binding.layer_id(),
                    )));
                }
            }
        }

        // Resolve the config file path: --rules-file overrides the default location.
        let config_path = rules_file
            .unwrap_or_else(|| workspace_root.join(".harness/catalogue-lint/config.json"));

        let loader = FsCatalogueLoader::new(items_dir, rules_path, workspace_root.clone());
        let config_loader = FsLintConfigLoader::new(config_path);
        let scanner = SynPrimitiveOccurrenceScanner;
        let interactor = RunCatalogueLintInteractor::new(loader, config_loader, scanner);
        let runner: &dyn RunCatalogueLint = &interactor;

        let mut stdout_lines = Vec::new();
        let mut total_violations = 0usize;

        for binding in &bindings {
            let result = runner.execute(RunCatalogueLintCommand {
                track_id: resolved_id.clone(),
                layer_id: binding.layer_id().to_owned(),
                rules: vec![],
            });

            match result {
                Ok(violations) => {
                    for v in &violations {
                        stdout_lines.push(format!(
                            "[{}] {} on {}: {}",
                            binding.layer_id(),
                            v.rule_kind(),
                            v.entry_name(),
                            v.message()
                        ));
                    }
                    total_violations += violations.len();
                }
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
                    return Err(CompositionError::Usecase(format!(
                        "catalogue lint failed for layer '{}': {e}",
                        binding.layer_id()
                    )));
                }
            }
        }

        let stderr_msg =
            format!("Found {total_violations} violation(s) across {} layer(s)", bindings.len());

        if total_violations > 0 {
            Ok(CommandOutcome {
                stdout: Some(stdout_lines.join("\n")),
                stderr: Some(stderr_msg),
                exit_code: 1,
            })
        } else {
            Ok(CommandOutcome { stdout: None, stderr: Some(stderr_msg), exit_code: 0 })
        }
    }
}
