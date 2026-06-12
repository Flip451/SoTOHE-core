//! Verification sub-functions for orchestra guardrail checks.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use domain::verify::{VerifyFinding, VerifyOutcome};

use super::constants::{
    AGENTS_DIR, ALLOWED_EXTRA_GIT_SUBCOMMANDS, EXPECTED_DENY, EXPECTED_HOOK_COMMANDS,
    EXPECTED_HOOK_PATHS, FORBIDDEN_HOOK_COMMAND_FRAGMENTS, HARDCODED_CODEX_MODEL_RE,
    MODEL_RESOLUTION_TARGETS, PERMISSION_EXTENSIONS_PATH, REQUIRED_AGENT_FILES,
    REVIEW_WRAPPER_TARGETS, SETTINGS_LOCAL_PATH, SETTINGS_PATH, SUBAGENT_MODEL_ALLOWLIST,
    TEAMMATE_IDLE_MARKERS,
};
use super::helpers::{
    cargo_make_task_name, expected_allow_map, forbidden_allow_set, git_subcommand_name,
    is_direct_repo_script_permission, known_cargo_make_tasks, known_git_subcommands,
};

// ---------------------------------------------------------------------------
// Verification sub-functions
// ---------------------------------------------------------------------------

/// Verify hook paths are present in commands and hook files exist on disk.
pub(crate) fn verify_hook_paths(commands: &[String], root: &Path, outcome: &mut VerifyOutcome) {
    for (hook_path, label) in EXPECTED_HOOK_PATHS {
        if !commands.iter().any(|c| c.contains(hook_path)) {
            outcome.add(VerifyFinding::error(format!("Missing in {SETTINGS_PATH}: {label}")));
        }
        if !root.join(hook_path).is_file() {
            outcome.add(VerifyFinding::error(format!("Missing hook file: {hook_path}")));
        }
    }

    for (label, fragments) in EXPECTED_HOOK_COMMANDS {
        let found = commands.iter().any(|c| fragments.iter().all(|f| c.contains(*f)));
        if !found {
            let frags = fragments.join(", ");
            outcome.add(VerifyFinding::error(format!(
                "Missing in {SETTINGS_PATH}: {label} (expected fragments: {frags})"
            )));
        }
    }

    for (label, fragment) in FORBIDDEN_HOOK_COMMAND_FRAGMENTS {
        if commands.iter().any(|c| c.contains(*fragment)) {
            outcome.add(VerifyFinding::error(format!(
                "{SETTINGS_PATH} contains forbidden hook command fragment for {label}: {fragment}"
            )));
        }
    }
}

/// Verify the allow list: expected present, forbidden absent, no unexpected entries.
pub(crate) fn verify_allowlist(
    allow: &BTreeSet<String>,
    extra_allow: &[String],
    outcome: &mut VerifyOutcome,
) {
    let expected = expected_allow_map();
    let forbidden = forbidden_allow_set();
    let extra_set: BTreeSet<&str> = extra_allow.iter().map(String::as_str).collect();

    for (entry, label) in &expected {
        if !allow.contains(*entry) {
            outcome.add(VerifyFinding::error(format!("Missing in {SETTINGS_PATH}: {label}")));
        }
    }

    for entry in &forbidden {
        if allow.contains(*entry) {
            outcome.add(VerifyFinding::error(format!(
                "{SETTINGS_PATH} contains {entry} - direct access would be silently allowed"
            )));
        }
    }

    for entry in allow {
        let s = entry.as_str();
        if forbidden.contains(s) {
            continue; // already reported above
        }
        if is_direct_repo_script_permission(entry) {
            outcome.add(VerifyFinding::error(format!(
                "{SETTINGS_PATH} contains {entry} - direct repo scripts must be routed \
                through cargo make wrappers"
            )));
            continue;
        }
        if !expected.contains_key(s) && !extra_set.contains(s) {
            outcome.add(VerifyFinding::error(format!(
                "{SETTINGS_PATH} contains unexpected allow entry: {entry} - \
                add it to {PERMISSION_EXTENSIONS_PATH} if this project intentionally extends \
                the baseline"
            )));
        }
    }
}

