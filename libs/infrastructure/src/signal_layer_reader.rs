//! Local filesystem adapter for [`usecase::signal::SignalLayerReader`].
//!
//! [`LocalSignalLayerReaderAdapter`] resolves the active-track ID from the
//! current git branch, enumerates TDDD-enabled layers from
//! `architecture-rules.json`, and reads per-layer catalogue bytes from
//! `track/items/<track-id>/<catalogue-file>`.
//!
//! No path is exposed through the port contract — path construction is an
//! infrastructure responsibility (D8-4).

use std::path::PathBuf;

use domain::TrackId;
use domain::tddd::LayerId;
use usecase::signal::{SignalLayerReader, SignalLayerReaderError};

use crate::git_cli::{GitRepository as _, SystemGitRepo};
use crate::verify::tddd_layers;

/// Local filesystem adapter implementing [`SignalLayerReader`].
///
/// Workspace root is discovered once via `SystemGitRepo::discover()`.
/// Layers are re-enumerated from `architecture-rules.json` on each
/// `enabled_layers` call (cheap disk read; no caching needed for CLI use).
#[derive(Debug)]
pub struct LocalSignalLayerReaderAdapter {
    workspace_root: PathBuf,
}

impl LocalSignalLayerReaderAdapter {
    /// Discover the git workspace root from the current working directory and
    /// construct the adapter.
    ///
    /// # Errors
    ///
    /// Returns `Err(String)` when git discovery fails.
    pub fn discover() -> Result<Self, String> {
        let repo = SystemGitRepo::discover()
            .map_err(|e| format!("LocalSignalLayerReaderAdapter: cannot discover git repo: {e}"))?;
        Ok(Self { workspace_root: repo.root().to_path_buf() })
    }

    /// Construct from an explicit workspace root (useful for tests).
    #[must_use]
    pub fn new(workspace_root: PathBuf) -> Self {
        Self { workspace_root }
    }
}

impl SignalLayerReader for LocalSignalLayerReaderAdapter {
    /// Resolve the active-track `TrackId` from the current git branch.
    ///
    /// Expects a branch name of the form `track/<track-id>`.
    /// Returns `TrackIdUnresolved` when the branch is not in that form or when
    /// git fails.
    fn active_track_id(&self) -> Result<TrackId, SignalLayerReaderError> {
        let repo = SystemGitRepo::discover_from(&self.workspace_root)
            .map_err(|_| SignalLayerReaderError::Io)?;
        let branch = repo
            .current_branch()
            .map_err(|_| SignalLayerReaderError::Io)?
            .ok_or(SignalLayerReaderError::TrackIdUnresolved)?;

        // Branch must be `track/<slug>`.
        let slug =
            branch.strip_prefix("track/").ok_or(SignalLayerReaderError::TrackIdUnresolved)?;

        TrackId::try_new(slug).map_err(|_| SignalLayerReaderError::TrackIdUnresolved)
    }

    /// Enumerate TDDD-enabled layer IDs from `architecture-rules.json`.
    fn enabled_layers(&self, _track_id: TrackId) -> Result<Vec<LayerId>, SignalLayerReaderError> {
        let rules_path = self.workspace_root.join("architecture-rules.json");
        let bindings = tddd_layers::load_tddd_layers(&rules_path, &self.workspace_root)
            .map_err(|_| SignalLayerReaderError::Io)?;

        bindings
            .into_iter()
            .map(|b| {
                LayerId::try_new(b.layer_id().to_owned()).map_err(|_| SignalLayerReaderError::Io)
            })
            .collect()
    }

    /// Read raw bytes of the configured catalogue file for the given track/layer.
    ///
    /// Returns `Ok(None)` when the file is absent.
    fn catalogue_bytes(
        &self,
        track_id: TrackId,
        layer: LayerId,
    ) -> Result<Option<Vec<u8>>, SignalLayerReaderError> {
        let rules_path = self.workspace_root.join("architecture-rules.json");
        let bindings = tddd_layers::load_tddd_layers(&rules_path, &self.workspace_root)
            .map_err(|_| SignalLayerReaderError::Io)?;
        let binding = tddd_layers::find_binding(&bindings, layer.as_ref())
            .ok_or(SignalLayerReaderError::Io)?;
        let catalogue_path = self
            .workspace_root
            .join("track")
            .join("items")
            .join(track_id.as_ref())
            .join(binding.catalogue_file());

        match crate::track::symlink_guard::reject_symlinks_below(
            &catalogue_path,
            &self.workspace_root,
        ) {
            Ok(true) => {}
            Ok(false) => return Ok(None),
            Err(_) => return Err(SignalLayerReaderError::Io),
        }

        match std::fs::read(&catalogue_path) {
            Ok(bytes) => Ok(Some(bytes)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(_) => Err(SignalLayerReaderError::Io),
        }
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    use crate::verify::test_support::{git_init, run_git};

    const ARCH_RULES: &str = r#"{
      "layers": [
        { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "domain-types.json" } },
        { "crate": "usecase", "tddd": { "enabled": true, "catalogue_file": "usecase-types.json" } },
        { "crate": "infrastructure", "tddd": { "enabled": false } }
      ]
    }"#;

