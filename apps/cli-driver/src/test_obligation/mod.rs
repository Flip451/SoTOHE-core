//! Primary adapters for `sotp test-obligation`.

use std::path::{Path, PathBuf};

use usecase::test_obligation::TestObligationCatalogueCommandInput;
use usecase::{DiagnosticMessage, TrackId};

pub mod bindings_skeleton;
pub mod check;
pub mod derive;
pub mod evaluate;
pub mod results;

pub(crate) fn parse_input_parts(
    track_id: Option<String>,
    current_branch: String,
) -> Result<(Option<TrackId>, DiagnosticMessage), String> {
    let track_id = track_id
        .map(TrackId::try_new)
        .transpose()
        .map_err(|e| format!("invalid --track-id: {e}"))?;
    let current_branch = DiagnosticMessage::try_new(current_branch)
        .map_err(|e| format!("invalid current branch diagnostic: {e}"))?;
    Ok((track_id, current_branch))
}

pub(crate) fn resolve_track_id(
    explicit: Option<&TrackId>,
    current_branch: &DiagnosticMessage,
) -> Result<TrackId, String> {
    if let Some(track_id) = explicit {
        return Ok(track_id.clone());
    }
    let branch = current_branch.as_str();
    let Some(raw) = branch.strip_prefix("track/") else {
        return Err(format!(
            "--track-id is required when current branch is not track/<id>: {branch}"
        ));
    };
    TrackId::try_new(raw.to_owned()).map_err(|e| format!("invalid track id from branch: {e}"))
}

/// Anchors the six TDDD-layer catalogue paths at `workspace_root` so the
/// resulting inputs work identically regardless of the process cwd (the
/// composition root discovers the git worktree, so we should never re-derive
/// the anchor from `PathBuf::from("track")`, which would silently follow cwd).
pub(crate) fn default_catalogue_paths(workspace_root: &Path, track_id: &TrackId) -> Vec<PathBuf> {
    let dir = workspace_root.join("track").join("items").join(track_id.as_ref());
    [
        "domain-types.json",
        "usecase-types.json",
        "infrastructure-types.json",
        "cli_driver-types.json",
        "cli_composition-types.json",
        "cli-types.json",
    ]
    .into_iter()
    .map(|name| dir.join(name))
    .collect()
}

pub(crate) fn catalogue_command_input(
    workspace_root: &Path,
    explicit_track_id: Option<&TrackId>,
    current_branch: &DiagnosticMessage,
) -> Result<TestObligationCatalogueCommandInput, String> {
    let track_id = resolve_track_id(explicit_track_id, current_branch)?;
    Ok(TestObligationCatalogueCommandInput::new(
        track_id.clone(),
        current_branch.as_str().to_owned(),
        default_catalogue_paths(workspace_root, &track_id),
    ))
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    fn branch(text: &str) -> DiagnosticMessage {
        DiagnosticMessage::try_new(text.to_owned()).unwrap()
    }

    #[test]
    fn test_resolve_track_id_with_explicit_value_returns_it() {
        let track_id = TrackId::try_new("explicit-track").unwrap();
        let resolved = resolve_track_id(Some(&track_id), &branch("main")).unwrap();
        assert_eq!(resolved, track_id);
    }

    #[test]
    fn test_resolve_track_id_without_explicit_value_reads_track_branch() {
        let resolved = resolve_track_id(None, &branch("track/example-2026-07-09")).unwrap();
        assert_eq!(resolved.as_ref(), "example-2026-07-09");
    }

    #[test]
    fn test_default_catalogue_paths_returns_all_tddd_layer_catalogues() {
        let track_id = TrackId::try_new("example").unwrap();
        let workspace_root = PathBuf::from("/repo");
        let paths = default_catalogue_paths(&workspace_root, &track_id);
        assert_eq!(paths.len(), 6);
        assert!(paths.iter().any(|p| p.ends_with("domain-types.json")));
        assert!(paths.iter().any(|p| p.ends_with("cli-types.json")));
    }

    #[test]
    fn test_default_catalogue_paths_anchors_at_workspace_root() {
        // Regression: paths must be rooted at the discovered workspace, not
        // at whatever cwd happens to be when `sotp test-obligation` runs.
        let track_id = TrackId::try_new("example").unwrap();
        let workspace_root = PathBuf::from("/discovered/workspace");
        let paths = default_catalogue_paths(&workspace_root, &track_id);
        let expected_prefix = PathBuf::from("/discovered/workspace/track/items/example/");
        for path in &paths {
            assert!(
                path.starts_with(&expected_prefix),
                "expected {:?} to be anchored under {:?}",
                path,
                expected_prefix,
            );
        }
    }

    #[test]
    fn test_catalogue_command_input_resolves_track_from_branch() {
        // Cross-check that `catalogue_command_input` threads the workspace
        // anchor through to `default_catalogue_paths` and resolves the track
        // id from the branch. `catalogue_paths()` is crate-private on the
        // usecase input, so the anchoring assertion lives in
        // `test_default_catalogue_paths_anchors_at_workspace_root`; here we
        // just verify the plumbing does not error.
        let workspace_root = PathBuf::from("/discovered/workspace");
        let branch = branch("track/example");
        let result = catalogue_command_input(&workspace_root, None, &branch);
        assert!(result.is_ok(), "expected Ok, got {result:?}");
    }
}
