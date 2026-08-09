//! Text-pattern verification checks for architecture docs.
//!
//! Rust port of the `_require_file` / `_require_line` checks in
//! `scripts/verify_architecture_docs.py`.

use std::path::Path;

use domain::verify::{VerifyFinding, VerifyOutcome};

use crate::trusted_file::read_bounded_regular_file;

const MAX_DOCUMENT_BYTES: u64 = 1024 * 1024;

/// A single "require file exists" check.
struct RequireFile {
    rel_path: &'static str,
    label: &'static str,
}

/// A single "require line contains pattern" check.
struct RequireLine {
    rel_path: &'static str,
    pattern: &'static str,
    label: &'static str,
}

/// A workflow document whose phase-writer dispatch must stay phase-entry-only.
struct PhaseWriterWorkflowDoc {
    rel_path: &'static str,
}

/// Run all text-pattern checks.
///
/// # Errors
///
/// Returns error findings for missing files or missing text patterns.
pub fn verify(root: &Path) -> VerifyOutcome {
    let mut outcome = VerifyOutcome::pass();

    // Required files (always checked).
    for check in REQUIRED_FILES {
        outcome.merge(require_file(root, check.rel_path, check.label));
    }

    // Required line patterns (always checked).
    for check in REQUIRED_LINES {
        outcome.merge(require_line(root, check.rel_path, check.pattern, check.label));
    }

    for document in PHASE_WRITER_WORKFLOW_DOCS {
        outcome.merge(require_no_direct_phase_writer_dispatch(root, document.rel_path));
    }

    // Convention docs checks — only when conventions are bootstrapped.
    let conventions_readme = root.join("knowledge").join("conventions").join("README.md");
    if conventions_readme.is_file() {
        for check in CONVENTIONS_REQUIRED_FILES {
            outcome.merge(require_file(root, check.rel_path, check.label));
        }
        for check in CONVENTIONS_REQUIRED_LINES {
            outcome.merge(require_line(root, check.rel_path, check.pattern, check.label));
        }
    }

    outcome
}

fn require_file(root: &Path, rel_path: &str, label: &str) -> VerifyOutcome {
    match read_document(root, rel_path) {
        Ok(Some(_)) => VerifyOutcome::pass(),
        Ok(None) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "Missing file: {rel_path} ({label})"
        ))]),
        Err(error) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "Cannot read {rel_path}: {error}"
        ))]),
    }
}

fn require_line(root: &Path, rel_path: &str, pattern: &str, label: &str) -> VerifyOutcome {
    match read_document(root, rel_path) {
        Ok(Some(content)) => {
            if content.contains(pattern) {
                VerifyOutcome::pass()
            } else {
                VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
                    "Missing in {rel_path}: {label}"
                ))])
            }
        }
        Ok(None) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{rel_path} not found (checking for: {label})"
        ))]),
        Err(error) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "Cannot read {rel_path}: {error}"
        ))]),
    }
}

fn require_no_direct_phase_writer_dispatch(root: &Path, rel_path: &str) -> VerifyOutcome {
    match read_document(root, rel_path) {
        Ok(Some(content)) => {
            let normalized_lines = normalized_scanned_lines(&content);
            let findings = normalized_lines
                .iter()
                .filter_map(|line| direct_phase_writer_dispatch_pattern(line))
                .map(|pattern| {
                    VerifyFinding::error(format!(
                        "Direct phase-writer dispatch in {rel_path}: {pattern}; use bin/sotp phase enter <phase-id>"
                    ))
                })
                .collect();
            VerifyOutcome::from_findings(findings)
        }
        Ok(None) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "{rel_path} not found (checking for direct phase-writer dispatch)"
        ))]),
        Err(error) => VerifyOutcome::from_findings(vec![VerifyFinding::error(format!(
            "Cannot read {rel_path}: {error}"
        ))]),
    }
}

fn direct_phase_writer_dispatch_pattern(line: &str) -> Option<&'static str> {
    let line = line.to_ascii_lowercase();
    if let Some(pattern) =
        DIRECT_PHASE_WRITER_DISPATCH_PATTERNS.iter().find(|pattern| line.contains(**pattern))
    {
        return Some(pattern);
    }

    DIRECT_PHASE_WRITERS.iter().find(|writer| prose_phase_writer_invocation(&line, writer)).copied()
}