    const ARCH_RULES_CUSTOM_CATALOGUE: &str = r#"{
      "layers": [
        { "crate": "domain", "tddd": { "enabled": true, "catalogue_file": "custom-types.json" } }
      ]
    }"#;

    fn git_repo_on_branch(root: &std::path::Path, branch: &str) {
        git_init(root);
        std::fs::write(root.join(".gitignore"), "").unwrap();
        run_git(root, &["add", ".gitignore"]);
        run_git(root, &["commit", "--quiet", "-m", "init"]);
        run_git(root, &["checkout", "--quiet", "-B", branch]);
    }

    fn write_arch_rules(root: &std::path::Path) {
        std::fs::write(root.join("architecture-rules.json"), ARCH_RULES).unwrap();
    }

    fn write_custom_arch_rules(root: &std::path::Path) {
        std::fs::write(root.join("architecture-rules.json"), ARCH_RULES_CUSTOM_CATALOGUE).unwrap();
    }

    #[test]
    fn test_active_track_id_uses_explicit_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        git_repo_on_branch(dir.path(), "track/reader-track-2026-01-01");

        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());

        let track_id = adapter.active_track_id().unwrap();
        assert_eq!(track_id.as_ref(), "reader-track-2026-01-01");
    }

    #[test]
    fn test_active_track_id_non_track_branch_returns_unresolved() {
        let dir = tempfile::tempdir().unwrap();
        git_repo_on_branch(dir.path(), "main");

        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());

        let err = adapter.active_track_id().unwrap_err();
        assert!(matches!(err, SignalLayerReaderError::TrackIdUnresolved));
    }

    #[test]
    fn test_enabled_layers_reads_architecture_rules() {
        let dir = tempfile::tempdir().unwrap();
        write_arch_rules(dir.path());
        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());
        let track_id = TrackId::try_new("reader-track-2026-01-01").unwrap();

        let layers = adapter.enabled_layers(track_id).unwrap();
        let layer_names: Vec<&str> = layers.iter().map(AsRef::as_ref).collect();

        assert_eq!(layer_names, vec!["domain", "usecase"]);
    }

    #[test]
    fn test_enabled_layers_missing_architecture_rules_returns_io() {
        let dir = tempfile::tempdir().unwrap();
        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());
        let track_id = TrackId::try_new("reader-track-2026-01-01").unwrap();

        let err = adapter.enabled_layers(track_id).unwrap_err();

        assert!(matches!(err, SignalLayerReaderError::Io));
    }

    #[test]
    fn test_catalogue_bytes_existing_file_returns_bytes() {
        let dir = tempfile::tempdir().unwrap();
        write_arch_rules(dir.path());
        let track_id = TrackId::try_new("reader-track-2026-01-01").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let catalogue_dir = dir.path().join("track/items").join(track_id.as_ref());
        std::fs::create_dir_all(&catalogue_dir).unwrap();
        std::fs::write(catalogue_dir.join("domain-types.json"), b"{\"types\":[]}").unwrap();
        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());

        let bytes = adapter.catalogue_bytes(track_id, layer).unwrap();

        assert_eq!(bytes, Some(br#"{"types":[]}"#.to_vec()));
    }

    #[test]
    fn test_catalogue_bytes_uses_custom_catalogue_file() {
        let dir = tempfile::tempdir().unwrap();
        write_custom_arch_rules(dir.path());
        let track_id = TrackId::try_new("reader-track-2026-01-01").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let catalogue_dir = dir.path().join("track/items").join(track_id.as_ref());
        std::fs::create_dir_all(&catalogue_dir).unwrap();
        std::fs::write(catalogue_dir.join("custom-types.json"), b"{\"custom\":true}").unwrap();
        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());

        let bytes = adapter.catalogue_bytes(track_id, layer).unwrap();

        assert_eq!(bytes, Some(br#"{"custom":true}"#.to_vec()));
    }

    #[test]
    fn test_catalogue_bytes_missing_file_returns_none() {
        let dir = tempfile::tempdir().unwrap();
        write_arch_rules(dir.path());
        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());
        let track_id = TrackId::try_new("reader-track-2026-01-01").unwrap();
        let layer = LayerId::try_new("domain").unwrap();

        let bytes = adapter.catalogue_bytes(track_id, layer).unwrap();

        assert_eq!(bytes, None);
    }

    #[cfg(unix)]
    #[test]
    fn test_catalogue_bytes_symlink_returns_io() {
        let dir = tempfile::tempdir().unwrap();
        write_arch_rules(dir.path());
        let track_id = TrackId::try_new("reader-track-2026-01-01").unwrap();
        let layer = LayerId::try_new("domain").unwrap();
        let catalogue_dir = dir.path().join("track/items").join(track_id.as_ref());
        std::fs::create_dir_all(&catalogue_dir).unwrap();
        let outside = dir.path().join("outside.json");
        std::fs::write(&outside, b"{\"outside\":true}").unwrap();
        std::os::unix::fs::symlink(&outside, catalogue_dir.join("domain-types.json")).unwrap();
        let adapter = LocalSignalLayerReaderAdapter::new(dir.path().to_path_buf());

        let err = adapter.catalogue_bytes(track_id, layer).unwrap_err();

        assert!(matches!(err, SignalLayerReaderError::Io));
    }
}
