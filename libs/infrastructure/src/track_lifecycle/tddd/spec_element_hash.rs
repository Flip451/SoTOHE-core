//! System adapter for the Track TDDD spec-element-hash port.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use domain::{ContentHash, SpecElementId, TrackId};
use usecase::track_lifecycle::TrackSpecAnchorSelection;
use usecase::track_lifecycle::tddd::spec_element_hash::{
    TrackSpecElementHashCommand, TrackSpecElementHashError, TrackSpecElementHashPort,
    TrackSpecElementHashResult,
};

/// System-backed adapter for reading canonical spec-element hashes.
pub struct SystemTrackSpecElementHashAdapter;

impl TrackSpecElementHashPort for SystemTrackSpecElementHashAdapter {
    fn execute(
        &self,
        track_id: TrackId,
        command: TrackSpecElementHashCommand,
    ) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError> {
        let items_dir = command.items_dir.as_path().to_path_buf();
        let workspace_root = workspace_root_for_items(&items_dir)
            .map_err(|error| execution_failed(format!("spec-element-hash failed: {error}")))?;
        validate_items_dir_within_workspace(&items_dir, &workspace_root)
            .map_err(|error| execution_failed(format!("spec-element-hash failed: {error}")))?;

        let anchor = match &command.anchor {
            TrackSpecAnchorSelection::All => None,
            TrackSpecAnchorSelection::One(anchor) => Some(anchor.as_ref()),
        };
        // Keep the caller-supplied path. The containment check above uses canonical paths only
        // for trust validation; the original path must reach the loader so its symlink guard
        // still rejects a symlinked `track/items` root.
        let hashes = crate::track::spec_element_hash::compute_spec_element_hashes(
            items_dir,
            track_id.as_ref(),
            anchor,
        )
        .map_err(|error| execution_failed(format!("spec-element-hash failed: {}", error.0)))?;

        match command.anchor {
            TrackSpecAnchorSelection::All => convert_all_hashes(hashes),
            TrackSpecAnchorSelection::One(anchor) => {
                let (_, hash) = hashes.into_iter().next().ok_or_else(|| {
                    execution_failed(format!("anchor '{anchor}' not found in spec.json"))
                })?;
                let hash = ContentHash::try_from_hex(hash)
                    .map_err(|error| execution_failed(format!("invalid content hash: {error}")))?;
                Ok(TrackSpecElementHashResult::Single(hash))
            }
        }
    }
}

fn convert_all_hashes(
    hashes: BTreeMap<String, String>,
) -> Result<TrackSpecElementHashResult, TrackSpecElementHashError> {
    let mut typed_hashes = BTreeMap::new();
    for (anchor, hash) in hashes {
        let anchor = SpecElementId::try_new(anchor)
            .map_err(|error| execution_failed(format!("invalid spec anchor: {error}")))?;
        let hash = ContentHash::try_from_hex(hash)
            .map_err(|error| execution_failed(format!("invalid content hash: {error}")))?;
        typed_hashes.insert(anchor, hash);
    }
    Ok(TrackSpecElementHashResult::All(typed_hashes))
}

fn workspace_root_for_items(items_dir: &Path) -> Result<PathBuf, String> {
    let track_dir =
        items_dir.parent().ok_or_else(|| "track items directory has no track parent".to_owned())?;
    let root = track_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| "track items directory has no workspace root".to_owned())?;
    if root.as_os_str().is_empty() { Ok(PathBuf::from(".")) } else { Ok(root) }
}

fn validate_items_dir_within_workspace(
    items_dir: &Path,
    workspace_root: &Path,
) -> Result<(), String> {
    let canonical_workspace = workspace_root.canonicalize().map_err(|error| {
        format!("cannot resolve workspace root '{}': {error}", workspace_root.display())
    })?;
    let canonical_items = items_dir.canonicalize().map_err(|error| {
        format!("cannot resolve track items directory '{}': {error}", items_dir.display())
    })?;
    if !canonical_items.starts_with(&canonical_workspace) {
        return Err(format!(
            "track items directory '{}' resolves outside workspace root '{}'; only paths under the workspace are allowed",
            items_dir.display(),
            workspace_root.display()
        ));
    }
    Ok(())
}