fn prose_phase_writer_invocation(line: &str, writer: &str) -> bool {
    line.match_indices("invoke").any(|(offset, _)| {
        if invocation_is_negated(line, offset) {
            return false;
        }

        let mut remaining = line[offset + "invoke".len()..].trim_start();
        for article in ["the ", "a "] {
            if let Some(without_article) = remaining.strip_prefix(article) {
                remaining = without_article;
                break;
            }
        }
        remaining = remaining.strip_prefix('`').unwrap_or(remaining);
        let Some(after_writer) = remaining.strip_prefix(writer) else {
            return false;
        };
        after_writer
            .chars()
            .next()
            .is_none_or(|character| !character.is_ascii_alphanumeric() && character != '-')
    })
}

fn invocation_is_negated(line: &str, invocation_offset: usize) -> bool {
    let mut words = line[..invocation_offset].split_whitespace().collect::<Vec<_>>();
    while words.last().is_some_and(|word| invocation_modifier(word)) {
        words.pop();
    }

    words.last() == Some(&"never")
        || words.ends_with(&["do", "not"])
        || words.ends_with(&["must", "not"])
}

fn invocation_modifier(word: &str) -> bool {
    word == "ever" || word.ends_with("ly")
}

fn normalized_scanned_lines(content: &str) -> Vec<String> {
    let mut normalized_lines = Vec::new();
    let mut continued_line = String::new();

    for line in content.lines() {
        let line = line.trim_end();
        let (line, continues) = match line.strip_suffix('\\') {
            Some(line) => (line, true),
            None => (line, false),
        };
        if !continued_line.is_empty() {
            continued_line.push(' ');
        }
        continued_line.push_str(line);

        if !continues {
            normalized_lines.push(continued_line.split_whitespace().collect::<Vec<_>>().join(" "));
            continued_line.clear();
        }
    }

    if !continued_line.is_empty() {
        normalized_lines.push(continued_line.split_whitespace().collect::<Vec<_>>().join(" "));
    }

    normalized_lines
}

fn read_document(root: &Path, rel_path: &str) -> std::io::Result<Option<String>> {
    read_bounded_regular_file(&root.join(rel_path), root, MAX_DOCUMENT_BYTES)
}

static REQUIRED_FILES: &[RequireFile] = &[RequireFile {
    rel_path: "architecture-rules.json",
    label: "architecture rules source of truth",
}];

/// Convention-specific required files — only checked when conventions are bootstrapped.
static CONVENTIONS_REQUIRED_FILES: &[RequireFile] = &[RequireFile {
    rel_path: ".claude/commands/conventions/add.md",
    label: "conventions add command",
}];

static REQUIRED_LINES: &[RequireLine] = &[
    // Workspace member references are checked dynamically in architecture_rules module.
];

static DIRECT_PHASE_WRITER_DISPATCH_PATTERNS: &[&str] = &[
    "bin/sotp capability exec spec-designer",
    "bin/sotp capability exec type-designer",
    "bin/sotp capability exec impl-planner",
    "bin/sotp capability exec <capability>",
];

static DIRECT_PHASE_WRITERS: &[&str] = &["spec-designer", "type-designer", "impl-planner"];

static PHASE_WRITER_WORKFLOW_DOCS: &[PhaseWriterWorkflowDoc] = &[
    PhaseWriterWorkflowDoc { rel_path: ".harness/workflows/track/plan.md" },
    PhaseWriterWorkflowDoc { rel_path: ".harness/workflows/track/spec-design.md" },
    PhaseWriterWorkflowDoc { rel_path: ".harness/workflows/track/type-design.md" },
    PhaseWriterWorkflowDoc { rel_path: ".harness/workflows/track/impl-plan.md" },
    PhaseWriterWorkflowDoc { rel_path: ".harness/workflows/track/adr2pr.md" },
    PhaseWriterWorkflowDoc { rel_path: ".claude/commands/track/plan.md" },
    PhaseWriterWorkflowDoc { rel_path: ".claude/commands/track/spec-design.md" },
    PhaseWriterWorkflowDoc { rel_path: ".claude/commands/track/type-design.md" },
    PhaseWriterWorkflowDoc { rel_path: ".claude/commands/track/impl-plan.md" },
    PhaseWriterWorkflowDoc { rel_path: ".claude/commands/track/adr2pr.md" },
    PhaseWriterWorkflowDoc { rel_path: ".agents/skills/track-plan/SKILL.md" },
    PhaseWriterWorkflowDoc { rel_path: ".agents/skills/track-spec-design/SKILL.md" },
    PhaseWriterWorkflowDoc { rel_path: ".agents/skills/track-type-design/SKILL.md" },
    PhaseWriterWorkflowDoc { rel_path: ".agents/skills/track-impl-plan/SKILL.md" },
    PhaseWriterWorkflowDoc { rel_path: ".agents/skills/track-adr2pr/SKILL.md" },
];

