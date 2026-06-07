//! Rendering of `contract-map.md` for the active track.

use std::path::{Path, PathBuf};

use domain::TrackId;
use domain::tddd::ContractMapRenderOptions;
use domain::tddd::{
    CatalogueLoader, CatalogueLoaderError, ContractMapRenderer, ContractMapRendererError,
};

use super::RenderError;
use super::TRACK_ITEMS_DIR;
use crate::tddd::contract_map_adapter::FsCatalogueLoader;
use crate::tddd::contract_map_renderer_adapter::ContractMapRendererAdapter;

/// Renders `contract-map.md` for the active track and appends the path to
/// `changed` when the content actually differs on disk.
///
/// # Errors
///
/// Returns `RenderError::Io` for hard configuration errors (missing
/// `architecture-rules.json`, style-config absent/invalid).  Non-fatal
/// catalogue errors (`CatalogueNotFound`, `DecodeFailed`) warn and return
/// `Ok(())`.
pub(super) fn render_contract_map_view(
    root: &Path,
    track_dir: &Path,
    track_id_str: Option<&str>,
    changed: &mut Vec<PathBuf>,
) -> Result<(), RenderError> {
    let Some(track_id_raw) = track_id_str else {
        return Ok(());
    };
    let Ok(track_id) = TrackId::try_new(track_id_raw) else {
        eprintln!(
            "warning: skipping contract-map.md render for {} (invalid track id)",
            track_dir.display()
        );
        return Ok(());
    };

    let items_dir = root.join(TRACK_ITEMS_DIR);
    let rules_path = root.join("architecture-rules.json");
    let loader = FsCatalogueLoader::new(items_dir, rules_path, root.to_path_buf());
    let (layer_order, catalogues) = match loader.load_all(&track_id) {
        Ok(result) => result,
        // Hard errors: architecture-rules.json missing / malformed / symlinked.
        // These are configuration errors that must be visible regardless of track
        // status (a done track with a missing arch-rules file is still broken).
        Err(CatalogueLoaderError::LayerDiscoveryFailed { reason }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "architecture-rules.json error for contract-map render at {}: {reason}",
                track_dir.display()
            ))));
        }
        Err(CatalogueLoaderError::SymlinkRejected { path }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "symlink rejected at {} (contract-map render for {})",
                path.display(),
                track_dir.display()
            ))));
        }
        // Hard error: non-symlink I/O failure reading a catalogue artifact or the
        // architecture-rules.json dependency graph.  Propagate so that callers
        // detect file-system corruption regardless of track status.
        Err(CatalogueLoaderError::IoError { path, reason }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "I/O error at {} (contract-map render for {}): {reason}",
                path.display(),
                track_dir.display()
            ))));
        }
        // Hard error: a cycle in `may_depend_on` is an invalid architecture-rules.json
        // configuration — the contract-map cannot be rendered until the cycle is
        // resolved, so propagate as a hard error rather than silently skipping.
        Err(CatalogueLoaderError::TopologicalSortFailed { reason }) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "architecture-rules.json cycle detected (contract-map render for {}): {reason}",
                track_dir.display()
            ))));
        }
        // Non-fatal catalogue-level errors (absent catalogue, decode failure): warn
        // and skip. The authoritative fail-closed gate for TDDD correctness lives in
        // `spec_states::evaluate_layer_catalogue`.
        Err(
            e @ (CatalogueLoaderError::CatalogueNotFound { .. }
            | CatalogueLoaderError::DecodeFailed { .. }),
        ) => {
            eprintln!(
                "warning: skipping contract-map.md render for {} ({})",
                track_dir.display(),
                e
            );
            return Ok(());
        }
    };
    if layer_order.is_empty() {
        // No TDDD-enabled layers on this track — nothing to render.
        return Ok(());
    }

    let opts = ContractMapRenderOptions::default();
    // Use ContractMapRendererAdapter for rendering.
    // Style config at `.harness/config/contract-map-style.toml` relative to workspace root.
    let style_config_path = root.join(".harness/config/contract-map-style.toml");
    let renderer = ContractMapRendererAdapter::new(style_config_path);
    let catalogues_vec: Vec<_> = catalogues.values().cloned().collect();
    let content = match renderer.render(&catalogues_vec, &layer_order, &opts) {
        Ok(c) => c,
        // Style-config errors are hard configuration errors (CN-02 / AC-11 fail-closed):
        // an absent or unreadable/invalid style config means the contract-map cannot be
        // rendered correctly and must surface as an error, not a silent skip.
        Err(
            e @ (ContractMapRendererError::StyleConfigNotFound { .. }
            | ContractMapRendererError::StyleConfigInvalid { .. }),
        ) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "contract-map style config error for {} (CN-02): {e}",
                track_dir.display()
            ))));
        }
        // Render-logic failures (RenderFailed, e.g. a missing required [edge.*] key or a
        // malformed TypeRef) are fatal in view-sync: leaving a stale contract-map.md in
        // place while silently returning Ok would be fail-open and mislead callers.
        // Surface the error so sync_rendered_views fails closed, consistent with the
        // StyleConfig* arms above.
        Err(e) => {
            return Err(RenderError::Io(std::io::Error::other(format!(
                "contract-map render failed for {} (RenderFailed): {e}",
                track_dir.display()
            ))));
        }
    };
    let contract_map_path = track_dir.join("contract-map.md");
    let old = match std::fs::read_to_string(&contract_map_path) {
        Ok(existing) => Some(existing),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
        Err(e) => {
            eprintln!(
                "warning: cannot read existing contract-map.md for {}: {e}",
                track_dir.display()
            );
            return Ok(());
        }
    };
    let rendered_str: &str = content.as_ref();
    if old.as_deref().is_none_or(|existing| !super::rendered_matches(existing, rendered_str)) {
        if let Err(e) = super::super::atomic_write::atomic_write_file(
            &contract_map_path,
            rendered_str.as_bytes(),
        ) {
            eprintln!("warning: cannot write contract-map.md for {}: {e}", track_dir.display());
            return Ok(());
        }
        changed.push(contract_map_path);
    }
    Ok(())
}
