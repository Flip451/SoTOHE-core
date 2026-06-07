//! Tests for [`orchestra`] (split out to keep the main module under the 200-400 line guideline).

use std::collections::BTreeSet;
use std::path::Path;

use serde_json::json;
use tempfile::TempDir;

use super::*;

// -----------------------------------------------------------------------
// Helpers
// -----------------------------------------------------------------------

fn write_settings(root: &Path, value: &serde_json::Value) {
    let dir = root.join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), serde_json::to_string(value).unwrap()).unwrap();
}

fn all_expected_allow_entries() -> Vec<&'static str> {
    EXPECTED_OTHER_ALLOW
        .iter()
        .chain(EXPECTED_GIT_ALLOW)
        .chain(EXPECTED_CARGO_MAKE_ALLOW)
        .map(|(k, _)| *k)
        .collect()
}

// -----------------------------------------------------------------------
// load_settings
// -----------------------------------------------------------------------

#[test]
fn test_load_settings_missing_file_returns_error() {
    let tmp = TempDir::new().unwrap();
    let result = load_settings(tmp.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Missing settings file"));
}

#[test]
fn test_load_settings_invalid_json_returns_error() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), "not json").unwrap();
    let result = load_settings(tmp.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Invalid JSON"));
}

#[test]
fn test_load_settings_non_object_returns_error() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), "[1,2,3]").unwrap();
    let result = load_settings(tmp.path());
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("JSON object"));
}

#[test]
fn test_load_settings_valid_returns_ok() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("settings.json"), r#"{"foo":"bar"}"#).unwrap();
    let result = load_settings(tmp.path());
    assert!(result.is_ok());
}

// -----------------------------------------------------------------------
// load_permission_extensions
// -----------------------------------------------------------------------

#[test]
fn test_load_permission_extensions_missing_file_returns_empty() {
    let tmp = TempDir::new().unwrap();
    let result = load_permission_extensions(tmp.path());
    assert!(result.is_ok());
    assert!(result.unwrap().is_empty());
}

#[test]
fn test_load_permission_extensions_valid_returns_entries() {
    let tmp = TempDir::new().unwrap();
    let dir = tmp.path().join(".claude");
    std::fs::create_dir_all(&dir).unwrap();
    let data = json!({"extra_allow": ["Bash(cargo make my-custom-task)"]});
    std::fs::write(dir.join("permission-extensions.json"), serde_json::to_string(&data).unwrap())
        .unwrap();
    let result = load_permission_extensions(tmp.path()).unwrap();
    assert_eq!(result, vec!["Bash(cargo make my-custom-task)"]);
}

// -----------------------------------------------------------------------
// hook_commands
// -----------------------------------------------------------------------

#[test]
fn test_hook_commands_extracts_commands() {
    let settings = json!({
        "hooks": {
            "PreToolUse": [{
                "hooks": [
                    {"command": "echo hello"},
                    {"command": "echo world"}
                ]
            }]
        }
    });
    let cmds = hook_commands(&settings).unwrap();
    assert_eq!(cmds.len(), 2);
    assert!(cmds.contains(&"echo hello".to_owned()));
    assert!(cmds.contains(&"echo world".to_owned()));
}

#[test]
fn test_hook_commands_missing_hooks_field_returns_error() {
    let settings = json!({"permissions": {}});
    let result = hook_commands(&settings);
    assert!(result.is_err());
}

// -----------------------------------------------------------------------
// permission_set
// -----------------------------------------------------------------------

#[test]
fn test_permission_set_returns_correct_entries() {
    let settings = json!({
        "permissions": {
            "allow": ["Bash(true)", "Read(./**)"],
            "deny": []
        }
    });
    let allow = permission_set(&settings, "allow").unwrap();
    assert!(allow.contains("Bash(true)"));
    assert!(allow.contains("Read(./**)"));
}

// -----------------------------------------------------------------------
// cargo_make_task_name / git_subcommand_name
// -----------------------------------------------------------------------

#[test]
fn test_cargo_make_task_name_matches_plain_task() {
    assert_eq!(cargo_make_task_name("Bash(cargo make ci)"), Some("ci".to_owned()));
}

#[test]
fn test_cargo_make_task_name_matches_wildcard_task() {
    assert_eq!(
        cargo_make_task_name("Bash(cargo make track-transition:*)"),
        Some("track-transition".to_owned())
    );
}

#[test]
fn test_cargo_make_task_name_returns_none_for_non_matching() {
    assert_eq!(cargo_make_task_name("Bash(git status:*)"), None);
    assert_eq!(cargo_make_task_name("Read(./**)"), None);
}

#[test]
fn test_git_subcommand_name_matches() {
    assert_eq!(git_subcommand_name("Bash(git show:*)"), Some("show".to_owned()));
    assert_eq!(git_subcommand_name("Bash(git rev-parse:*)"), Some("rev-parse".to_owned()));
}

