//! Track TDDD composition-root regressions.

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    clippy::expect_used,
    clippy::missing_panics_doc
)]
mod tests {
    use std::path::PathBuf;

    use crate::error::CompositionError;

    use cli_driver::track_tddd::{
        TrackItemsDirectoryInput, TrackTdddCatalogueSpecSignalsInput, TrackTdddInput,
        TrackWorkspaceRootInput,
    };

    use crate::track::composition_root::TrackCompositionRoot;

    fn parse_layer_filter_ids(raw: &str) -> Result<Vec<usecase::LayerId>, CompositionError> {
        let mut layers = Vec::new();
        for token in raw.split(',') {
            let trimmed = token.trim();
            if trimmed.is_empty() {
                continue;
            }
            let id = usecase::LayerId::try_new(trimmed).map_err(|e| {
                CompositionError::WiringFailed(format!("invalid layer id '{trimmed}': {e}"))
            })?;
            layers.push(id);
        }
        Ok(layers)
    }

    fn catalogue_spec_signals_outcome(
        items_dir: PathBuf,
        track_id: &str,
        workspace_root: PathBuf,
    ) -> cli_driver::CommandOutcome {
        let track_id = track_id.parse().unwrap();
        let items_dir = TrackItemsDirectoryInput::try_new(items_dir).unwrap();
        let workspace_root = TrackWorkspaceRootInput::try_from(workspace_root).unwrap();
        TrackCompositionRoot::new().track_tddd_driver().handle(
            TrackTdddInput::CatalogueSpecSignals(TrackTdddCatalogueSpecSignalsInput {
                track_id: Some(track_id),
                items_dir,
                workspace_root,
                layer: None,
            }),
        )
    }

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

    #[test]
    fn test_parse_layer_filter_ids_invalid_value_returns_error() {
        // A value with an internal space is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("domain core");
        assert!(result.is_err(), "layer id with space must be rejected");

        // A value starting with a digit is rejected by LayerId::try_new (CN-12).
        let result = parse_layer_filter_ids("1layer");
        assert!(result.is_err(), "layer id starting with digit must be rejected");
    }

    // ── T003: catalogue-spec-signals absent-catalogue skip path ─────────────

    /// Helper: create a minimal git repo on `track/<track_id>` branch.
    fn init_git_repo_on_track_branch(root: &std::path::Path, track_id: &str) {
        let branch_name = format!("track/{track_id}");
        let run_git = |args: &[&str]| {
            let status = std::process::Command::new("git")
                .args(args)
                .current_dir(root)
                .status()
                .expect("git command failed to spawn");
            assert!(status.success(), "git {} exited with status {status}", args.join(" "));
        };
        run_git(&["init", "-q"]);
        run_git(&["config", "user.email", "test@example.com"]);
        run_git(&["config", "user.name", "Test"]);
        run_git(&["config", "commit.gpgsign", "false"]);
        run_git(&["commit", "--allow-empty", "-q", "-m", "init", "--no-gpg-sign"]);
        run_git(&["branch", "-m", &branch_name]);
    }

