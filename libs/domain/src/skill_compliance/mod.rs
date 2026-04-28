//! Skill compliance check for `/track:*` commands.
//!
//! Detects `/track:*` slash commands in user prompts and generates
//! SKILL.md phase requirement reminders as `additionalContext`.

/// A detected `/track:*` command with its skill phase requirements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMatch {
    /// The matched command (e.g., "/track:plan").
    pub command: String,
    /// Phase reminders for the matched command.
    pub reminders: Vec<String>,
}

/// Result of a skill compliance check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplianceContext {
    /// Matched skill command (if any).
    pub skill_match: Option<SkillMatch>,
}

impl ComplianceContext {
    /// Returns `true` if no context was generated (nothing to inject).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skill_match.is_none()
    }

    /// Renders the compliance context as `additionalContext` string.
    #[must_use]
    pub fn render(&self) -> Option<String> {
        let skill = self.skill_match.as_ref()?;
        let mut lines = vec![format!(
            "[Skill Compliance] Detected `{}` — follow the SKILL.md workflow phases:",
            skill.command,
        )];
        for reminder in &skill.reminders {
            lines.push(format!("- {reminder}"));
        }
        Some(lines.join("\n"))
    }
}

// ---------------------------------------------------------------------------
// Skill command detection
// ---------------------------------------------------------------------------

/// Known `/track:*` commands and their SKILL.md phase reminders.
/// Ordered longest-first to avoid `/track:plan` matching `/track:plan-only`.
const SKILL_COMMANDS: &[(&str, &[&str])] = &[
    (
        "/track:plan-only",
        &[
            "Phase 0-3: Same as /track:plan",
            "Artifacts created on plan/<id> branch for PR review before activation",
        ],
    ),
    (
        "/track:plan",
        &[
            "Phase 0: Mode Selection (Full/Focused/Quick)",
            "Phase 1: UNDERSTAND (researcher capability + Claude Lead)",
            "Phase 1.5: DESIGN REVIEW (planner capability — mandatory for Full mode)",
            "Phase 2: RESEARCH & DESIGN (Agent Teams — Full mode only)",
            "Phase 3: PLAN & APPROVE (plan synthesis + user approval before artifacts)",
        ],
    ),
    (
        "/track:implement",
        &[
            "Read spec.md related_conventions before implementation",
            "Follow TDD cycle: Red → Green → Refactor",
            "Run cargo make ci before marking tasks done",
        ],
    ),
    (
        "/track:review",
        &[
            "Use the reviewer capability (resolved via agent-profiles.json)",
            "Do not substitute inline self-review for external reviewer",
            "Fix → rebuild → re-review until zero findings",
        ],
    ),
    (
        "/track:full-cycle",
        &[
            "Autonomous implementation: implement → review → fix → commit",
            "Read spec.md related_conventions before starting",
            "Follow TDD cycle: Red → Green → Refactor",
        ],
    ),
    (
        "/track:pr-review",
        &[
            "Use cargo make track-pr-review for full cycle (push + trigger + poll)",
            "Do NOT substitute manual sleep + gh api loops",
            "Continue until zero findings (👍) — do not stop at Accepted Deviations without user approval",
        ],
    ),
    (
        "/track:commit",
        &[
            "Verify staged scope matches intended commit",
            "Run cargo make track-commit-message (guarded commit)",
            "Attach git note after successful commit",
        ],
    ),
    ("/track:activate", &["Materialize a planning-only track and switch to its track branch"]),
    ("/track:ci", &["Run cargo make ci for standard CI checks"]),
    ("/track:status", &["Show current track progress from registry.md and metadata.json"]),
    ("/track:archive", &["Archive a completed track, moving it out of active view"]),
    ("/track:merge", &["Wait for PR CI checks to pass, then merge"]),
    ("/track:done", &["Switch to main, pull latest, and show track completion summary"]),
    ("/track:revert", &["Revert the latest track change set safely"]),
];

/// Detects `/track:*` commands in the prompt and returns the one that
/// appears earliest in the prompt. When multiple commands start at the
/// same position, the longest match wins (e.g. `/track:plan-only` over
/// `/track:plan`).
#[must_use]
pub fn detect_skill_command(prompt: &str) -> Option<SkillMatch> {
    let prompt_lower = prompt.to_lowercase();
    let mut best: Option<(usize, &str, &[&str])> = None;
    for (command, reminders) in SKILL_COMMANDS {
        // Scan all occurrences — the first match may fail boundary check
        // (e.g. "/track:planner then /track:plan").
        let mut search_start = 0;
        let found_pos = loop {
            let Some(rel) = prompt_lower[search_start..].find(command) else {
                break None;
            };
            let pos = search_start + rel;
            let after_pos = pos + command.len();
            let after_char = prompt_lower.as_bytes().get(after_pos).copied();
            let at_boundary =
                after_char.is_none_or(|b| !b.is_ascii_alphanumeric() && b != b'-' && b != b'_');
            if at_boundary {
                break Some(pos);
            }
            search_start = pos + 1;
            while search_start < prompt_lower.len() && !prompt_lower.is_char_boundary(search_start)
            {
                search_start += 1;
            }
        };
        let Some(pos) = found_pos else {
            continue;
        };
        let is_better = match &best {
            None => true,
            Some((best_pos, best_cmd, _)) => {
                pos < *best_pos || (pos == *best_pos && command.len() > best_cmd.len())
            }
        };
        if is_better {
            best = Some((pos, command, reminders));
        }
    }
    best.map(|(_, command, reminders)| SkillMatch {
        command: (*command).to_owned(),
        reminders: reminders.iter().map(|r| (*r).to_owned()).collect(),
    })
}

/// Performs a skill compliance check: detect a `/track:*` command in the prompt.
#[must_use]
pub fn check_compliance(prompt: &str) -> ComplianceContext {
    let skill_match = detect_skill_command(prompt);
    ComplianceContext { skill_match }
}

#[cfg(test)]
mod tests;
