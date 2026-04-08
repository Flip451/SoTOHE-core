//! Skill compliance check for `/track:*` commands.
//!
//! Detects `/track:*` slash commands in user prompts and generates
//! SKILL.md phase requirement reminders as `additionalContext`.
//! Also matches external guide triggers against the prompt.

/// A detected `/track:*` command with its skill phase requirements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillMatch {
    /// The matched command (e.g., "/track:plan").
    pub command: String,
    /// Phase reminders for the matched command.
    pub reminders: Vec<String>,
}

/// Result of external guide matching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuideMatch {
    /// Guide ID.
    pub id: String,
    /// The trigger keyword that matched.
    pub trigger: String,
    /// Guide summary lines.
    pub summary: Vec<String>,
    /// Project usage notes.
    pub project_usage: Vec<String>,
    /// Cache path for the raw guide file.
    pub cache_path: String,
}

/// External guide entry (domain representation — no serde here).
#[derive(Debug, Clone)]
pub struct GuideEntry {
    /// Unique guide identifier.
    pub id: String,
    /// Trigger keywords for matching.
    pub trigger_keywords: Vec<String>,
    /// Summary lines.
    pub summary: Vec<String>,
    /// Project usage notes.
    pub project_usage: Vec<String>,
    /// Cache path.
    pub cache_path: String,
}

/// Result of a skill compliance check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComplianceContext {
    /// Matched skill command (if any).
    pub skill_match: Option<SkillMatch>,
    /// Matched external guides.
    pub guide_matches: Vec<GuideMatch>,
}

impl ComplianceContext {
    /// Returns `true` if no context was generated (nothing to inject).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.skill_match.is_none() && self.guide_matches.is_empty()
    }

    /// Renders the compliance context as `additionalContext` string.
    #[must_use]
    pub fn render(&self) -> Option<String> {
        if self.is_empty() {
            return None;
        }

        let mut parts = Vec::new();

        if let Some(skill) = &self.skill_match {
            let mut lines = vec![format!(
                "[Skill Compliance] Detected `{}` — follow the SKILL.md workflow phases:",
                skill.command,
            )];
            for reminder in &skill.reminders {
                lines.push(format!("- {reminder}"));
            }
            parts.push(lines.join("\n"));
        }

        if !self.guide_matches.is_empty() {
            let mut lines = vec!["[External Guide Context] Relevant guide summaries:".to_owned()];
            for guide in &self.guide_matches {
                let summary = if guide.summary.is_empty() {
                    "Summary not recorded.".to_owned()
                } else {
                    guide.summary.join(" ")
                };
                let usage = if guide.project_usage.is_empty() {
                    "Project usage not recorded.".to_owned()
                } else {
                    guide.project_usage.join(" ")
                };
                lines.push(format!("- {} (trigger: {}): {}", guide.id, guide.trigger, summary));
                lines.push(format!("  project usage: {usage}"));
                if !guide.cache_path.is_empty() {
                    lines.push(format!("  cache path: {}", guide.cache_path));
                }
            }
            parts.push(lines.join("\n"));
        }

        Some(parts.join("\n\n"))
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

// ---------------------------------------------------------------------------
// External guide matching
// ---------------------------------------------------------------------------

/// Whether a trigger keyword matches in the given text.
///
/// Both text and trigger are lowercased internally.
/// Word-boundary aware for triggers containing alphanumeric characters.
#[must_use]
pub fn trigger_matches(text: &str, trigger: &str) -> bool {
    let text_lower = text.to_lowercase();
    let trigger_lower = trigger.to_lowercase();
    if trigger_lower.chars().any(|c| c.is_ascii_alphanumeric()) {
        // Word-boundary aware match — scan all occurrences
        let mut start = 0;
        while let Some(rel) = text_lower[start..].find(&trigger_lower) {
            let pos = start + rel;
            let before_ok = pos == 0
                || text_lower
                    .as_bytes()
                    .get(pos - 1)
                    .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
            let after_pos = pos + trigger_lower.len();
            let after_ok = after_pos >= text_lower.len()
                || text_lower
                    .as_bytes()
                    .get(after_pos)
                    .is_none_or(|&b| !b.is_ascii_alphanumeric() && b != b'_');
            if before_ok && after_ok {
                return true;
            }
            // Advance past the current match start on a valid char boundary.
            start = pos + 1;
            while start < text_lower.len() && !text_lower.is_char_boundary(start) {
                start += 1;
            }
        }
        false
    } else {
        text_lower.contains(&trigger_lower)
    }
}

/// Matches guide entries against the prompt text.
///
/// Returns up to `limit` matches, each with the guide and the trigger that matched.
#[must_use]
pub fn find_matching_guides(prompt: &str, guides: &[GuideEntry], limit: usize) -> Vec<GuideMatch> {
    let prompt_lower = prompt.to_lowercase();
    let mut matches = Vec::new();

    for guide in guides {
        if matches.len() >= limit {
            break;
        }
        for trigger in &guide.trigger_keywords {
            if trigger_matches(&prompt_lower, trigger) {
                matches.push(GuideMatch {
                    id: guide.id.clone(),
                    trigger: trigger.clone(),
                    summary: guide.summary.clone(),
                    project_usage: guide.project_usage.clone(),
                    cache_path: guide.cache_path.clone(),
                });
                break;
            }
        }
    }

    matches
}

/// Performs a full skill compliance check: detect command + match guides.
///
/// `track_context` is optional additional text (e.g. from spec.md/plan.md of the
/// active track) that is concatenated with the prompt for guide trigger matching.
/// This mirrors the Python `find_relevant_guides_for_track_workflow()` behavior.
#[must_use]
pub fn check_compliance(
    prompt: &str,
    track_context: Option<&str>,
    guides: &[GuideEntry],
    guide_limit: usize,
) -> ComplianceContext {
    let skill_match = detect_skill_command(prompt);
    // Match guides when any /track: command is detected (not just SKILL_COMMANDS).
    let has_track_command = prompt.to_lowercase().contains("/track:");
    let guide_matches = if has_track_command {
        let combined = match track_context {
            Some(ctx) if !ctx.is_empty() => format!("{prompt}\n{ctx}"),
            _ => prompt.to_owned(),
        };
        find_matching_guides(&combined, guides, guide_limit)
    } else {
        Vec::new()
    };
    ComplianceContext { skill_match, guide_matches }
}

#[cfg(test)]
mod tests;
