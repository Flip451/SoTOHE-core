//! `CliApp` compatibility shims for the `track` command family.
//!
//! All methods here delegate to the corresponding `TrackCompositionRoot`
//! method.  They exist solely so that existing `apps/cli` call-sites
//! (`CliApp::new().track_*()`) continue to compile without change.
//!
//! No business logic lives here — every body is a one-line delegation.

use std::path::{Path, PathBuf};

use crate::error::CompositionError;
use crate::track::composition_root::TrackCompositionRoot;
use crate::track::fixpoint_resolve::FixpointResolveInput;
use crate::{CliApp, CommandOutcome};

// ---------------------------------------------------------------------------
// Core track operations (from track/mod.rs impl block)
// ---------------------------------------------------------------------------

impl CliApp {
    /// Delegates to [`TrackCompositionRoot::track_transition`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_transition(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        task_id: String,
        target_status: String,
        commit_hash: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_transition(
            items_dir,
            track_id,
            task_id,
            target_status,
            commit_hash,
        )
    }

    /// Delegates to [`TrackCompositionRoot::track_branch_create`].
    ///
    /// # Errors
    /// Returns `Err` when git discovery or branch creation fails.
    pub fn track_branch_create(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_branch_create(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_branch_switch`].
    ///
    /// # Errors
    /// Returns `Err` when git discovery or branch switch fails.
    pub fn track_branch_switch(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_branch_switch(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_resolve`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_resolve(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_resolve(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_views_validate`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_views_validate(
        &self,
        project_root: PathBuf,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_views_validate(project_root)
    }

    /// Delegates to [`TrackCompositionRoot::track_views_sync`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_views_sync(
        &self,
        project_root: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_views_sync(project_root, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_add_task`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_add_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_add_task(items_dir, track_id, description, section, after)
    }

    /// Delegates to [`TrackCompositionRoot::track_set_override`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_set_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        status: String,
        reason: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_set_override(items_dir, track_id, status, reason)
    }

    /// Delegates to [`TrackCompositionRoot::track_clear_override`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_clear_override(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_clear_override(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_next_task`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_next_task(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_next_task(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_task_counts`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_task_counts(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_task_counts(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_archive`].
    ///
    /// # Errors
    /// Returns `Err` when validation, `git mv`, or the optional `logs/` rename fails.
    pub fn track_archive(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_archive(items_dir, track_id)
    }
}

// ---------------------------------------------------------------------------
// Pre-resolved state operations (from track/ops.rs)
// ---------------------------------------------------------------------------

impl CliApp {
    /// Delegates to [`TrackCompositionRoot::track_add_task_resolved`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_add_task_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
        description: String,
        section: Option<String>,
        after: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_add_task_resolved(
            items_dir,
            track_id,
            description,
            section,
            after,
        )
    }

    /// Delegates to [`TrackCompositionRoot::track_set_override_resolved`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_set_override_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
        status: String,
        reason: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_set_override_resolved(items_dir, track_id, status, reason)
    }

    /// Delegates to [`TrackCompositionRoot::track_clear_override_resolved`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_clear_override_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_clear_override_resolved(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_next_task_resolved`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_next_task_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_next_task_resolved(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::track_task_counts_resolved`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_task_counts_resolved(
        &self,
        items_dir: PathBuf,
        track_id: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_task_counts_resolved(items_dir, track_id)
    }

    /// Delegates to [`TrackCompositionRoot::detect_active_track_from_branch`].
    pub fn detect_active_track_from_branch(&self, project_root: &Path) -> Option<String> {
        TrackCompositionRoot::new().detect_active_track_from_branch(project_root)
    }
}

// ---------------------------------------------------------------------------
// Resolution facade (from track/resolution.rs)
// ---------------------------------------------------------------------------

