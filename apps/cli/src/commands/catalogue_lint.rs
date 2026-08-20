//! `catalogue-lint` subcommands for the `sotp` CLI.
//!
//! Provides:
//! - `check-active-track`: run the default-config catalogue lint ruleset across
//!   every `tddd.enabled` layer of the active track and exit non-zero if any
//!   layer reports a violation. Reuses the same active-track-resolution
//!   mechanism already used by `sotp track lint` / `sotp signal
//!   calc-impl-catalog` (CN-07: no new track-scoping logic).
//!
//! ADR `knowledge/adr/2026-07-01-0004-catalogue-primitive-obsession-guard.md`
//! §D5: blocking from day one, no warn→block staged migration — this command
//! is wired into `track-active-gate` (see `Makefile.toml`) so it runs on every
//! commit/review cycle from the moment this track lands.
//!
//! All composition (adapter construction, interactor wiring, layer
//! enumeration) lives in `cli_composition`; this module is a thin
//! arg-parsing layer that hands off to the `track` primary-adapter driver
//! (`cli_driver::track::TrackDriver`) — it never calls `cli_composition`
//! workflow methods directly (CN-01 / CN-02).

use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Subcommand};
use cli_composition::TrackCompositionRoot;
use cli_driver::adr_baseline::TrackIdInput;
use cli_driver::track_tddd::{
    TrackLintRulesFileInput, TrackTdddCatalogueLintActiveInput, TrackTdddInput,
    TrackWorkspaceRootInput,
};

// ── sotp catalogue-lint ─────────────────────────────────────────────────────

/// Subcommands for `sotp catalogue-lint`.
#[derive(Debug, Clone, Subcommand)]
pub enum CatalogueLintCommand {
    /// Run the catalogue lint ruleset across every `tddd.enabled` layer of the
    /// active track and exit non-zero if any layer reports a violation.
    ///
    /// Active-track resolution: when `--track-id` is omitted, the track is
    /// auto-resolved from the current git branch (`track/<id>`), the same
    /// mechanism `sotp track lint` already uses. Non-track branches fail
    /// closed (non-zero exit), matching the existing convention.
    ///
    /// Layers whose catalogue file does not exist yet (track has not
    /// finished Phase 2 `type-design` for that layer) cause the whole gate
    /// to be skipped for this run (exit 0) rather than erroring, since
    /// `CatalogueLoader::load_all` requires every `tddd.enabled` layer's
    /// catalogue file to be present.
    ///
    /// Exits 0 when zero violations are found (or the gate is skipped);
    /// exits 1 when one or more violations are found, or when the lint
    /// config is missing.
    CheckActiveTrack(CatalogueLintCheckActiveTrackArgs),
}

// ── sotp catalogue-lint check-active-track ──────────────────────────────────

/// Arguments for `sotp catalogue-lint check-active-track`.
///
/// `track_id` is optional; when omitted, the active track is auto-resolved
/// from the current git branch (`track/<id>`), matching the convention of
/// `sotp track lint` and other track-aware commands. `workspace_root`
/// defaults to `.` (the current directory). `rules_file` optionally
/// overrides the default lint config location
/// (`.harness/catalogue-lint/config.json`).
#[derive(Debug, Clone, Args)]
pub struct CatalogueLintCheckActiveTrackArgs {
    /// Active track identifier. When omitted, auto-resolved from the current
    /// git branch (only `track/<id>` branches are accepted).
    #[arg(long)]
    pub track_id: Option<String>,

    /// Workspace root directory (contains `architecture-rules.json` and
    /// `track/items/`).
    #[arg(long, default_value = ".")]
    pub workspace_root: PathBuf,

    /// Optional override for the lint config file path (defaults to
    /// `.harness/catalogue-lint/config.json` under `workspace_root`).
    #[arg(long)]
    pub rules_file: Option<PathBuf>,
}

// ── Dispatch ─────────────────────────────────────────────────────────────────

