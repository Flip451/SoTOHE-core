#![allow(clippy::indexing_slicing, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use super::*;

// -- detect_skill_command --

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
    let result = detect_skill_command("/Track:Review my-feature");
    assert!(result.is_some());
    assert_eq!(result.unwrap().command, "/track:review");
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
fn test_detect_skill_command_commit() {
    let result = detect_skill_command("/track:commit fix things");
    assert!(result.is_some());
    assert_eq!(result.unwrap().command, "/track:commit");
}

#[test]
fn test_detect_skill_command_rejects_unknown_suffix() {
    let result = detect_skill_command("/track:reviewer something");
    assert!(result.is_none());
}

#[test]
fn test_detect_skill_command_skips_unknown_suffix_before_valid_command() {
    let result = detect_skill_command("/track:reviewer then /track:review feature");
    assert_eq!(result.map(|skill| skill.command), Some("/track:review".to_owned()));
}

#[test]
fn test_detect_skill_command_accepts_space_after_command() {
    let result = detect_skill_command("/track:review my-feature");
    assert!(result.is_some());
    assert_eq!(result.unwrap().command, "/track:review");
}

#[test]
fn test_detect_skill_command_accepts_end_of_string() {
    let result = detect_skill_command("/track:review");
    assert!(result.is_some());
    assert_eq!(result.unwrap().command, "/track:review");
}

#[test]
fn test_detect_skill_command_ignores_removed_track_plan_skill() {
    assert!(detect_skill_command("/track:plan feature").is_none());
}

// -- check_compliance --

#[test]
fn test_check_compliance_empty_prompt() {
    let ctx = check_compliance("hello");
    assert!(ctx.is_empty());
    assert!(ctx.render().is_none());
}

#[test]
fn test_check_compliance_with_known_command() {
    let ctx = check_compliance("/track:review");
    assert!(ctx.skill_match.is_some());
    assert!(!ctx.is_empty());
}

// -- render --

#[test]
fn test_render_skill_only() {
    let ctx = ComplianceContext {
        skill_match: Some(SkillMatch {
            command: "/track:review".to_owned(),
            reminders: vec!["Phase 1.5 is mandatory".to_owned()],
        }),
    };
    let rendered = ctx.render().unwrap();
    assert!(rendered.contains("[Skill Compliance]"));
    assert!(rendered.contains("/track:review"));
    assert!(rendered.contains("Phase 1.5 is mandatory"));
}