fn execution_failed(message: impl Into<String>) -> TrackSpecElementHashError {
    TrackSpecElementHashError::ExecutionFailed(usecase::git_workflow::DiagnosticText::new(message))
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]
mod tests {
    use std::fs;

    use super::*;
    use usecase::track_lifecycle::{TrackItemsDirectory, TrackSelection};

    const VALID_SPEC_JSON: &str = r#"{
  "schema_version": 2,
  "version": "1.0",
  "title": "Test Spec",
  "goal": [
    {"id": "GL-01", "text": "First goal"}
  ],
  "scope": {
    "in_scope": [
      {"id": "IN-01", "text": "In scope item"}
    ],
    "out_of_scope": []
  }
}"#;

    fn setup(root: &Path) -> (TrackId, TrackSpecElementHashCommand) {
        let items_dir = root.join("track/items");
        let track_dir = items_dir.join("hash-track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        fs::write(track_dir.join("spec.json"), VALID_SPEC_JSON).expect("spec document is written");
        let track_id = TrackId::try_new("hash-track").expect("track id is valid");
        let command = TrackSpecElementHashCommand {
            track: TrackSelection::Explicit(track_id.clone()),
            items_dir: TrackItemsDirectory::try_new(items_dir).expect("items directory is valid"),
            anchor: TrackSpecAnchorSelection::All,
        };
        (track_id, command)
    }

    #[test]
    fn test_system_track_spec_element_hash_adapter_returns_typed_all_hashes() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let (track_id, command) = setup(workspace.path());

        let result = SystemTrackSpecElementHashAdapter
            .execute(track_id, command)
            .expect("all spec hashes succeed");

        let TrackSpecElementHashResult::All(hashes) = result else {
            panic!("all-anchor lookup must return the all result");
        };
        assert_eq!(hashes.len(), 2);
        assert!(hashes.keys().any(|anchor| anchor.as_ref() == "GL-01"));
    }

    #[test]
    fn test_system_track_spec_element_hash_adapter_returns_typed_single_hash() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let (track_id, mut command) = setup(workspace.path());
        command.anchor = TrackSpecAnchorSelection::One(
            SpecElementId::try_new("GL-01").expect("anchor is valid"),
        );

        let result = SystemTrackSpecElementHashAdapter
            .execute(track_id, command)
            .expect("single-anchor lookup succeeds");

        let TrackSpecElementHashResult::Single(hash) = result else {
            panic!("single-anchor lookup must return the single result");
        };
        assert_eq!(hash.to_hex().len(), 64);
    }

    #[cfg(unix)]
    #[test]
    fn test_system_track_spec_element_hash_adapter_preserves_original_path_for_symlink_guard() {
        let workspace = tempfile::tempdir().expect("temporary workspace exists");
        let real_items = workspace.path().join("real-items");
        let real_track = real_items.join("hash-track");
        fs::create_dir_all(&real_track).expect("real track directory exists");
        fs::write(real_track.join("spec.json"), VALID_SPEC_JSON).expect("spec document is written");
        let track_dir = workspace.path().join("track");
        fs::create_dir_all(&track_dir).expect("track directory exists");
        std::os::unix::fs::symlink(&real_items, track_dir.join("items"))
            .expect("items symlink exists");
        let items_dir = track_dir.join("items");
        let track_id = TrackId::try_new("hash-track").expect("track id is valid");
        let command = TrackSpecElementHashCommand {
            track: TrackSelection::Explicit(track_id.clone()),
            items_dir: TrackItemsDirectory::try_new(items_dir)
                .expect("symlinked items directory is valid input"),
            anchor: TrackSpecAnchorSelection::All,
        };

        let error = match SystemTrackSpecElementHashAdapter.execute(track_id, command) {
            Ok(_) => panic!("symlinked items directory must fail closed"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("symlink guard"));
    }
}