/// Dispatch `sotp catalogue-lint <subcommand>` to the appropriate execute_* handler.
pub fn execute(cmd: CatalogueLintCommand) -> ExitCode {
    match cmd {
        CatalogueLintCommand::CheckActiveTrack(args) => execute_check_active_track(args),
    }
}

/// Execute `sotp catalogue-lint check-active-track`.
///
/// Routes through the Track TDDD primary adapter and its typed input DTO. The
/// existing public subcommand remains unchanged; only its internal execution
/// boundary is migrated.
pub fn execute_check_active_track(args: CatalogueLintCheckActiveTrackArgs) -> ExitCode {
    let track_id = match args.track_id.map(|value| value.parse::<TrackIdInput>()).transpose() {
        Ok(track_id) => track_id,
        Err(error) => {
            eprintln!("invalid track id: {error}");
            return ExitCode::FAILURE;
        }
    };
    let workspace_root = match TrackWorkspaceRootInput::try_from(args.workspace_root) {
        Ok(workspace_root) => workspace_root,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let rules_file = match args.rules_file.map(TrackLintRulesFileInput::try_new).transpose() {
        Ok(rules_file) => rules_file,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    let outcome = TrackCompositionRoot::new().track_tddd_driver().handle(
        TrackTdddInput::CatalogueLintActive(TrackTdddCatalogueLintActiveInput {
            track_id,
            workspace_root,
            rules_file,
        }),
    );
    let mut stdout = std::io::stdout();
    let mut stderr = std::io::stderr();
    match crate::commands::track::tddd::emit_driver_outcome!(outcome, &mut stdout, &mut stderr) {
        Ok(code) => code,
        Err(error) => {
            eprintln!("{error}");
            error.exit_code()
        }
    }
}

// ── Tests ────────────────────────────────────────────────────────────────────

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use clap::Parser;
    use cli_driver::CommandOutcome;

    use super::*;

    /// Thin clap wrapper for parsing `sotp catalogue-lint <subcmd>` in tests.
    #[derive(Parser)]
    struct TestCli {
        #[command(subcommand)]
        cmd: CatalogueLintCommand,
    }

    fn parse_catalogue_lint(args: &[&str]) -> CatalogueLintCommand {
        TestCli::parse_from(args).cmd
    }

    #[test]
    fn test_check_active_track_parses_track_id_arg() {
        let cmd = parse_catalogue_lint(&[
            "catalogue-lint",
            "check-active-track",
            "--track-id",
            "my-track",
        ]);
        match cmd {
            CatalogueLintCommand::CheckActiveTrack(args) => {
                assert_eq!(args.track_id, Some("my-track".to_owned()));
                assert_eq!(args.workspace_root, PathBuf::from("."));
                assert_eq!(args.rules_file, None);
            }
        }
    }

    #[test]
    fn test_check_active_track_omitting_track_id_is_accepted() {
        // --track-id is optional; omitting it triggers auto-resolution from
        // the current git branch (`track/<id>`) at runtime. Clap-level parse
        // must accept this; resolution itself is exercised by integration tests.
        let result = TestCli::try_parse_from(["catalogue-lint", "check-active-track"]);
        assert!(result.is_ok(), "--track-id is optional; omitting it should be accepted");
        match result.unwrap().cmd {
            CatalogueLintCommand::CheckActiveTrack(args) => {
                assert_eq!(args.track_id, None, "omitting --track-id must yield None");
            }
        }
    }

    #[test]
    fn test_check_active_track_parses_custom_workspace_root_and_rules_file() {
        let cmd = parse_catalogue_lint(&[
            "catalogue-lint",
            "check-active-track",
            "--workspace-root",
            "custom/root",
            "--rules-file",
            "custom/rules.json",
        ]);
        match cmd {
            CatalogueLintCommand::CheckActiveTrack(args) => {
                assert_eq!(args.workspace_root, PathBuf::from("custom/root"));
                assert_eq!(args.rules_file, Some(PathBuf::from("custom/rules.json")));
            }
        }
    }

    #[test]
    fn test_catalogue_lint_unknown_subcommand_is_rejected() {
        let result = TestCli::try_parse_from(["catalogue-lint", "unknown-subcmd"]);
        assert!(result.is_err(), "unrecognized catalogue-lint subcommand must be rejected by clap");
    }

    #[test]
    fn test_check_active_track_missing_config_exits_one_without_stderr_contract_change() {
        let workspace = tempfile::tempdir().expect("temp workspace");
        let code = execute_check_active_track(CatalogueLintCheckActiveTrackArgs {
            track_id: Some("missing-track".to_owned()),
            workspace_root: workspace.path().to_path_buf(),
            rules_file: None,
        });
        assert_eq!(code, ExitCode::from(1), "missing lint config must keep exit 1");
    }

    const RULES_JSON: &str = r#"{
  "version": 2,
  "layers": [
    {
      "crate": "domain",
      "path": "libs/domain",
      "may_depend_on": [],
      "deny_reason": "no reverse dep",
      "tddd": {
        "enabled": true,
        "catalogue_file": "domain-types.json",
        "schema_export": {"method": "rustdoc", "targets": ["domain"]}
      }
    }
  ]
}"#;

    const LINT_CONFIG_WITH_INVARIANT_RULE: &str = r#"{
  "schema_version": 1,
  "rules": [
    {
      "target_roles": ["ValueObject"],
      "kind": { "FieldNonEmpty": { "target_field": "invariants" } }
    }
  ]
}"#;

    const CATALOGUE_WITH_INVARIANT: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "MyValueObject": {
      "action": "add",
      "role": {
        "ValueObject": {
          "invariants": [
            { "name": "is_valid", "predicate": { "SelfMethod": "is_valid" } }
          ]
        }
      },
      "kind": {"kind": "struct", "shape": {"kind": "plain"}},
      "methods": [
        {"name": "is_valid", "receiver": "&self", "params": [], "returns": "bool"}
      ],
      "module_path": "",
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

    const CATALOGUE_NO_INVARIANTS: &str = r#"{
  "schema_version": 5,
  "crate_name": "domain",
  "layer": "domain",
  "types": {
    "BareValueObject": {
      "action": "add",
      "role": { "ValueObject": {} },
      "kind": {"kind": "struct", "shape": {"kind": "plain"}},
      "methods": [],
      "module_path": "",
      "spec_refs": [],
      "informal_grounds": []
    }
  },
  "traits": {},
  "functions": {}
}"#;

    fn write_file(path: &std::path::Path, content: &str) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create fixture parent");
        }
        std::fs::write(path, content).expect("write fixture");
    }

    fn list_relative_files(root: &std::path::Path) -> Vec<String> {
        let mut files = Vec::new();
        let mut pending = vec![root.to_path_buf()];
        while let Some(dir) = pending.pop() {
            for entry in std::fs::read_dir(&dir).expect("read fixture dir") {
                let entry = entry.expect("fixture dir entry");
                let path = entry.path();
                if path.is_dir() {
                    pending.push(path);
                } else {
                    files.push(
                        path.strip_prefix(root)
                            .expect("relative fixture path")
                            .display()
                            .to_string(),
                    );
                }
            }
        }
        files.sort();
        files
    }

    fn check_active_track_outcome(args: CatalogueLintCheckActiveTrackArgs) -> CommandOutcome {
        let track_id = match args.track_id.map(|value| value.parse::<TrackIdInput>()).transpose() {
            Ok(track_id) => track_id,
            Err(error) => {
                return CommandOutcome::failure(Some(format!("invalid track id: {error}")));
            }
        };
        let workspace_root = match TrackWorkspaceRootInput::try_from(args.workspace_root) {
            Ok(workspace_root) => workspace_root,
            Err(error) => return CommandOutcome::failure(Some(error.to_string())),
        };
        let rules_file = match args.rules_file.map(TrackLintRulesFileInput::try_new).transpose() {
            Ok(rules_file) => rules_file,
            Err(error) => return CommandOutcome::failure(Some(error.to_string())),
        };
        TrackCompositionRoot::new().track_tddd_driver().handle(TrackTdddInput::CatalogueLintActive(
            TrackTdddCatalogueLintActiveInput { track_id, workspace_root, rules_file },
        ))
    }

    fn run_migrated_call_site(
        workspace: &std::path::Path,
        track_id: &str,
        rules_file: Option<std::path::PathBuf>,
    ) -> (CommandOutcome, Vec<String>) {
        let before = list_relative_files(workspace);
        let outcome = check_active_track_outcome(CatalogueLintCheckActiveTrackArgs {
            track_id: Some(track_id.to_owned()),
            workspace_root: workspace.to_path_buf(),
            rules_file,
        });
        let after = list_relative_files(workspace);
        assert_eq!(after, before, "catalogue-lint must not persist extra files");
        (outcome, after)
    }

    #[test]
    fn test_check_active_track_call_site_preserves_cli_contract_across_migration() {
        let workspace = tempfile::tempdir().expect("temp workspace");
        let root = workspace.path();
        write_file(&root.join("architecture-rules.json"), RULES_JSON);
        write_file(
            &root.join(".harness/catalogue-lint/config.json"),
            LINT_CONFIG_WITH_INVARIANT_RULE,
        );
        write_file(
            &root.join("track/items/test-track/domain-types.json"),
            CATALOGUE_WITH_INVARIANT,
        );

        let (outcome, _) = run_migrated_call_site(root, "test-track", None);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None, "stdout must stay empty on zero violations");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("Found 0 violation(s) across 1 layer(s)")),
            "stderr must keep the zero-violation summary: {:?}",
            outcome.stderr
        );

        write_file(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);
        let (outcome, _) = run_migrated_call_site(root, "test-track", None);
        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome.stdout.as_deref().is_some_and(|stdout| {
                stdout.contains("FieldNonEmpty") && stdout.contains("BareValueObject")
            }),
            "stdout must keep the violation detail: {:?}",
            outcome.stdout
        );
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("Found 1 violation(s) across 1 layer(s)")),
            "stderr must keep the violation summary: {:?}",
            outcome.stderr
        );

        std::fs::remove_file(root.join(".harness/catalogue-lint/config.json"))
            .expect("remove lint config");
        let (outcome, _) = run_migrated_call_site(root, "test-track", None);
        assert_eq!(outcome.exit_code, 1);
        assert_eq!(outcome.stdout, None, "missing config must not write stdout");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("lint config not found")),
            "stderr must keep the missing-config diagnostic: {:?}",
            outcome.stderr
        );

        write_file(
            &root.join(".harness/catalogue-lint/config.json"),
            LINT_CONFIG_WITH_INVARIANT_RULE,
        );
        std::fs::remove_file(root.join("track/items/test-track/domain-types.json"))
            .expect("remove catalogue");
        let (outcome, files) = run_migrated_call_site(root, "test-track", None);
        assert_eq!(outcome.exit_code, 0);
        assert_eq!(outcome.stdout, None, "skip must not write stdout");
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("catalogue-lint skipped")),
            "stderr must keep the skip diagnostic: {:?}",
            outcome.stderr
        );
        assert!(
            !files
                .iter()
                .any(|path| path.contains("impl-plan.json") || path.ends_with(".commit_hash")),
            "skip must not persist track state: {files:?}"
        );

        let custom = root.join("custom-lint-config.json");
        write_file(&custom, LINT_CONFIG_WITH_INVARIANT_RULE);
        write_file(&root.join("track/items/test-track/domain-types.json"), CATALOGUE_NO_INVARIANTS);
        let (outcome, _) = run_migrated_call_site(root, "test-track", Some(custom));
        assert_eq!(outcome.exit_code, 1);
        assert!(
            outcome.stdout.as_deref().is_some_and(|stdout| stdout.contains("FieldNonEmpty")),
            "--rules-file must still select the override config: {:?}",
            outcome.stdout
        );
        assert!(
            outcome
                .stderr
                .as_deref()
                .is_some_and(|stderr| stderr.contains("Found 1 violation(s) across 1 layer(s)")),
            "--rules-file must keep the violation summary: {:?}",
            outcome.stderr
        );
    }
}