impl CliApp {
    /// Delegates to [`TrackCompositionRoot::track_resolve_id`].
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id(
        &self,
        explicit_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<String, String> {
        TrackCompositionRoot::new().track_resolve_id(explicit_id, items_dir)
    }

    /// Delegates to [`TrackCompositionRoot::track_resolve_id_from_root`].
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id_from_root(
        &self,
        explicit_id: Option<String>,
        workspace_root: PathBuf,
    ) -> Result<String, String> {
        TrackCompositionRoot::new().track_resolve_id_from_root(explicit_id, workspace_root)
    }

    /// Delegates to [`TrackCompositionRoot::track_resolve_id_for_write`].
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id_for_write(
        &self,
        explicit_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<String, String> {
        TrackCompositionRoot::new().track_resolve_id_for_write(explicit_id, items_dir)
    }

    /// Delegates to [`TrackCompositionRoot::track_resolve_id_from_root_for_write`].
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id_from_root_for_write(
        &self,
        explicit_id: Option<String>,
        workspace_root: PathBuf,
    ) -> Result<String, String> {
        TrackCompositionRoot::new()
            .track_resolve_id_from_root_for_write(explicit_id, workspace_root)
    }

    /// Delegates to [`TrackCompositionRoot::track_validate_id`].
    ///
    /// # Errors
    /// Returns an error string when the slug format is invalid.
    pub fn track_validate_id(&self, value: &str) -> Result<(), String> {
        TrackCompositionRoot::new().track_validate_id(value)
    }

    /// Delegates to [`TrackCompositionRoot::track_resolve_project_root`].
    ///
    /// # Errors
    /// Returns an error string when the path structure is not canonical.
    pub fn track_resolve_project_root(&self, items_dir: PathBuf) -> Result<PathBuf, String> {
        TrackCompositionRoot::new().track_resolve_project_root(items_dir)
    }
}

// ---------------------------------------------------------------------------
// Commit hash (from track/set_commit_hash.rs)
// ---------------------------------------------------------------------------

impl CliApp {
    /// Delegates to [`TrackCompositionRoot::track_set_commit_hash`].
    ///
    /// # Errors
    /// Returns `Err` only on unexpected internal failures.
    pub fn track_set_commit_hash(
        &self,
        track_id: &str,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_set_commit_hash(track_id)
    }
}

// ---------------------------------------------------------------------------
// TDDD subcommands (from track/tddd.rs)
// ---------------------------------------------------------------------------

impl CliApp {
    /// Delegates to [`TrackCompositionRoot::track_type_signals`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_type_signals(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_type_signals(track_id, workspace_root, layer)
    }

    /// Delegates to [`TrackCompositionRoot::track_type_graph`].
    ///
    /// # Errors
    /// Always returns `Err` (command is removed in T008).
    pub fn track_type_graph(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
        cluster_depth: usize,
        edges: String,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_type_graph(
            items_dir,
            track_id,
            workspace_root,
            layer,
            cluster_depth,
            edges,
        )
    }

    /// Delegates to [`TrackCompositionRoot::track_baseline_graph`].
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
        TrackCompositionRoot::new().track_baseline_graph(
            items_dir,
            track_id,
            workspace_root,
            layers,
        )
    }

    /// Delegates to [`TrackCompositionRoot::track_contract_map`].
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
        TrackCompositionRoot::new().track_contract_map(items_dir, track_id, workspace_root, layers)
    }

    /// Delegates to [`TrackCompositionRoot::track_catalogue_spec_signals`].
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
        TrackCompositionRoot::new().track_catalogue_spec_signals(
            items_dir,
            track_id,
            workspace_root,
            layer,
        )
    }

    /// Delegates to [`TrackCompositionRoot::track_spec_element_hash`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_spec_element_hash(
        &self,
        items_dir: PathBuf,
        track_id: Option<String>,
        anchor: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_spec_element_hash(items_dir, track_id, anchor)
    }

    /// Delegates to [`TrackCompositionRoot::track_baseline_capture`].
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
        TrackCompositionRoot::new().track_baseline_capture(
            track_id,
            workspace_root,
            source_workspace,
            layer,
        )
    }

    /// Delegates to [`TrackCompositionRoot::track_lint`].
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
        TrackCompositionRoot::new().track_lint(track_id, layer_id, workspace_root, rules_file)
    }

    /// Delegates to [`TrackCompositionRoot::track_catalogue_impl_signals`].
    ///
    /// # Errors
    /// Returns `Err` when the underlying composition logic fails.
    pub fn track_catalogue_impl_signals(
        &self,
        track_id: Option<String>,
        workspace_root: PathBuf,
        layer: Option<String>,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().track_catalogue_impl_signals(track_id, workspace_root, layer)
    }
}

// ---------------------------------------------------------------------------
// Fixpoint resolve (from track/fixpoint_resolve.rs)
// ---------------------------------------------------------------------------

impl CliApp {
    /// Delegates to [`TrackCompositionRoot::fixpoint_resolve`].
    ///
    /// # Errors
    /// Returns `Err` when validation, git discovery, or gate evaluation fails.
    pub fn fixpoint_resolve(
        &self,
        input: FixpointResolveInput,
    ) -> Result<CommandOutcome, CompositionError> {
        TrackCompositionRoot::new().fixpoint_resolve(input)
    }
}
