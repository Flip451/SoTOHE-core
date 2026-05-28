//! Public resolution façade for the `track` command family.
//!
//! Wraps the private `resolve_track_id*` helpers in `track/mod.rs` as `CliApp`
//! methods so that `apps/cli` callers never need to import infra/usecase directly.

use std::path::PathBuf;

use crate::CliApp;

use super::{
    resolve_project_root, resolve_track_id, resolve_track_id_for_write, resolve_track_id_from_root,
    resolve_track_id_inner, validate_track_id_str,
};

impl CliApp {
    /// Resolve a track ID for a READ operation, anchored to `items_dir`.
    ///
    /// When `explicit_id` is `Some`, it is returned as-is (git discovery skipped).
    /// When `None`, the current branch is used to derive the track ID.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id(
        &self,
        explicit_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<String, String> {
        resolve_track_id(explicit_id, &items_dir)
    }

    /// Resolve a track ID for a READ operation, anchored to `workspace_root`.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id_from_root(
        &self,
        explicit_id: Option<String>,
        workspace_root: PathBuf,
    ) -> Result<String, String> {
        resolve_track_id_from_root(explicit_id, &workspace_root)
    }

    /// Resolve a track ID for a WRITE operation, anchored to `items_dir`.
    ///
    /// Branch is always read; explicit ID must match the branch-derived ID.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id_for_write(
        &self,
        explicit_id: Option<String>,
        items_dir: PathBuf,
    ) -> Result<String, String> {
        resolve_track_id_for_write(explicit_id, &items_dir)
    }

    /// Resolve a track ID for a WRITE operation, anchored to `workspace_root`.
    ///
    /// # Errors
    /// Returns a human-readable error string on failure.
    pub fn track_resolve_id_from_root_for_write(
        &self,
        explicit_id: Option<String>,
        workspace_root: PathBuf,
    ) -> Result<String, String> {
        resolve_track_id_inner(explicit_id, &workspace_root, true)
    }

    /// Validate a track ID string (lowercase slug format).
    ///
    /// # Errors
    /// Returns an error string when the slug format is invalid.
    pub fn track_validate_id(&self, value: &str) -> Result<(), String> {
        validate_track_id_str(value)
    }

    /// Resolve the project root from an items_dir path (`<root>/track/items`).
    ///
    /// # Errors
    /// Returns an error string when the path structure is not canonical.
    pub fn track_resolve_project_root(&self, items_dir: PathBuf) -> Result<PathBuf, String> {
        resolve_project_root(&items_dir)
    }
}