/// Convention-specific required lines — only checked when conventions are bootstrapped.
static CONVENTIONS_REQUIRED_LINES: &[RequireLine] = &[
    RequireLine {
        rel_path: "CLAUDE.md",
        pattern: "knowledge/conventions/",
        label: "CLAUDE project conventions reference",
    },
    RequireLine {
        rel_path: ".codex/instructions.md",
        pattern: "knowledge/conventions/",
        label: "Codex project conventions reference",
    },
    RequireLine {
        rel_path: "README.md",
        pattern: "knowledge/conventions/",
        label: "README project conventions reference",
    },
];

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use tempfile::TempDir;

    use super::*;

    fn write_file(root: &Path, rel: &str, content: &str) {
        let path = root.join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, content).unwrap();
    }

    #[test]
    fn test_require_file_passes_when_exists() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "test.txt", "content");
        let outcome = require_file(tmp.path(), "test.txt", "test file");
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_require_file_fails_when_missing() {
        let tmp = TempDir::new().unwrap();
        let outcome = require_file(tmp.path(), "missing.txt", "test file");
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_line_passes_when_pattern_found() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "test.md", "line with pattern here");
        let outcome = require_line(tmp.path(), "test.md", "pattern", "test label");
        assert!(outcome.is_ok());
    }

    #[test]
    fn test_require_line_fails_when_pattern_missing() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "test.md", "no match here");
        let outcome = require_line(tmp.path(), "test.md", "pattern", "test label");
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_line_fails_when_file_missing() {
        let tmp = TempDir::new().unwrap();
        let outcome = require_line(tmp.path(), "missing.md", "pattern", "test label");
        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_passes_for_phase_entry() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "workflow.md",
            "run bin/sotp phase enter spec-design for the configured writer",
        );

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.is_ok());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_fails_for_direct_writer() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "workflow.md",
            "run bin/sotp capability exec spec-designer --briefing-file tmp/spec.md",
        );

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_fails_for_prose_direct_writer() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "workflow.md",
            "catalogue-side → invoke `type-designer`, then re-run semantic review",
        );

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_fails_for_case_article_and_backtick_variants() {
        let tmp = TempDir::new().unwrap();

        for (index, content) in [
            "Invoke the `spec-designer` capability",
            "Invoke spec-designer capability",
            "INVOKE THE `TYPE-DESIGNER` CAPABILITY",
            "INVOKE IMPL-PLANNER CAPABILITY",
        ]
        .iter()
        .enumerate()
        {
            let rel_path = format!("workflow-{index}.md");
            write_file(tmp.path(), &rel_path, content);

            let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), &rel_path);

            assert!(outcome.has_errors(), "expected direct-writer detection for: {content}");
        }
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_passes_for_prohibition_phrasing() {
        let tmp = TempDir::new().unwrap();

        for (index, content) in [
            "Do not invoke the `spec-designer` directly; use phase entry.",
            "Never invoke type-designer capability outside phase entry.",
            "You must not directly invoke a `impl-planner` capability.",
        ]
        .iter()
        .enumerate()
        {
            let rel_path = format!("workflow-{index}.md");
            write_file(tmp.path(), &rel_path, content);

            let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), &rel_path);

            assert!(outcome.is_ok(), "expected prohibition to pass: {content}");
        }
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_fails_when_unrelated_action_is_negated() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "workflow.md",
            "Do not skip briefing validation when you invoke `spec-designer` directly.",
        );

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_double_space_direct_writer_returns_error() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "workflow.md",
            "run bin/sotp capability  exec spec-designer --briefing-file tmp/spec.md",
        );

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_tab_direct_writer_returns_error() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "workflow.md",
            "run bin/sotp capability\texec spec-designer --briefing-file tmp/spec.md",
        );

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_continued_direct_writer_returns_error() {
        let tmp = TempDir::new().unwrap();
        write_file(
            tmp.path(),
            "workflow.md",
            "run bin/sotp capability \\\n             exec spec-designer --briefing-file tmp/spec.md",
        );

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn test_require_no_direct_phase_writer_dispatch_symlinked_document_returns_error() {
        let tmp = TempDir::new().unwrap();
        write_file(tmp.path(), "target.md", "run bin/sotp phase enter spec-design");
        std::os::unix::fs::symlink("target.md", tmp.path().join("workflow.md")).unwrap();

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }

    #[test]
    fn test_require_no_direct_phase_writer_dispatch_oversized_document_returns_error() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("workflow.md");
        std::fs::File::create(&path)
            .unwrap()
            .set_len(MAX_DOCUMENT_BYTES.saturating_add(1))
            .unwrap();

        let outcome = require_no_direct_phase_writer_dispatch(tmp.path(), "workflow.md");

        assert!(outcome.has_errors());
    }
}
