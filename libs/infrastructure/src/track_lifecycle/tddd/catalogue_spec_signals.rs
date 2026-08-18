//! System adapter for the Track TDDD catalogue-spec signals port.

use std::fs;
use std::path::Path;

use domain::{ConfidenceSignal, SignalCounts, TrackId};
use usecase::track_lifecycle::tddd::catalogue_spec_signals::{
    TrackCatalogueSpecSignalsCommand, TrackCatalogueSpecSignalsError,
    TrackCatalogueSpecSignalsPort, TrackCatalogueSpecSignalsResult,
};
use usecase::track_lifecycle::{TrackLayerSelection, TrackLayerSignalResult};

use crate::tddd::fs_catalogue_spec_signals_store::FsCatalogueSpecSignalsStore;
use crate::track::symlink_guard::reject_symlinks_below;
use crate::verify::tddd_layers::{LoadTdddLayersError, TdddLayerBinding, load_tddd_layers};

/// System-backed adapter for catalogue-spec signal refresh.
pub struct SystemTrackCatalogueSpecSignalsAdapter;

impl TrackCatalogueSpecSignalsPort for SystemTrackCatalogueSpecSignalsAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackCatalogueSpecSignalsCommand,
    ) -> Result<TrackCatalogueSpecSignalsResult, TrackCatalogueSpecSignalsError> {
        let items_dir = command.items_dir.as_path();
        let workspace_root = command.workspace_root.as_path();
        ensure_not_symlink(items_dir, "items_dir")?;

        let track_dir = items_dir.join(track_id.as_ref());
        ensure_not_symlink(&track_dir, "track directory")?;

        let rules_path = workspace_root.join("architecture-rules.json");
        let bindings = load_tddd_layers(&rules_path, workspace_root).map_err(|error| {
            execution_failed(format!("layer bindings load failed: {}", layer_bindings_error(error)))
        })?;
        let bindings = select_bindings(bindings, &command.layer)?;
        if bindings.is_empty() {
            return Err(execution_failed(
                "no tddd.enabled layers found in architecture-rules.json; nothing to evaluate",
            ));
        }

        let writer = FsCatalogueSpecSignalsStore::new(command.items_dir.as_path().to_path_buf());
        let mut layers = Vec::new();

        for binding in &bindings {
            if !binding.catalogue_spec_signal_enabled() {
                continue;
            }

            let catalogue_path = track_dir.join(binding.catalogue_file());
            match catalogue_path.symlink_metadata() {
                Ok(metadata) if metadata.file_type().is_file() => {}
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    remove_stale_signals(&track_dir, binding)?;
                    layers.push(skipped_layer(binding)?);
                    continue;
                }
                Err(error) => {
                    return Err(execution_failed(format!(
                        "cannot stat catalogue '{}' for layer '{}': {error}",
                        catalogue_path.display(),
                        binding.layer_id(),
                    )));
                }
            }

            crate::tddd::catalogue_spec_signals_refresher::refresh_one_layer(
                items_dir,
                &track_dir,
                track_id.as_ref(),
                binding,
                &writer,
            )
            .map_err(|error| {
                execution_failed(format!(
                    "catalogue-spec-signals refresh failed for layer '{}': {error}",
                    binding.layer_id()
                ))
            })?;

            let counts = read_signal_counts(items_dir, &track_dir, binding)?;
            let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
                .map_err(|error| execution_failed(format!("invalid TDDD layer id: {error}")))?;
            layers.push(TrackLayerSignalResult::Evaluated { layer, counts });
        }

        Ok(TrackCatalogueSpecSignalsResult { layers })
    }
}

fn ensure_not_symlink(path: &Path, label: &str) -> Result<(), TrackCatalogueSpecSignalsError> {
    match path.symlink_metadata() {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(execution_failed(format!(
            "symlink guard: refusing to follow symlink at {label}: {}",
            path.display()
        ))),
        Ok(_) => Ok(()),
        Err(error) => Err(execution_failed(format!(
            "symlink guard: cannot stat {label} {}: {error}",
            path.display()
        ))),
    }
}

fn select_bindings(
    bindings: Vec<TdddLayerBinding>,
    layer: &TrackLayerSelection,
) -> Result<Vec<TdddLayerBinding>, TrackCatalogueSpecSignalsError> {
    match layer {
        TrackLayerSelection::All => Ok(bindings),
        TrackLayerSelection::One(selected) => bindings
            .into_iter()
            .find(|binding| binding.layer_id() == selected.as_ref())
            .map(|binding| vec![binding])
            .ok_or_else(|| {
                execution_failed(format!(
                    "layer '{}' is not tddd.enabled in architecture-rules.json",
                    selected.as_ref()
                ))
            }),
    }
}

fn remove_stale_signals(
    track_dir: &Path,
    binding: &TdddLayerBinding,
) -> Result<(), TrackCatalogueSpecSignalsError> {
    let stale_signals_path = track_dir.join(binding.catalogue_spec_signal_file());
    match fs::remove_file(&stale_signals_path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(execution_failed(format!(
            "failed to remove stale signals file '{}': {error}",
            stale_signals_path.display()
        ))),
    }
}

