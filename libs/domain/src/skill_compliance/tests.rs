#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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
    let result = detect_skill_command("do /track:review then /track:implement");
    assert!(result.is_some());
    assert_eq!(result.unwrap().command, "/track:review");
}

#[test]
fn test_detect_skill_command_longest_match_at_same_position() {
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

#[test]
fn test_detect_skill_command_rejects_unknown_suffix() {
    let result = detect_skill_command("/track:planner something");
    assert!(result.is_none());
}

#[test]
fn test_detect_skill_command_accepts_space_after_command() {
    let result = detect_skill_command("/track:plan my-feature");
    assert!(result.is_some());
    assert_eq!(result.unwrap().command, "/track:plan");
}

#[test]
fn test_detect_skill_command_accepts_end_of_string() {
    let result = detect_skill_command("/track:plan");
    assert!(result.is_some());
    assert_eq!(result.unwrap().command, "/track:plan");
}

// -- trigger_matches --

#[test]
fn test_trigger_matches_word_boundary() {
    assert!(trigger_matches("use harness pattern", "harness"));
    assert!(!trigger_matches("use harnessing pattern", "harness"));
}

#[test]
fn test_trigger_matches_second_occurrence() {
    assert!(trigger_matches("xharness then harness", "harness"));
}

#[test]
fn test_trigger_matches_multibyte_text() {
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
