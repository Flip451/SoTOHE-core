//! Skill compliance use case boundary wrappers.
//!
//! Wraps `domain::skill_compliance` so that the CLI layer (`commands/hook.rs`)
//! never imports `domain::skill_compliance::SkillMatch` or
//! `domain::skill_compliance::ComplianceContext` directly (CN-01 / AC-03).

/// Returns `true` when the given prompt contains a recognized `/track:*` command.
///
/// Callers use this for early-exit guards without needing to import any
/// domain skill-compliance types.
#[must_use]
pub fn has_skill_command(prompt: &str) -> bool {
    domain::skill_compliance::detect_skill_command(prompt).is_some()
}

/// Runs a skill compliance check and returns the rendered `additionalContext`
/// string if any compliance guidance should be injected, or `None` otherwise.
///
/// The returned `String` is ready to embed in the Claude hook `additionalContext`
/// JSON field. Returns `None` when the prompt contains no recognized `/track:*`
/// command or when no guidance needs to be injected.
#[must_use]
pub fn check_compliance_render(prompt: &str) -> Option<String> {
    domain::skill_compliance::check_compliance(prompt).render()
}
