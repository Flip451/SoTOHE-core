//! Verify Claude orchestra hooks, permissions, and agent definitions.
//!
//! Rust port of `scripts/verify_orchestra_guardrails.py`.

mod checks;
mod constants;
mod helpers;
mod loaders;

use std::path::Path;

use domain::verify::VerifyFinding;

// Re-export domain type so tests can use it via `super::*`.
pub use domain::verify::VerifyOutcome;

// Re-export constants needed by tests via `super::*`.
#[cfg(test)]
pub(crate) use constants::{
    EXPECTED_CARGO_MAKE_ALLOW, EXPECTED_DENY, EXPECTED_GIT_ALLOW, EXPECTED_HOOK_COMMANDS,
    EXPECTED_HOOK_PATHS, EXPECTED_OTHER_ALLOW, REQUIRED_AGENT_FILES,
};

// Re-export helpers needed by tests via `super::*`.
#[cfg(test)]
pub(crate) use helpers::{
    cargo_make_task_name, expected_allow_map, git_subcommand_name, known_cargo_make_tasks,
    known_git_subcommands,
};

// Re-export loaders needed by tests via `super::*`.
#[cfg(test)]
pub(crate) use loaders::{
    hook_commands, load_permission_extensions, load_settings, permission_set,
};

// Re-export checks needed by tests via `super::*`.
#[cfg(test)]
pub(crate) use checks::{
    validate_permission_extensions, verify_agent_definitions, verify_allowlist, verify_denylist,
    verify_env, verify_hook_paths, verify_no_hardcoded_codex_model_literals,
    verify_no_local_settings_committed, verify_teammate_idle_feedback,
};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Verify Claude orchestra hooks, permissions, and agent definitions.
///
/// # Errors
///
/// Returns findings for every structural violation found in
/// `.claude/settings.json`, `.claude/permission-extensions.json`,
/// and the `.claude/agents/` directory.
pub fn verify(root: &Path) -> VerifyOutcome {
    let mut outcome = VerifyOutcome::pass();

    let settings = match loaders::load_settings(root) {
        Ok(s) => s,
        Err(e) => {
            outcome.add(VerifyFinding::error(e));
            return outcome;
        }
    };

    let extra_allow = match loaders::load_permission_extensions(root) {
        Ok(e) => e,
        Err(e) => {
            outcome.add(VerifyFinding::error(e));
            return outcome;
        }
    };

    let commands = match loaders::hook_commands(&settings) {
        Ok(c) => c,
        Err(e) => {
            outcome.add(VerifyFinding::error(e));
            return outcome;
        }
    };

    let allow = match loaders::permission_set(&settings, "allow") {
        Ok(a) => a,
        Err(e) => {
            outcome.add(VerifyFinding::error(e));
            return outcome;
        }
    };

    let deny = match loaders::permission_set(&settings, "deny") {
        Ok(d) => d,
        Err(e) => {
            outcome.add(VerifyFinding::error(e));
            return outcome;
        }
    };

    checks::verify_hook_paths(&commands, root, &mut outcome);
    checks::validate_permission_extensions(&extra_allow, &allow, &mut outcome);
    checks::verify_allowlist(&allow, &extra_allow, &mut outcome);
    checks::verify_denylist(&deny, &mut outcome);
    checks::verify_env(&settings, &mut outcome);
    checks::verify_teammate_idle_feedback(&settings, &mut outcome);
    checks::verify_agent_definitions(root, &mut outcome);
    checks::verify_no_hardcoded_codex_model_literals(root, &mut outcome);
    checks::verify_override_first_model_resolution(root, &mut outcome);
    checks::verify_reviewer_wrapper_guidance(root, &mut outcome);
    checks::verify_no_local_settings_committed(root, &mut outcome);

    outcome
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::indexing_slicing)]
#[path = "orchestra_tests.rs"]
mod tests;