fn skipped_layer(
    binding: &TdddLayerBinding,
) -> Result<TrackLayerSignalResult, TrackCatalogueSpecSignalsError> {
    let layer = domain::tddd::LayerId::try_new(binding.layer_id().to_owned())
        .map_err(|error| execution_failed(format!("invalid TDDD layer id: {error}")))?;
    Ok(TrackLayerSignalResult::Skipped { layer })
}

fn read_signal_counts(
    items_dir: &Path,
    track_dir: &Path,
    binding: &TdddLayerBinding,
) -> Result<SignalCounts, TrackCatalogueSpecSignalsError> {
    let path = track_dir.join(binding.catalogue_spec_signal_file());
    match reject_symlinks_below(&path, items_dir) {
        Ok(true) => {}
        Ok(false) => {
            return Err(execution_failed(format!(
                "cannot read catalogue-spec signals '{}': file not found after refresh",
                path.display()
            )));
        }
        Err(error) => {
            return Err(execution_failed(format!("symlink guard: {}: {error}", path.display())));
        }
    }
    let json = fs::read_to_string(&path).map_err(|error| {
        execution_failed(format!(
            "cannot read catalogue-spec signals '{}': {error}",
            path.display()
        ))
    })?;
    let document = crate::tddd::catalogue_spec_signals_codec::decode(&json).map_err(|error| {
        execution_failed(format!(
            "cannot decode catalogue-spec signals '{}': {error}",
            path.display()
        ))
    })?;

    let mut blue = 0u32;
    let mut yellow = 0u32;
    let mut red = 0u32;
    for signal in document.signals {
        match signal.signal {
            ConfidenceSignal::Blue => blue = blue.saturating_add(1),
            ConfidenceSignal::Yellow => yellow = yellow.saturating_add(1),
            ConfidenceSignal::Red => red = red.saturating_add(1),
            _ => red = red.saturating_add(1),
        }
    }
    Ok(SignalCounts::new(blue, yellow, red))
}

fn layer_bindings_error(error: LoadTdddLayersError) -> String {
    match error {
        LoadTdddLayersError::Io { path, source } => {
            format!("{}: {source}", path.display())
        }
        LoadTdddLayersError::Parse(error) => error.to_string(),
    }
}

fn execution_failed(message: impl Into<String>) -> TrackCatalogueSpecSignalsError {
    TrackCatalogueSpecSignalsError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(
        message,
    ))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;

    use super::*;
    use usecase::track_lifecycle::{TrackItemsDirectory, TrackSelection, TrackWorkspaceRoot};

    fn command(root: &std::path::Path) -> TrackCatalogueSpecSignalsCommand {
        TrackCatalogueSpecSignalsCommand {
            track: TrackSelection::Explicit(
                TrackId::try_new("signals-track").expect("track id is valid"),
            ),
            items_dir: TrackItemsDirectory::try_new(root.join("track/items"))
                .expect("items directory is valid"),
            workspace_root: TrackWorkspaceRoot::try_new(root.to_path_buf())
                .expect("workspace root is valid"),
            layer: TrackLayerSelection::All,
        }
    }

    fn write_rules(root: &std::path::Path) {
        fs::write(
            root.join("architecture-rules.json"),
            r#"{
              "version": 2,
              "layers": [{
                "crate": "domain",
                "tddd": {
                  "enabled": true,
                  "catalogue_file": "domain-types.json",
                  "catalogue_spec_signal": { "enabled": true }
                }
              }]
            }"#,
        )
        .expect("architecture rules are written");
    }

    #[test]
    fn test_system_track_catalogue_spec_signals_adapter_missing_rules_returns_execution_error() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/signals-track"))
            .expect("track directory exists");

        let error = match SystemTrackCatalogueSpecSignalsAdapter.execute(
            TrackId::try_new("signals-track").expect("track id is valid"),
            command(workspace.path()),
        ) {
            Ok(_) => panic!("missing architecture rules must fail"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("layer bindings load failed"));
    }

    #[test]
    fn test_system_track_catalogue_spec_signals_adapter_absent_catalogue_returns_skipped_layer() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        fs::create_dir_all(workspace.path().join("track/items/signals-track"))
            .expect("track directory exists");
        write_rules(workspace.path());

        let result = SystemTrackCatalogueSpecSignalsAdapter
            .execute(
                TrackId::try_new("signals-track").expect("track id is valid"),
                command(workspace.path()),
            )
            .expect("absent catalogue is a skipped layer");

        assert!(matches!(
            result.layers.as_slice(),
            [TrackLayerSignalResult::Skipped { layer }] if layer.as_ref() == "domain"
        ));
    }

    #[cfg(unix)]
    #[test]
    fn test_read_signal_counts_rejects_symlink_signals_file() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let items_dir = workspace.path().join("track/items");
        let track_dir = items_dir.join("signals-track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        write_rules(workspace.path());

        let target = workspace.path().join("outside.json");
        fs::write(&target, "{}").expect("symlink target exists");
        std::os::unix::fs::symlink(&target, track_dir.join("domain-catalogue-spec-signals.json"))
            .expect("signals symlink exists");

        let binding =
            load_tddd_layers(&workspace.path().join("architecture-rules.json"), workspace.path())
                .expect("layer bindings load")
                .into_iter()
                .next()
                .expect("domain binding exists");

        let error = match read_signal_counts(&items_dir, &track_dir, &binding) {
            Ok(_) => panic!("symlink signals file must be rejected"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("symlink guard"));
    }
}