    fn minimal_active_metadata_json(track_id: &str) -> String {
        format!(
            r#"{{
  "schema_version": 5,
  "id": "{track_id}",
  "branch": "track/{track_id}",
  "title": "Test Track",
  "created_at": "2026-04-15T00:00:00Z",
  "updated_at": "2026-04-15T00:00:00Z"
}}
"#
        )
    }

    fn minimal_impl_plan_json() -> &'static str {
        r#"{"schema_version":1,"tasks":[],"plan":{"summary":[],"sections":[]}}"#
    }

    fn setup_catalogue_spec_signal_track() -> (tempfile::TempDir, PathBuf, PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_on_track_branch(dir.path(), "test-track");

        let items_dir = dir.path().join("track/items");
        let track_dir = items_dir.join("test-track");
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(track_dir.join("metadata.json"), minimal_active_metadata_json("test-track"))
            .unwrap();
        std::fs::write(track_dir.join("impl-plan.json"), minimal_impl_plan_json()).unwrap();

        let rules_json = r#"{
          "version": 2,
          "layers": [{
            "crate": "domain",
            "tddd": {
              "enabled": true,
              "catalogue_file": "domain-types.json",
              "catalogue_spec_signal": { "enabled": true }
            }
          }]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        (dir, items_dir, track_dir)
    }

    /// AC-01/AC-02: `catalogue-spec-signals` gate at Phase 0 (no catalogue file) succeeds.
    ///
    /// The gate (`track-active-gate`) calls `sotp signal calc-catalog-spec` after
    /// `sotp signal calc-impl-catalog`. When no catalogue exists, both commands
    /// must exit zero so the full gate chain succeeds at Phase 0/1.
    #[test]
    fn test_track_catalogue_spec_signals_absent_catalogue_returns_ok() {
        let (dir, items_dir, _track_dir) = setup_catalogue_spec_signal_track();

        let outcome =
            catalogue_spec_signals_outcome(items_dir, "test-track", dir.path().to_path_buf());
        assert_eq!(
            outcome.exit_code, 0,
            "absent catalogue in catalogue-spec-signals must return Ok (Phase 0 skip), \
             got: {outcome:?}"
        );
    }

    #[test]
    fn test_track_catalogue_spec_signals_missing_track_dir_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        init_git_repo_on_track_branch(dir.path(), "test-track");

        let items_dir = dir.path().join("track/items");
        std::fs::create_dir_all(&items_dir).unwrap();

        let rules_json = r#"{
          "version": 2,
          "layers": [{
            "crate": "domain",
            "tddd": {
              "enabled": true,
              "catalogue_file": "domain-types.json",
              "catalogue_spec_signal": { "enabled": true }
            }
          }]
        }"#;
        std::fs::write(dir.path().join("architecture-rules.json"), rules_json).unwrap();

        let outcome =
            catalogue_spec_signals_outcome(items_dir, "test-track", dir.path().to_path_buf());

        assert_ne!(
            outcome.exit_code, 0,
            "missing track directory must not be hidden by absent-catalogue leniency"
        );
    }

    /// CN-02/AC-03: `catalogue-spec-signals` with a PRESENT catalogue does NOT silently
    /// skip — it evaluates normally. The absent-catalogue skip must only apply when the
    /// file is genuinely absent (no fail-open on present catalogues).
    #[test]
    fn test_track_catalogue_spec_signals_present_catalogue_is_evaluated_not_skipped() {
        let (dir, items_dir, track_dir) = setup_catalogue_spec_signal_track();

        // Write a minimal v5 catalogue with a Red-signal entry.
        let v5_catalogue = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "RedType": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": { "kind": "struct", "shape": { "kind": "unit" } },
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;
        std::fs::write(track_dir.join("domain-types.json"), v5_catalogue).unwrap();

        let outcome =
            catalogue_spec_signals_outcome(items_dir, "test-track", dir.path().to_path_buf());

        // A present catalogue with a red signal must still be evaluated (not silently
        // skipped). The catalogue-spec-signals refresher writes the signals file — it
        // does NOT block on red signals itself (blocking is the gate's job). The
        // command must succeed because signal computation is a regen, not a gate.
        // This test confirms the absent-catalogue skip does NOT fire when catalogue IS present.
        assert_eq!(
            outcome.exit_code, 0,
            "present catalogue must be evaluated (not skipped): {outcome:?}"
        );

        // Verify the signal file was written (catalogue was processed, not skipped).
        let signals_path = track_dir.join("domain-catalogue-spec-signals.json");
        assert!(
            signals_path.exists(),
            "signals file must be written when catalogue IS present (not silently skipped)"
        );
    }

    /// T003: stale signals file is removed when catalogue is absent.
    ///
    /// If a catalogue was removed/renamed but a previously-generated
    /// `<layer>-catalogue-spec-signals.json` is still present, the
    /// absent-catalogue arm must delete it so that the later
    /// `signal check-catalog-spec` does not find signals without
    /// a backing catalogue (which would be an error).
    #[test]
    fn test_track_catalogue_spec_signals_absent_catalogue_removes_stale_signals_file() {
        let (dir, items_dir, track_dir) = setup_catalogue_spec_signal_track();

        // Write a stale signals file (catalogue was removed but signals remained).
        let stale_signals_path = track_dir.join("domain-catalogue-spec-signals.json");
        std::fs::write(&stale_signals_path, r#"{"stale": true}"#).unwrap();
        assert!(stale_signals_path.exists(), "pre-condition: stale signals file must exist");

        let outcome =
            catalogue_spec_signals_outcome(items_dir, "test-track", dir.path().to_path_buf());

        assert_eq!(
            outcome.exit_code, 0,
            "absent catalogue must return Ok even with a stale signals file, got: {outcome:?}"
        );
        assert!(
            !stale_signals_path.exists(),
            "stale signals file must be removed when catalogue is absent"
        );
    }
}
