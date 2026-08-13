use std::process::ExitCode;

use cli_composition::TrackCompositionRoot;
use cli_driver::track::TrackInput;

use crate::CliError;

use super::state_ops::track_driver_outcome_to_result;
use super::{ResolveArgs, resolve_project_root, resolve_track_id, validate_track_id_str};

pub(super) fn execute_resolve(args: ResolveArgs) -> Result<ExitCode, CliError> {
    let ResolveArgs { items_dir, track_id } = args;

    // Validate items_dir structure (must be <root>/track/items) unconditionally,
    // even when track_id is explicitly provided (resolve_track_id only calls
    // resolve_project_root when explicit_id is None).
    resolve_project_root(&items_dir).map_err(|e| CliError::Message(e.to_string()))?;

    // Delegate to resolve_track_id which anchors git discovery to the repository
    // owning items_dir (via resolve_project_root). Explicit id short-circuits git
    // discovery (CN-02 / AC-19).
    let effective_track_id = resolve_track_id(track_id, &items_dir)
        .map_err(|err| CliError::Message(format!("resolve failed: {err}")))?;

    // Validate the track ID before any filesystem probing.
    // `items_dir.join(track_id)` would otherwise let a caller traverse outside
    // `track/items` with values like `..` or absolute paths.
    validate_track_id_str(&effective_track_id)
        .map_err(|err| CliError::Message(format!("resolve failed: invalid track id: {err}")))?;

    let outcome = TrackCompositionRoot::new()
        .track_driver()
        .handle(TrackInput::Resolve { items_dir, track_id: Some(effective_track_id) });
    track_driver_outcome_to_result(outcome)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::fs;
    use std::process::ExitCode;

    use cli_composition::TrackCompositionRoot;
    use cli_driver::track::TrackInput;

    use super::*;

    fn write_track_fixture(track_dir: &std::path::Path) {
        fs::write(
            track_dir.join("metadata.json"),
            r#"{
  "schema_version": 6,
  "id": "resolve-regression",
  "branch": null,
  "title": "Resolve Regression",
  "created_at": "2026-01-01T00:00:00Z",
  "updated_at": "2026-01-01T00:00:00Z",
  "branch_strategy_snapshot": {
    "base_branch": "main",
    "merge_target": "main",
    "merge_method": "squash"
  }
}"#,
        )
        .unwrap();
        fs::write(
            track_dir.join("impl-plan.json"),
            r#"{
  "schema_version": 1,
  "tasks": [
    {"id": "T001", "description": "Existing work", "status": "in_progress"}
  ],
  "plan": {
    "summary": [],
    "sections": [
      {"id": "S1", "title": "Existing", "description": [], "task_ids": ["T001"]}
    ]
  }
}"#,
        )
        .unwrap();
    }

    #[test]
    fn test_track_resolve_in_progress_output_has_no_signal_report_occurrence_summary() {
        let tmp = tempfile::tempdir().unwrap();
        let items_dir = tmp.path().join("track/items");
        let track_dir = items_dir.join("resolve-regression");
        fs::create_dir_all(&track_dir).unwrap();
        write_track_fixture(&track_dir);

        let outcome = TrackCompositionRoot::new().track_driver().handle(TrackInput::Resolve {
            items_dir: items_dir.clone(),
            track_id: Some("resolve-regression".to_owned()),
        });

        assert_eq!(outcome.exit_code, 0);
        assert_eq!(
            outcome.stdout.as_deref(),
            Some(
                "Current phase: In Progress\nReason: track has unresolved tasks\nRecommended next command: /track:implement"
            )
        );
        assert!(
            !outcome.stdout.as_deref().unwrap_or_default().contains("signal report"),
            "track resolve must not add a signal-report occurrence summary"
        );
        assert_eq!(
            execute_resolve(ResolveArgs {
                items_dir,
                track_id: Some("resolve-regression".to_owned()),
            })
            .unwrap(),
            ExitCode::SUCCESS
        );
    }
}