/// Validate entries in `permission-extensions.json` `extra_allow`.
pub(crate) fn validate_permission_extensions(
    extra_allow: &[String],
    allow: &BTreeSet<String>,
    outcome: &mut VerifyOutcome,
) {
    let expected = expected_allow_map();
    let forbidden = forbidden_allow_set();
    let known_tasks = known_cargo_make_tasks();
    let known_git = known_git_subcommands();
    let allowed_git_subs: BTreeSet<&str> = ALLOWED_EXTRA_GIT_SUBCOMMANDS.iter().copied().collect();

    for entry in extra_allow {
        let s = entry.as_str();

        if !allow.contains(s) {
            outcome.add(VerifyFinding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains latent extra_allow entry not present in \
                {SETTINGS_PATH} permissions.allow: {entry}"
            )));
            continue;
        }

        if expected.contains_key(s) {
            outcome.add(VerifyFinding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains baseline allow entry: {entry} - \
                extra_allow is only for project-specific additions"
            )));
            continue;
        }

        if forbidden.contains(s) {
            outcome.add(VerifyFinding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains forbidden extra_allow entry: {entry} - \
                direct access would be silently allowed"
            )));
            continue;
        }

        if is_direct_repo_script_permission(entry) {
            outcome.add(VerifyFinding::error(format!(
                "{PERMISSION_EXTENSIONS_PATH} contains {entry} - \
                direct repo scripts must be routed through cargo make wrappers"
            )));
            continue;
        }

        if let Some(task) = cargo_make_task_name(s) {
            if known_tasks.contains(&task) {
                outcome.add(VerifyFinding::error(format!(
                    "{PERMISSION_EXTENSIONS_PATH} contains extension for guarded cargo make \
                    task: {entry} - baseline or approval-gated cargo make task names cannot be \
                    widened via extra_allow"
                )));
            }
            // else: valid project cargo make extension
            continue;
        }

        if let Some(sub) = git_subcommand_name(s) {
            if allowed_git_subs.contains(sub.as_str()) {
                if known_git.contains(&sub) {
                    outcome.add(VerifyFinding::error(format!(
                        "{PERMISSION_EXTENSIONS_PATH} contains extension for guarded git \
                        subcommand: {entry} - baseline git permissions cannot be widened via \
                        extra_allow"
                    )));
                }
                // else: valid read-only git extension
                continue;
            }
        }

        outcome.add(VerifyFinding::error(format!(
            "{PERMISSION_EXTENSIONS_PATH} contains unsupported extra_allow entry: {entry} - \
            only project-specific Bash(cargo make <task>) / Bash(cargo make <task>:*) and \
            whitelisted read-only Bash(git <subcommand>) / Bash(git <subcommand>:*) are allowed"
        )));
    }
}

/// Verify deny list entries are present.
pub(crate) fn verify_denylist(deny: &BTreeSet<String>, outcome: &mut VerifyOutcome) {
    for (entry, label) in EXPECTED_DENY {
        if !deny.contains(*entry) {
            outcome.add(VerifyFinding::error(format!("Missing in {SETTINGS_PATH}: {label}")));
        }
    }
}

/// Verify env configuration: agent teams flag and subagent model.
pub(crate) fn verify_env(settings: &serde_json::Value, outcome: &mut VerifyOutcome) {
    let env = match settings.get("env").and_then(|v| v.as_object()) {
        Some(e) => e,
        None => {
            outcome
                .add(VerifyFinding::error(format!("{SETTINGS_PATH} is missing env configuration")));
            return;
        }
    };

    match env.get("CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS").and_then(|v| v.as_str()) {
        Some("1") => {}
        _ => {
            outcome.add(VerifyFinding::error(format!(
                "Missing in {SETTINGS_PATH}: agent teams enabled"
            )));
        }
    }

    let model = env.get("CLAUDE_CODE_SUBAGENT_MODEL").and_then(|v| v.as_str());
    match model {
        Some(m) if SUBAGENT_MODEL_ALLOWLIST.contains(&m) => {}
        other => {
            let allowlist: Vec<&str> = SUBAGENT_MODEL_ALLOWLIST.to_vec();
            outcome.add(VerifyFinding::error(format!(
                "{SETTINGS_PATH}: CLAUDE_CODE_SUBAGENT_MODEL must be one of {allowlist:?}, \
                got {other:?}"
            )));
        }
    }
}

/// Verify TeammateIdle hooks contain required marker phrases.
pub(crate) fn verify_teammate_idle_feedback(
    settings: &serde_json::Value,
    outcome: &mut VerifyOutcome,
) {
    let hooks = match settings.get("hooks").and_then(|v| v.as_object()) {
        Some(h) => h,
        None => return,
    };

    let mut feedback_text = String::new();
    if let Some(idle_bindings) = hooks.get("TeammateIdle").and_then(|v| v.as_array()) {
        for binding in idle_bindings {
            if let Some(nested) = binding.get("hooks").and_then(|v| v.as_array()) {
                for hook in nested {
                    if let Some(cmd) = hook.get("command").and_then(|v| v.as_str()) {
                        feedback_text.push_str(cmd);
                    }
                }
            }
        }
    }

    for (marker, label) in TEAMMATE_IDLE_MARKERS {
        if !feedback_text.contains(marker) {
            outcome.add(VerifyFinding::error(format!(
                "Missing in {SETTINGS_PATH} TeammateIdle feedback: {label:?}"
            )));
        }
    }
}

/// Verify required agent definition files exist.
pub(crate) fn verify_agent_definitions(root: &Path, outcome: &mut VerifyOutcome) {
    for required in REQUIRED_AGENT_FILES {
        if !root.join(AGENTS_DIR).join(required).is_file() {
            outcome.add(VerifyFinding::error(format!(
                "Missing required agent definition: {AGENTS_DIR}/{required}"
            )));
        }
    }
}

