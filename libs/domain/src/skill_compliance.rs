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
        "/track:commit",
        &[
            "Verify staged scope matches intended commit",
            "Run cargo make track-commit-message (guarded commit)",
            "Attach git note after successful commit",
        ],
    ),
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
        if let Some(pos) = prompt_lower.find(command) {
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    // -- detect_skill_command --

    #[test]
    fn test_detect_skill_command_with_track_plan() {
        let result = detect_skill_command("/track:plan my-feature");
        assert!(result.is_some());
        let m = result.unwrap();
        assert_eq!(m.command, "/track:plan");
        assert!(!m.reminders.is_empty());
        assert!(m.reminders.iter().any(|r| r.contains("planner capability")));
    }

    #[test]
    fn test_detect_skill_command_with_track_implement() {
        let result = detect_skill_command("/track:implement");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:implement");
    }

    #[test]
    fn test_detect_skill_command_with_track_review() {
        let result = detect_skill_command("/track:review");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:review");
    }

    #[test]
    fn test_detect_skill_command_no_match() {
        let result = detect_skill_command("please review this code");
        assert!(result.is_none());
    }

    #[test]
    fn test_detect_skill_command_case_insensitive() {
        let result = detect_skill_command("/Track:Plan my-feature");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:plan");
    }

    #[test]
    fn test_detect_skill_command_plan_only() {
        let result = detect_skill_command("/track:plan-only my-feature");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:plan-only");
    }

    #[test]
    fn test_detect_skill_command_full_cycle() {
        let result = detect_skill_command("/track:full-cycle T01");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:full-cycle");
    }

    #[test]
    fn test_detect_skill_command_earliest_in_prompt_wins() {
        // /track:review appears before /track:implement
        let result = detect_skill_command("do /track:review then /track:implement");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:review");
    }

    #[test]
    fn test_detect_skill_command_longest_match_at_same_position() {
        // /track:plan-only and /track:plan both start at same position
        let result = detect_skill_command("/track:plan-only my-feature");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:plan-only");
    }

    #[test]
    fn test_detect_skill_command_commit() {
        let result = detect_skill_command("/track:commit fix things");
        assert!(result.is_some());
        assert_eq!(result.unwrap().command, "/track:commit");
    }

    // -- trigger_matches --

    #[test]
    fn test_trigger_matches_word_boundary() {
        assert!(trigger_matches("use harness pattern", "harness"));
        assert!(!trigger_matches("use harnessing pattern", "harness"));
    }

    #[test]
    fn test_trigger_matches_second_occurrence() {
        // First "harness" is embedded in "xharness", second is standalone
        assert!(trigger_matches("xharness then harness", "harness"));
    }

    #[test]
    fn test_trigger_matches_multibyte_text() {
        // Ensure no panic on multi-byte UTF-8 text
        assert!(trigger_matches("日本語 harness パターン", "harness"));
        assert!(!trigger_matches("日本語テスト", "harness"));
    }

    #[test]
    fn test_trigger_matches_at_start() {
        assert!(trigger_matches("harness design", "harness"));
    }

    #[test]
    fn test_trigger_matches_at_end() {
        assert!(trigger_matches("design harness", "harness"));
    }

    #[test]
    fn test_trigger_matches_case_insensitive() {
        assert!(trigger_matches("use Harness pattern", "harness"));
    }

    #[test]
    fn test_trigger_matches_no_match() {
        assert!(!trigger_matches("use other pattern", "harness"));
    }

    #[test]
    fn test_trigger_matches_non_alphanumeric_trigger() {
        // Non-alphanumeric triggers use simple contains
        assert!(trigger_matches("test /track:plan feature", "/track:plan"));
    }

    // -- find_matching_guides --

    fn sample_guides() -> Vec<GuideEntry> {
        vec![GuideEntry {
            id: "harness-design".to_owned(),
            trigger_keywords: vec!["harness".to_owned(), "multi-agent".to_owned()],
            summary: vec!["Generator-Evaluator pattern".to_owned()],
            project_usage: vec!["Phase 2 CC-SDD-01".to_owned()],
            cache_path: ".cache/external-guides/harness.md".to_owned(),
        }]
    }

    #[test]
    fn test_find_matching_guides_with_match() {
        let guides = sample_guides();
        let matches = find_matching_guides("design a harness", &guides, 3);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "harness-design");
        assert_eq!(matches[0].trigger, "harness");
    }

    #[test]
    fn test_find_matching_guides_no_match() {
        let guides = sample_guides();
        let matches = find_matching_guides("unrelated prompt", &guides, 3);
        assert!(matches.is_empty());
    }

    #[test]
    fn test_find_matching_guides_respects_limit() {
        let guides = vec![
            GuideEntry {
                id: "a".to_owned(),
                trigger_keywords: vec!["alpha".to_owned()],
                summary: vec![],
                project_usage: vec![],
                cache_path: String::new(),
            },
            GuideEntry {
                id: "b".to_owned(),
                trigger_keywords: vec!["beta".to_owned()],
                summary: vec![],
                project_usage: vec![],
                cache_path: String::new(),
            },
        ];
        let matches = find_matching_guides("alpha beta", &guides, 1);
        assert_eq!(matches.len(), 1);
    }

    // -- check_compliance --

    #[test]
    fn test_check_compliance_with_track_plan_and_guide() {
        let guides = sample_guides();
        let ctx = check_compliance("/track:plan harness-feature", None, &guides, 3);
        assert!(ctx.skill_match.is_some());
        assert_eq!(ctx.guide_matches.len(), 1);
        assert!(!ctx.is_empty());
    }

    #[test]
    fn test_check_compliance_empty_prompt() {
        let ctx = check_compliance("hello", None, &[], 3);
        assert!(ctx.is_empty());
        assert!(ctx.render().is_none());
    }

    // -- render --

    #[test]
    fn test_render_skill_only() {
        let ctx = ComplianceContext {
            skill_match: Some(SkillMatch {
                command: "/track:plan".to_owned(),
                reminders: vec!["Phase 1.5 is mandatory".to_owned()],
            }),
            guide_matches: vec![],
        };
        let rendered = ctx.render().unwrap();
        assert!(rendered.contains("[Skill Compliance]"));
        assert!(rendered.contains("/track:plan"));
        assert!(rendered.contains("Phase 1.5 is mandatory"));
    }

    #[test]
    fn test_render_guide_only() {
        let ctx = ComplianceContext {
            skill_match: None,
            guide_matches: vec![GuideMatch {
                id: "test-guide".to_owned(),
                trigger: "test".to_owned(),
                summary: vec!["A test guide".to_owned()],
                project_usage: vec!["Used in testing".to_owned()],
                cache_path: ".cache/test.md".to_owned(),
            }],
        };
        let rendered = ctx.render().unwrap();
        assert!(rendered.contains("[External Guide Context]"));
        assert!(rendered.contains("test-guide"));
    }

    #[test]
    fn test_render_both() {
        let ctx = ComplianceContext {
            skill_match: Some(SkillMatch {
                command: "/track:review".to_owned(),
                reminders: vec!["Use reviewer capability".to_owned()],
            }),
            guide_matches: vec![GuideMatch {
                id: "g1".to_owned(),
                trigger: "t1".to_owned(),
                summary: vec!["summary".to_owned()],
                project_usage: vec![],
                cache_path: String::new(),
            }],
        };
        let rendered = ctx.render().unwrap();
        assert!(rendered.contains("[Skill Compliance]"));
        assert!(rendered.contains("[External Guide Context]"));
    }
}
