//! Text-pattern verification checks for architecture docs.
//!
//! Rust port of the `_require_file` / `_require_line` checks in
//! `scripts/verify_architecture_docs.py`.

use std::path::Path;

use domain::verify::{Finding, VerifyOutcome};

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

    // Convention docs checks — only when conventions are bootstrapped.
    let conventions_readme = root.join("project-docs").join("conventions").join("README.md");
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
    if root.join(rel_path).is_file() {
        VerifyOutcome::pass()
    } else {
        VerifyOutcome::from_findings(vec![Finding::error(format!(
            "Missing file: {rel_path} ({label})"
        ))])
    }
}

fn require_line(root: &Path, rel_path: &str, pattern: &str, label: &str) -> VerifyOutcome {
    let path = root.join(rel_path);
    if !path.is_file() {
        return VerifyOutcome::from_findings(vec![Finding::error(format!(
            "{rel_path} not found (checking for: {label})"
        ))]);
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => {
            if content.contains(pattern) {
                VerifyOutcome::pass()
            } else {
                VerifyOutcome::from_findings(vec![Finding::error(format!(
                    "Missing in {rel_path}: {label}"
                ))])
            }
        }
        Err(e) => VerifyOutcome::from_findings(vec![Finding::error(format!(
            "Cannot read {rel_path}: {e}"
        ))]),
    }
}

static REQUIRED_FILES: &[RequireFile] = &[
    RequireFile {
        rel_path: "docs/architecture-rules.json",
        label: "architecture rules source of truth",
    },
    RequireFile { rel_path: "scripts/architecture_rules.py", label: "architecture rules helper" },
];

/// Convention-specific required files — only checked when conventions are bootstrapped.
static CONVENTIONS_REQUIRED_FILES: &[RequireFile] = &[RequireFile {
    rel_path: ".claude/commands/conventions/add.md",
    label: "conventions add command",
}];

static REQUIRED_LINES: &[RequireLine] = &[
    // Workspace member references are checked dynamically in architecture_rules module.
    // Workflow gates in track/workflow.md.
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make check-layers` passes",
        label: "workflow quality gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make verify-plan-progress` passes",
        label: "workflow track gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make verify-track-metadata` passes",
        label: "workflow metadata gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make verify-tech-stack` passes",
        label: "workflow tech-stack gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make scripts-selftest` passes",
        label: "workflow scripts selftest gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make hooks-selftest` passes",
        label: "workflow hooks selftest gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make verify-orchestra` passes",
        label: "workflow orchestra gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "`cargo make verify-latest-track` passes",
        label: "workflow latest-track gate",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "/track:revert",
        label: "workflow revert command",
    },
    RequireLine {
        rel_path: "track/workflow.md",
        pattern: "D[Infra Layer] --> C",
        label: "workflow mermaid dependency direction",
    },
    // Traceability markers.
    RequireLine {
        rel_path: "TRACK_TRACEABILITY.md",
        pattern: "Responsibility Split (Fixed)",
        label: "traceability role section",
    },
    RequireLine {
        rel_path: "TRACK_TRACEABILITY.md",
        pattern: "scripts-selftest-local",
        label: "traceability scripts selftest gate",
    },
    RequireLine {
        rel_path: "TRACK_TRACEABILITY.md",
        pattern: "hooks-selftest-local",
        label: "traceability hooks selftest gate",
    },
    RequireLine {
        rel_path: "TRACK_TRACEABILITY.md",
        pattern: "verify-latest-track-local",
        label: "traceability latest-track gate",
    },
    RequireLine {
        rel_path: "TRACK_TRACEABILITY.md",
        pattern: "cargo make ci",
        label: "traceability ci overview",
    },
    // Developer workflow references.
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "cargo make verify-orchestra",
        label: "workflow orchestra guardrail",
    },
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "cargo make verify-track-metadata",
        label: "workflow metadata guardrail",
    },
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "cargo make verify-tech-stack",
        label: "workflow tech-stack guardrail",
    },
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "cargo make verify-latest-track",
        label: "workflow latest-track guardrail",
    },
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "/track:revert",
        label: "developer workflow revert command",
    },
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "cargo make scripts-selftest",
        label: "developer workflow scripts selftest gate",
    },
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "cargo make hooks-selftest",
        label: "developer workflow hooks selftest gate",
    },
];

/// Convention-specific required lines — only checked when conventions are bootstrapped.
static CONVENTIONS_REQUIRED_LINES: &[RequireLine] = &[
    RequireLine {
        rel_path: "CLAUDE.md",
        pattern: "project-docs/conventions/",
        label: "CLAUDE project conventions reference",
    },
    RequireLine {
        rel_path: ".codex/instructions.md",
        pattern: "project-docs/conventions/",
        label: "Codex project conventions reference",
    },
    RequireLine {
        rel_path: "DEVELOPER_AI_WORKFLOW.md",
        pattern: "project-docs/conventions/",
        label: "developer workflow project conventions reference",
    },
    RequireLine {
        rel_path: "docs/README.md",
        pattern: "project-docs/conventions/",
        label: "docs README project conventions reference",
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
}
