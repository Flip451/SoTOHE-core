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

/// Anchors every `tddd.enabled` catalogue path at `workspace_root` so the
/// resulting inputs work identically regardless of the process cwd (the
/// composition root discovers the git worktree, so we should never re-derive
/// the anchor from `PathBuf::from("track")`, which would silently follow cwd).
///
/// Catalogue filenames come from `architecture-rules.json` so architecture-
/// customizer renames stay coherent without hardcoding consumer layer ids.
pub(crate) fn default_catalogue_paths(workspace_root: &Path, track_id: &TrackId) -> Vec<PathBuf> {
    let dir = workspace_root.join("track").join("items").join(track_id.as_ref());
    catalogue_files_from_architecture_rules(workspace_root)
        .unwrap_or_else(|_| {
            // Tests and tightly-controlled fixtures may omit architecture-rules;
            // fall back to the SoTOHE-core template layer set.
            vec![
                "domain-types.json".to_owned(),
                "usecase-types.json".to_owned(),
                "infrastructure-types.json".to_owned(),
                "cli_driver-types.json".to_owned(),
                "cli_composition-types.json".to_owned(),
                "cli-types.json".to_owned(),
            ]
        })
        .into_iter()
        .map(|name| dir.join(name))
        .collect()
}

fn catalogue_files_from_architecture_rules(workspace_root: &Path) -> Result<Vec<String>, String> {
    let rules_path = workspace_root.join("architecture-rules.json");
    let raw = std::fs::read_to_string(&rules_path)
        .map_err(|e| format!("failed to read {}: {e}", rules_path.display()))?;
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| format!("failed to parse {}: {e}", rules_path.display()))?;
    let layers = value
        .get("layers")
        .and_then(|v| v.as_array())
        .ok_or_else(|| format!("{} missing layers array", rules_path.display()))?;
    let mut files = Vec::new();
    for layer in layers {
        let tddd = layer.get("tddd");
        let enabled = tddd
            .and_then(|t| t.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !enabled {
            continue;
        }
        let Some(file) = tddd.and_then(|t| t.get("catalogue_file")).and_then(|v| v.as_str()) else {
            continue;
        };
        files.push(file.to_owned());
    }
    if files.is_empty() {
        return Err(format!(
            "no tddd.enabled catalogue_file entries in {}",
            rules_path.display()
        ));
    }
    Ok(files)
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
    fn test_default_catalogue_paths_reads_architecture_rules_when_present() {
        let tmp = std::env::temp_dir().join(format!(
            "sotp-test-obligation-arch-rules-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let rules = tmp.join("architecture-rules.json");
        std::fs::write(
            &rules,
            r#"{"version":2,"layers":[
              {"crate":"entities","path":"libs/entities","tddd":{"enabled":true,"catalogue_file":"entities-types.json"}},
              {"crate":"web","path":"apps/web","tddd":{"enabled":true,"catalogue_file":"web-types.json"}},
              {"crate":"skip","path":"libs/skip","tddd":{"enabled":false,"catalogue_file":"skip-types.json"}}
            ]}"#,
        )
        .unwrap();
        let track_id = TrackId::try_new("example").unwrap();
        let paths = default_catalogue_paths(&tmp, &track_id);
        let _ = std::fs::remove_dir_all(&tmp);
        assert_eq!(paths.len(), 2);
        assert!(paths.iter().any(|p| p.ends_with("entities-types.json")));
        assert!(paths.iter().any(|p| p.ends_with("web-types.json")));
        assert!(paths.iter().all(|p| p.starts_with(tmp.join("track/items/example"))));
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
