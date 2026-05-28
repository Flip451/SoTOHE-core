//! `track tddd` subcommands — CliApp impl methods.

use std::path::PathBuf;

use crate::{CliApp, CommandOutcome};

impl CliApp {
    /// Evaluate domain type signals via rustdoc schema export.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_type_signals(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, workspace_root, layer);
        Err(String::from("not implemented"))
    }

    /// Render a mermaid type graph from rustdoc schema export.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_type_graph(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
        cluster_depth: usize,
        edges: String,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, workspace_root, layer, cluster_depth, edges);
        Err(String::from("not implemented"))
    }

    /// Render the rustdoc-input baseline graph (Reality View) for a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_baseline_graph(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layers: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, workspace_root, layers);
        Err(String::from("not implemented"))
    }

    /// Render the catalogue-input contract map for a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_contract_map(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layers: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, workspace_root, layers);
        Err(String::from("not implemented"))
    }

    /// Regenerate catalogue-spec-signals.json for each catalogue-spec-enabled layer.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_catalogue_spec_signals(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, workspace_root, layer);
        Err(String::from("not implemented"))
    }

    /// Emit canonical SHA-256 hashes for spec.json elements.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_spec_element_hash(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        anchor: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (items_dir, track_id, anchor);
        Err(String::from("not implemented"))
    }

    /// Capture the current TypeGraph as a baseline snapshot for TDDD reverse signal filtering.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_baseline_capture(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        source_workspace: Option<PathBuf>,
        layer: Option<String>,
        force: bool,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, workspace_root, source_workspace, layer, force);
        Err(String::from("not implemented"))
    }

    /// Run catalogue lint rules against a layer catalogue.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_lint(
        &self,
        track_id: Option<String>,
        layer_id: String,
        workspace_root: PathBuf,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, layer_id, workspace_root);
        Err(String::from("not implemented"))
    }

    /// Diagnose SoT Chain ③ (catalogue ↔ implementation) for a track.
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails or is not yet implemented.
    pub fn track_catalogue_impl_signals(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, String> {
        let _ = (track_id, workspace_root, layer);
        Err(String::from("not implemented"))
    }
}