#[test]
fn test_git_subcommand_name_returns_none_for_non_matching() {
    assert_eq!(git_subcommand_name("Bash(cargo make ci)"), None);
}

// -----------------------------------------------------------------------
// verify_env
// -----------------------------------------------------------------------

#[test]
fn test_verify_env_passes_with_valid_config() {
    let settings = json!({
        "env": {
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1",
            "CLAUDE_CODE_SUBAGENT_MODEL": "claude-sonnet-4-6"
        }
    });
    let mut outcome = VerifyOutcome::pass();
    verify_env(&settings, &mut outcome);
    assert!(outcome.is_ok());
}

#[test]
fn test_verify_env_fails_with_missing_agent_teams() {
    let settings = json!({
        "env": {
            "CLAUDE_CODE_SUBAGENT_MODEL": "claude-sonnet-4-6"
        }
    });
    let mut outcome = VerifyOutcome::pass();
    verify_env(&settings, &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_verify_env_fails_with_unknown_subagent_model() {
    let settings = json!({
        "env": {
            "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1",
            "CLAUDE_CODE_SUBAGENT_MODEL": "gpt-99"
        }
    });
    let mut outcome = VerifyOutcome::pass();
    verify_env(&settings, &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_verify_env_fails_when_env_missing() {
    let settings = json!({});
    let mut outcome = VerifyOutcome::pass();
    verify_env(&settings, &mut outcome);
    assert!(outcome.has_errors());
}

// -----------------------------------------------------------------------
// verify_denylist
// -----------------------------------------------------------------------

#[test]
fn test_verify_denylist_passes_when_all_present() {
    let deny: BTreeSet<String> = EXPECTED_DENY.iter().map(|(k, _)| k.to_string()).collect();
    let mut outcome = VerifyOutcome::pass();
    verify_denylist(&deny, &mut outcome);
    assert!(outcome.is_ok());
}

#[test]
fn test_verify_denylist_fails_when_entry_missing() {
    let mut deny: BTreeSet<String> = EXPECTED_DENY.iter().map(|(k, _)| k.to_string()).collect();
    deny.remove("Read(./.env)");
    let mut outcome = VerifyOutcome::pass();
    verify_denylist(&deny, &mut outcome);
    assert!(outcome.has_errors());
}

// -----------------------------------------------------------------------
// verify_allowlist
// -----------------------------------------------------------------------

#[test]
fn test_verify_allowlist_passes_with_all_expected_entries() {
    let allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    let mut outcome = VerifyOutcome::pass();
    verify_allowlist(&allow, &[], &mut outcome);
    assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
}

#[test]
fn test_verify_allowlist_fails_when_expected_entry_missing() {
    let mut allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    allow.remove("Bash(true)");
    let mut outcome = VerifyOutcome::pass();
    verify_allowlist(&allow, &[], &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_verify_allowlist_fails_when_forbidden_entry_present() {
    let mut allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    allow.insert("Bash(git add:*)".to_owned());
    let mut outcome = VerifyOutcome::pass();
    verify_allowlist(&allow, &[], &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_verify_allowlist_fails_on_unexpected_entry() {
    let mut allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    allow.insert("Bash(some-unknown-tool)".to_owned());
    let mut outcome = VerifyOutcome::pass();
    verify_allowlist(&allow, &[], &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_verify_allowlist_accepts_extra_allow_entries() {
    let mut allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    allow.insert("Bash(cargo make my-project-task)".to_owned());
    let extra_allow = vec!["Bash(cargo make my-project-task)".to_owned()];
    let mut outcome = VerifyOutcome::pass();
    verify_allowlist(&allow, &extra_allow, &mut outcome);
    assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
}

// -----------------------------------------------------------------------
// verify_agent_definitions
// -----------------------------------------------------------------------

#[test]
fn test_verify_agent_definitions_passes_when_files_exist() {
    let tmp = TempDir::new().unwrap();
    let agents = tmp.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).unwrap();
    for f in REQUIRED_AGENT_FILES {
        std::fs::write(agents.join(f), "# agent").unwrap();
    }
    let mut outcome = VerifyOutcome::pass();
    verify_agent_definitions(tmp.path(), &mut outcome);
    assert!(outcome.is_ok());
}

#[test]
fn test_verify_agent_definitions_fails_when_file_missing() {
    let tmp = TempDir::new().unwrap();
    let agents = tmp.path().join(".claude").join("agents");
    std::fs::create_dir_all(&agents).unwrap();
    if let Some(f) = REQUIRED_AGENT_FILES.first() {
        std::fs::write(agents.join(f), "# agent").unwrap();
    }
    let mut outcome = VerifyOutcome::pass();
    verify_agent_definitions(tmp.path(), &mut outcome);
    if REQUIRED_AGENT_FILES.len() > 1 {
        assert!(outcome.has_errors());
    }
}

// -----------------------------------------------------------------------
// verify_hook_paths
// -----------------------------------------------------------------------

#[test]
fn test_verify_hook_paths_reports_missing_command_fragment() {
    let commands = vec!["echo hello".to_owned()];
    let tmp = TempDir::new().unwrap();
    let mut outcome = VerifyOutcome::pass();
    verify_hook_paths(&commands, tmp.path(), &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_verify_hook_paths_passes_when_all_fragments_present() {
    let tmp = TempDir::new().unwrap();

    // Build a single command that contains every required hook command fragment.
    // After RV2-17 there are no Python hook script paths to inject — only the
    // Rust hook command-fragment expectations remain.
    let mut big_cmd = String::new();
    for (_, fragments) in EXPECTED_HOOK_COMMANDS {
        for f in *fragments {
            big_cmd.push_str(f);
            big_cmd.push(' ');
        }
    }

    let commands = vec![big_cmd];
    let mut outcome = VerifyOutcome::pass();
    verify_hook_paths(&commands, tmp.path(), &mut outcome);
    assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
}

#[test]
fn test_verify_hook_paths_does_not_require_any_python_hook_scripts() {
    // Post RV2-17: EXPECTED_HOOK_PATHS must be empty so that no .claude/hooks/*.py
    // files are required to exist on disk. Verifying with an absent .claude/hooks/
    // directory and a command list containing only the required Rust hook fragments
    // should produce zero findings.
    assert!(
        EXPECTED_HOOK_PATHS.is_empty(),
        "EXPECTED_HOOK_PATHS must be empty after Python hooks removal (RV2-17); \
         found {} entries",
        EXPECTED_HOOK_PATHS.len()
    );

    let tmp = TempDir::new().unwrap();
    // Intentionally do NOT create .claude/hooks/ — verify it is no longer required.
    assert!(!tmp.path().join(".claude").join("hooks").exists());

    let mut big_cmd = String::new();
    for (_, fragments) in EXPECTED_HOOK_COMMANDS {
        for f in *fragments {
            big_cmd.push_str(f);
            big_cmd.push(' ');
        }
    }
    let commands = vec![big_cmd];
    let mut outcome = VerifyOutcome::pass();
    verify_hook_paths(&commands, tmp.path(), &mut outcome);
    assert!(
        outcome.is_ok(),
        "verify_hook_paths must not require any Python hook scripts; errors: {:?}",
        outcome.findings()
    );
}

// -----------------------------------------------------------------------
// verify_teammate_idle_feedback
// -----------------------------------------------------------------------

#[test]
fn test_verify_teammate_idle_feedback_passes_when_markers_present() {
    let feedback = "parent directory and agent-teams logs here";
    let settings = json!({
        "hooks": {
            "TeammateIdle": [{
                "hooks": [{"command": feedback}]
            }]
        }
    });
    let mut outcome = VerifyOutcome::pass();
    verify_teammate_idle_feedback(&settings, &mut outcome);
    assert!(outcome.is_ok());
}

#[test]
fn test_verify_teammate_idle_feedback_fails_when_marker_absent() {
    let settings = json!({
        "hooks": {
            "TeammateIdle": [{
                "hooks": [{"command": "some other feedback"}]
            }]
        }
    });
    let mut outcome = VerifyOutcome::pass();
    verify_teammate_idle_feedback(&settings, &mut outcome);
    assert!(outcome.has_errors());
}

// -----------------------------------------------------------------------
// verify_no_hardcoded_codex_model_literals
// -----------------------------------------------------------------------

#[test]
fn test_verify_no_hardcoded_codex_model_literals_passes_for_clean_files() {
    let tmp = TempDir::new().unwrap();
    let skills = tmp.path().join(".claude").join("skills");
    std::fs::create_dir_all(&skills).unwrap();
    std::fs::write(skills.join("clean.md"), "Use `{model}` from profiles.").unwrap();
    let mut outcome = VerifyOutcome::pass();
    verify_no_hardcoded_codex_model_literals(tmp.path(), &mut outcome);
    assert!(outcome.is_ok());
}

#[test]
fn test_verify_no_hardcoded_codex_model_literals_fails_on_gpt_literal() {
    let tmp = TempDir::new().unwrap();
    let skills = tmp.path().join(".claude").join("skills");
    std::fs::create_dir_all(&skills).unwrap();
    std::fs::write(skills.join("bad.md"), "Use gpt-4 for this task.").unwrap();
    let mut outcome = VerifyOutcome::pass();
    verify_no_hardcoded_codex_model_literals(tmp.path(), &mut outcome);
    assert!(outcome.has_errors());
}

// -----------------------------------------------------------------------
// validate_permission_extensions
// -----------------------------------------------------------------------

#[test]
fn test_validate_extensions_accepts_valid_project_task() {
    let mut allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    let extra_entry = "Bash(cargo make my-project-task)".to_owned();
    allow.insert(extra_entry.clone());
    let extra_allow = vec![extra_entry];
    let mut outcome = VerifyOutcome::pass();
    validate_permission_extensions(&extra_allow, &allow, &mut outcome);
    assert!(outcome.is_ok(), "errors: {:?}", outcome.findings());
}

#[test]
fn test_validate_extensions_rejects_forbidden_entry() {
    let mut allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    let forbidden_entry = "Bash(git add:*)".to_owned();
    allow.insert(forbidden_entry.clone());
    let extra_allow = vec![forbidden_entry];
    let mut outcome = VerifyOutcome::pass();
    validate_permission_extensions(&extra_allow, &allow, &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_validate_extensions_rejects_baseline_entry() {
    let all_expected: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    let extra_allow = vec!["Bash(true)".to_owned()];
    let mut outcome = VerifyOutcome::pass();
    validate_permission_extensions(&extra_allow, &all_expected, &mut outcome);
    assert!(outcome.has_errors());
}

#[test]
fn test_validate_extensions_rejects_scripts_direct_access() {
    let mut allow: BTreeSet<String> =
        all_expected_allow_entries().iter().map(|s| s.to_string()).collect();
    let scripts_entry = "Bash(python3 scripts/my_script.py:*)".to_owned();
    allow.insert(scripts_entry.clone());
    let extra_allow = vec![scripts_entry];
    let mut outcome = VerifyOutcome::pass();
    validate_permission_extensions(&extra_allow, &allow, &mut outcome);
    assert!(outcome.has_errors());
}

// -----------------------------------------------------------------------
// verify_no_local_settings_committed
// -----------------------------------------------------------------------

#[test]
fn test_verify_no_local_settings_committed_passes_in_non_git_dir() {
    let tmp = TempDir::new().unwrap();
    let mut outcome = VerifyOutcome::pass();
    verify_no_local_settings_committed(tmp.path(), &mut outcome);
    // In a non-git dir, git exits 128 -- treated as ok
    assert!(outcome.is_ok());
}

// -----------------------------------------------------------------------
// known_cargo_make_tasks / known_git_subcommands
// -----------------------------------------------------------------------

#[test]
fn test_known_cargo_make_tasks_contains_baseline_tasks() {
    let tasks = known_cargo_make_tasks();
    assert!(tasks.contains("ci"));
    assert!(tasks.contains("test"));
    assert!(tasks.contains("clippy"));
}

#[test]
fn test_known_git_subcommands_contains_baseline_subcommands() {
    let subs = known_git_subcommands();
    assert!(subs.contains("status"));
    assert!(subs.contains("diff"));
    assert!(subs.contains("log"));
}

// -----------------------------------------------------------------------
// expected_allow_map completeness
// -----------------------------------------------------------------------

#[test]
fn test_expected_allow_map_has_all_three_sections() {
    let map = expected_allow_map();
    // Other allow
    assert!(map.contains_key("Read(./**)"));
    assert!(map.contains_key("Bash(true)"));
    // Git allow
    assert!(map.contains_key("Bash(git status:*)"));
    // Cargo make allow
    assert!(map.contains_key("Bash(cargo make ci)"));
    assert!(map.contains_key("Bash(cargo make track-switch-main)"));
}

// -----------------------------------------------------------------------
// Integration: verify() with the real project root (smoke test)
// -----------------------------------------------------------------------

#[test]
fn test_verify_does_not_panic_on_project_root() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(std::path::Path::new("."));
    let outcome = verify(root);
    // Just confirm it returns without panicking.
    let _ = outcome.findings().len();
}

// -----------------------------------------------------------------------
// verify() early-exit on missing settings
// -----------------------------------------------------------------------

#[test]
fn test_verify_returns_error_outcome_when_settings_missing() {
    let tmp = TempDir::new().unwrap();
    let outcome = verify(tmp.path());
    assert!(outcome.has_errors());
    let msgs: Vec<&str> = outcome.findings().iter().map(|f| f.message()).collect();
    assert!(msgs.iter().any(|m| m.contains("Missing settings file")));
}

// -----------------------------------------------------------------------
// write_settings helper (used in integration sub-tests below)
// -----------------------------------------------------------------------

#[test]
fn test_write_settings_helper_creates_readable_file() {
    let tmp = TempDir::new().unwrap();
    let settings = json!({"hooks": {}, "permissions": {"allow": [], "deny": []}});
    write_settings(tmp.path(), &settings);
    let loaded = load_settings(tmp.path());
    assert!(loaded.is_ok());
}