/// Verify no hardcoded Codex model literals in `.claude/skills/` and `.claude/commands/`.
pub(crate) fn verify_no_hardcoded_codex_model_literals(root: &Path, outcome: &mut VerifyOutcome) {
    for dir in &[".claude/skills", ".claude/commands"] {
        let base = root.join(dir);
        if !base.is_dir() {
            continue;
        }
        if let Err(e) = scan_dir_for_gpt_pattern(&base, outcome) {
            outcome.add(VerifyFinding::error(format!("Error scanning {dir}: {e}")));
        }
    }
}

pub(crate) fn scan_dir_for_gpt_pattern(
    dir: &Path,
    outcome: &mut VerifyOutcome,
) -> Result<(), String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| format!("Cannot read dir {}: {e}", dir.display()))?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    paths.sort();

    for path in paths {
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "__pycache__") {
                continue;
            }
            scan_dir_for_gpt_pattern(&path, outcome)?;
        } else if path.is_file() {
            let text = std::fs::read_to_string(&path)
                .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
            if HARDCODED_CODEX_MODEL_RE.as_ref().is_some_and(|re| re.is_match(&text)) {
                outcome.add(VerifyFinding::error(format!(
                    "{} contains hardcoded Codex model literal matching {}",
                    path.display(),
                    r"gpt-\d+"
                )));
            }
        }
    }
    Ok(())
}

/// Verify override-first model resolution guidance in target files.
pub(crate) fn verify_override_first_model_resolution(root: &Path, outcome: &mut VerifyOutcome) {
    for (rel_path, label, required_snippets, forbidden_snippets) in MODEL_RESOLUTION_TARGETS {
        let path = root.join(rel_path);
        if !path.is_file() {
            outcome
                .add(VerifyFinding::error(format!("Missing model resolution target: {rel_path}")));
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                outcome.add(VerifyFinding::error(format!("Cannot read {rel_path}: {e}")));
                continue;
            }
        };

        let missing: Vec<&&str> =
            required_snippets.iter().filter(|s| !content.contains(**s)).collect();
        if !missing.is_empty() {
            let joined: Vec<&str> = missing.iter().map(|s| **s).collect();
            outcome.add(VerifyFinding::error(format!(
                "{rel_path} is missing canonical override-first guidance for {label}: {}",
                joined.join("; ")
            )));
        }

        for forbidden in *forbidden_snippets {
            if content.contains(*forbidden) {
                outcome.add(VerifyFinding::error(format!(
                    "{rel_path} still contains stale default_model-only guidance: {forbidden}"
                )));
            }
        }
    }
}

/// Verify reviewer wrapper guidance in target files.
pub(crate) fn verify_reviewer_wrapper_guidance(root: &Path, outcome: &mut VerifyOutcome) {
    for (rel_path, label, required_snippets, forbidden_snippets) in REVIEW_WRAPPER_TARGETS {
        let path = root.join(rel_path);
        if !path.is_file() {
            outcome
                .add(VerifyFinding::error(format!("Missing reviewer wrapper target: {rel_path}")));
            continue;
        }
        let content = match std::fs::read_to_string(&path) {
            Ok(c) => c,
            Err(e) => {
                outcome.add(VerifyFinding::error(format!("Cannot read {rel_path}: {e}")));
                continue;
            }
        };

        let missing: Vec<&&str> =
            required_snippets.iter().filter(|s| !content.contains(**s)).collect();
        if !missing.is_empty() {
            let joined: Vec<&str> = missing.iter().map(|s| **s).collect();
            outcome.add(VerifyFinding::error(format!(
                "{rel_path} is missing reviewer wrapper guidance for {label}: {}",
                joined.join("; ")
            )));
        }

        for forbidden in *forbidden_snippets {
            if content.contains(*forbidden) {
                outcome.add(VerifyFinding::error(format!(
                    "{rel_path} still contains stale reviewer command guidance: {forbidden}"
                )));
            }
        }
    }
}

/// Verify `.claude/settings.local.json` is NOT tracked by git.
pub(crate) fn verify_no_local_settings_committed(root: &Path, outcome: &mut VerifyOutcome) {
    let result = std::process::Command::new("git")
        .args(["ls-files", "--error-unmatch", SETTINGS_LOCAL_PATH])
        .current_dir(root)
        .output();

    match result {
        Err(e) => {
            outcome.add(VerifyFinding::error(format!(
                "Cannot run git ls-files to check {SETTINGS_LOCAL_PATH}: {e}"
            )));
        }
        Ok(output) => {
            if output.status.success() {
                outcome.add(VerifyFinding::error(format!(
                    "{SETTINGS_LOCAL_PATH} is tracked by git. \
                    Local overrides must not be committed — add to .gitignore and run: \
                    git rm --cached .claude/settings.local.json"
                )));
            } else {
                let stderr = String::from_utf8_lossy(&output.stderr);
                let code = output.status.code().unwrap_or(-1);

                if code == 128 && stderr.to_lowercase().contains("not a git repository") {
                    // Not a git repo — that's fine
                } else if code != 1 {
                    outcome.add(VerifyFinding::error(format!(
                        "git ls-files failed (exit {code}): {}",
                        stderr.trim()
                    )));
                }
                // exit code 1 = file not tracked = expected state
            }
        }
    }
}
