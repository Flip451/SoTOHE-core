//! `sotp track baseline-graph` — render the rustdoc-input baseline graph
//! (Reality View) for a track.
//!
//! Composition root that wires the usecase interactor
//! (`usecase::baseline_graph_workflow::RenderBaselineGraphInteractor`) to its
//! three secondary-port adapters:
//! * `BaselineGraphLoaderAdapter` — loads per-layer rustdoc JSON baselines (T013).
//! * `BaselineGraphRendererAdapter` — renders mermaid depth-1 overview +
//!   depth-2 cluster files. Style config at
//!   `.harness/config/baseline-graph-style.toml` (fail-closed if absent or
//!   invalid, CN-02 / AC-15).
//! * `BaselineGraphWriterAdapter` — writes depth-1 `<layer>-graph-d1/index.md`
//!   and depth-2 `<layer>-graph-d2/<cluster>.md` files to the track dir (T014).
//!
//! Symmetric to `contract_map.rs`. (IN-02 / IN-19 / AC-02 / AC-18)

use std::path::PathBuf;
use std::process::ExitCode;

use infrastructure::tddd::baseline_graph_loader_adapter::BaselineGraphLoaderAdapter;
use infrastructure::tddd::baseline_graph_renderer_adapter::BaselineGraphRendererAdapter;
use infrastructure::tddd::baseline_graph_writer_adapter::BaselineGraphWriterAdapter;
use usecase::baseline_graph_workflow::{
    RenderBaselineGraph, RenderBaselineGraphCommand, RenderBaselineGraphInteractor,
};
use usecase::{LayerId, TrackId};

use crate::CliError;

/// Render the baseline graph (Reality View) for a single track.
///
/// # Errors
///
/// Returns `CliError` when the track id is invalid, the current branch does
/// not match `track/<track_id>` (CN-07 guard), `--layers` cannot be parsed,
/// or the interactor fails (loader / renderer / writer / empty baseline /
/// unknown layer).
pub fn execute_baseline_graph(
    items_dir: PathBuf,
    track_id: String,
    workspace_root: PathBuf,
    layers: Option<String>,
) -> Result<ExitCode, CliError> {
    // Validate track_id into domain type at the CLI boundary (CN-12).
    // Runs before git discovery so a malformed track_id is always caught
    // here and never masked by "cannot discover git repo" or branch-mismatch
    // errors.
    let typed_track_id = TrackId::try_new(track_id.clone())
        .map_err(|e| CliError::Message(format!("invalid track ID '{track_id}': {e}")))?;

    // Parse and validate layer filter strings into LayerId at the CLI boundary (CN-12).
    let layer_filter_parsed: Option<Vec<LayerId>> =
        layers.as_deref().map(parse_layer_filter_ids).transpose()?;

    // Branch guard is enforced at the CLI dispatch layer (mod.rs) via
    // `resolve_track_id_from_root_for_write` (D7 / AC-18 / CN-02 / CN-03).
    // Inline duplication removed per T016.
    let rules_path = workspace_root.join("architecture-rules.json");
    let loader =
        BaselineGraphLoaderAdapter::new(items_dir.clone(), rules_path, workspace_root.clone());
    let writer = BaselineGraphWriterAdapter::new(items_dir.clone(), workspace_root.clone());

    // Inject BaselineGraphRendererAdapter (T004).
    // Style config at .harness/config/baseline-graph-style.toml (fail-closed: absent/invalid
    // config causes the interactor to return RenderBaselineGraphError::RendererFailed, CN-02).
    let style_config_path = workspace_root.join(".harness/config/baseline-graph-style.toml");
    let renderer = BaselineGraphRendererAdapter::new(style_config_path);

    let interactor = RenderBaselineGraphInteractor::new(loader, renderer, writer);

    // Dispatch through the primary port — CLI does not depend on the
    // concrete `RenderBaselineGraphInteractor` type.
    let renderer_ref: &dyn RenderBaselineGraph = &interactor;
    let cmd =
        RenderBaselineGraphCommand { track_id: typed_track_id, layer_filter: layer_filter_parsed };
    let out = renderer_ref
        .execute(&cmd)
        .map_err(|e| CliError::Message(format!("baseline-graph render failed: {e}")))?;

    println!(
        "[OK] baseline-graph: wrote depth-1 overview + depth-2 cluster files for track '{track_id}' \
         (layers={}, files={})",
        out.rendered_layer_count, out.written_file_count,
    );
    Ok(ExitCode::SUCCESS)
}

/// Parses a `--layers` CSV value into validated [`LayerId`] values (CN-12).
/// Validation that the layer is enabled happens in the interactor.
///
/// # Errors
///
/// Returns `CliError` if any token is not a valid `LayerId`.
fn parse_layer_filter_ids(raw: &str) -> Result<Vec<LayerId>, CliError> {
    let mut layers = Vec::new();
    for token in raw.split(',') {
        let trimmed = token.trim();
        if trimmed.is_empty() {
            continue;
        }
        let id = LayerId::try_new(trimmed)
            .map_err(|e| CliError::Message(format!("invalid layer id '{trimmed}': {e}")))?;
        layers.push(id);
    }
    Ok(layers)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::panic, clippy::indexing_slicing, clippy::expect_used)]
mod tests {
    use super::*;

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

    /// Verifies that `TrackId::try_new` validation runs at the CLI boundary
    /// (CN-12) before git discovery, so a malformed `track_id` is always
    /// caught here and never masked by `cannot discover git repo` or
    /// branch-mismatch errors.
    ///
    /// `../evil` is rejected by `TrackId::try_new` (contains `/`) regardless
    /// of whether `workspace_root` is a valid git repository.
    #[test]
    fn test_execute_baseline_graph_with_invalid_track_id_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let result =
            execute_baseline_graph(items_dir, "../evil".to_owned(), dir.path().into(), None);
        let err = result.expect_err("path traversal track id must be rejected by CN-12");
        let msg = format!("{err}");
        assert!(
            msg.contains("invalid track ID"),
            "error must be a CN-12 track ID validation rejection, got: {msg}"
        );
    }

    /// Verifies that `parse_layer_filter_ids` returns an error for an invalid
    /// layer ID token, covering the CN-12 `LayerId::try_new` validation path
    /// directly without going through the CN-07 branch guard.
    #[test]
    fn test_parse_layer_filter_ids_invalid_value_returns_error() {
        // A value with an internal space is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("domain core");
        assert!(result.is_err(), "layer id with space must be rejected");

        // A value starting with a digit is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("1layer");
        assert!(result.is_err(), "layer id starting with digit must be rejected");
    }

    // Note: branch guard tests for baseline-graph are exercised via the CLI dispatch layer
    // (`mod.rs` `resolve_track_id_from_root_for_write`) rather than inline in this function
    // (T016 — inline guard removed, duplicated by the shared WRITE guard in mod.rs).
    // See `mod.rs` unit tests for `resolve_track_id_from_root_for_write` for full coverage.
}
