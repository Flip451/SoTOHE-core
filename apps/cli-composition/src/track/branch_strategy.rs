//! Branch strategy and switch-base regressions for the `track` command family.

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use std::path::Path;
    use std::process::Command;

    use cli_driver::track::TrackInput;

    fn write_branch_strategy_config(root: &Path) {
        let config_dir = root.join(".harness").join("config");
        std::fs::create_dir_all(&config_dir).unwrap();
        std::fs::write(
            config_dir.join("branch-strategy.json"),
            r#"{
  "base_branch": "main",
  "merge_target": "main",
  "merge_method": "merge"
}"#,
        )
        .unwrap();
    }

    fn write_track_metadata(root: &Path, track_id: &str, base_branch: &str) {
        let track_dir = root.join("track").join("items").join(track_id);
        std::fs::create_dir_all(&track_dir).unwrap();
        std::fs::write(
            track_dir.join("metadata.json"),
            format!(
                r#"{{
  "schema_version": 6,
  "id": "{track_id}",
  "branch": null,
  "title": "Switch Base Track",
  "created_at": "2026-03-13T00:00:00Z",
  "updated_at": "2026-03-13T00:00:00Z",
  "branch_strategy_snapshot": {{
    "base_branch": "{base_branch}",
    "merge_target": "main",
    "merge_method": "merge"
  }}
}}"#
            ),
        )
        .unwrap();
    }

    #[test]
    fn test_track_switch_base_call_site_preserves_cli_contract_across_migration() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "track/active-track");
        write_branch_strategy_config(root.path());
        write_track_metadata(root.path(), "active-track", "main");
        let create_main =
            Command::new("git").args(["branch", "main"]).current_dir(root.path()).status().unwrap();
        assert!(create_main.success(), "fixture must have a main branch");

        let argv_project_root = root.path().to_path_buf();
        let outcome = crate::TrackCompositionRoot::new()
            .track_driver()
            .handle(TrackInput::SwitchBase { project_root: argv_project_root.clone() });

        assert_eq!(outcome.exit_code, 0, "stderr={:?}", outcome.stderr);
        assert_eq!(argv_project_root, root.path());
        let stdout = outcome.stdout.as_deref().unwrap_or("");
        assert!(stdout.contains("Switching to main..."), "stdout={stdout}");
        assert!(
            stdout.contains("[WARN] Pull failed (may not have remote tracking branch)")
                || stdout.contains("[OK] On main, up to date."),
            "stdout={stdout}"
        );
        assert_eq!(outcome.stderr, None);

        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert_eq!(String::from_utf8(branch.stdout).unwrap().trim(), "main");
    }

    #[test]
    fn test_track_switch_base_checkout_failure_preserves_legacy_cli_contract() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "track/active-track");
        write_track_metadata(root.path(), "active-track", "missing-base");

        let argv_project_root = root.path().to_path_buf();
        let outcome = crate::TrackCompositionRoot::new()
            .track_driver()
            .handle(TrackInput::SwitchBase { project_root: argv_project_root.clone() });

        assert_eq!(argv_project_root, root.path());
        assert_eq!(outcome.stdout.as_deref(), Some("Failed to checkout missing-base"));
        assert_eq!(outcome.stderr, None);
        assert_ne!(outcome.exit_code, 0);
        assert!(
            !outcome.stdout.as_deref().unwrap_or("").contains("[ERROR]"),
            "composition must not template driver presentation"
        );
    }

    #[test]
    fn test_track_branch_create_date_prefixed_ids_create_sorted_track_branches() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "main");
        write_branch_strategy_config(root.path());
        let items_dir = root.path().join("track").join("items");
        let composition = crate::TrackCompositionRoot::new();

        for track_id in ["2026-07-31-later-track", "2026-07-01-earlier-track"] {
            let outcome = composition.track_driver().handle(TrackInput::BranchCreate {
                items_dir: items_dir.clone(),
                track_id: track_id.to_owned(),
            });
            assert_eq!(outcome.exit_code, 0);

            let status = Command::new("git")
                .args(["switch", "main"])
                .current_dir(root.path())
                .status()
                .unwrap();
            assert!(status.success(), "must return fixture to the configured base branch");
        }

        let output = Command::new("git")
            .args(["branch", "--list", "--sort=refname", "--format=%(refname:short)", "track/*"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(output.status.success());
        let listed_branches = String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .map(str::to_owned)
            .collect::<Vec<_>>();

        assert_eq!(
            listed_branches,
            ["track/2026-07-01-earlier-track", "track/2026-07-31-later-track",],
            "ascending branch listing must put the earlier date first"
        );
    }

    #[test]
    fn test_track_branch_create_suffix_form_id_preserves_branch_name() {
        let root = tempfile::tempdir().unwrap();
        crate::test_support::seed_repo(root.path(), "main");
        write_branch_strategy_config(root.path());
        let items_dir = root.path().join("track").join("items");
        let track_id = "legacy-suffix-track-2026-07-31";

        let outcome = crate::TrackCompositionRoot::new()
            .track_driver()
            .handle(TrackInput::BranchCreate { items_dir, track_id: track_id.to_owned() });

        assert_eq!(outcome.exit_code, 0);
        let branch = Command::new("git")
            .args(["branch", "--show-current"])
            .current_dir(root.path())
            .output()
            .unwrap();
        assert!(branch.status.success());
        assert_eq!(
            String::from_utf8(branch.stdout).unwrap().trim(),
            format!("track/{track_id}"),
            "a suffix-form ID must remain unchanged in its track branch"
        );
    }
}
