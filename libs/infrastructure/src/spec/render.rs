//! Renders `spec.md` from a [`SpecDocument`].
//!
//! `spec.md` is a read-only rendered view generated from `spec.json` (the SSoT).
//! The first line of the rendered output is a machine-readable comment that marks
//! the file as generated, preventing accidental direct edits.

use domain::{SpecDocument, SpecRequirement};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Renders the contents of `spec.md` from a [`SpecDocument`].
///
/// The output always ends with a trailing newline.
///
/// # Format
///
/// ```text
/// <!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
/// ---
/// status: draft
/// version: "1.0"
/// signals: { blue: 15, yellow: 0, red: 0 }
/// ---
///
/// # Feature Title
///
/// ## Goal
///
/// Goal paragraph line 1
///
/// ## Scope
///
/// ### In Scope
/// - Requirement text [source: PRD §3.2]
///
/// ### Out of Scope
/// - Excluded item [source: inference — not needed]
///
/// ## Constraints
/// - Constraint 1 [source: convention — hex.md]
///
/// ## Domain States
///
/// | State | Description |
/// |-------|-------------|
/// | Draft | Initial state |
///
/// ## Acceptance Criteria
/// - [ ] AC text [source: PRD §4.1]
///
/// ## Custom Section Title
///
/// Free-form line 1
///
/// ## Related Conventions (Required Reading)
/// - knowledge/conventions/source-attribution.md
/// ```
#[must_use]
pub fn render_spec(doc: &SpecDocument) -> String {
    let mut out = String::new();

    // Header comment
    out.push_str("<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->\n");

    // YAML frontmatter
    out.push_str("---\n");
    out.push_str(&format!("status: {}\n", doc.status()));
    if let Some(ts) = doc.approved_at() {
        out.push_str(&format!("approved_at: \"{}\"\n", ts));
    }
    out.push_str(&format!("version: \"{}\"\n", doc.version()));
    if let Some(signals) = doc.signals() {
        out.push_str(&format!(
            "signals: {{ blue: {}, yellow: {}, red: {} }}\n",
            signals.blue(),
            signals.yellow(),
            signals.red()
        ));
    }
    out.push_str("---\n");
    out.push('\n');

    // Title
    out.push_str(&format!("# {}\n", doc.title()));
    out.push('\n');

    // Goal
    let goal = doc.goal();
    if !goal.is_empty() {
        out.push_str("## Goal\n");
        out.push('\n');
        for line in goal {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }

    // Scope
    {
        let scope = doc.scope();
        out.push_str("## Scope\n");
        out.push('\n');

        out.push_str("### In Scope\n");
        for req in scope.in_scope() {
            out.push_str(&render_requirement(req));
        }
        out.push('\n');

        if !scope.out_of_scope().is_empty() {
            out.push_str("### Out of Scope\n");
            for req in scope.out_of_scope() {
                out.push_str(&render_requirement(req));
            }
            out.push('\n');
        }
    }

    // Constraints
    let constraints = doc.constraints();
    if !constraints.is_empty() {
        out.push_str("## Constraints\n");
        for req in constraints {
            out.push_str(&render_requirement(req));
        }
        out.push('\n');
    }

    // Acceptance Criteria
    let ac = doc.acceptance_criteria();
    if !ac.is_empty() {
        out.push_str("## Acceptance Criteria\n");
        for req in ac {
            out.push_str(&render_acceptance_criterion(req));
        }
        out.push('\n');
    }

    // Additional sections
    for section in doc.additional_sections() {
        out.push_str(&format!("## {}\n", section.title()));
        out.push('\n');
        for line in section.content() {
            out.push_str(line);
            out.push('\n');
        }
        out.push('\n');
    }

    // Related Conventions
    let conventions = doc.related_conventions();
    if !conventions.is_empty() {
        out.push_str("## Related Conventions (Required Reading)\n");
        for path in conventions {
            out.push_str(&format!("- {path}\n"));
        }
        out.push('\n');
    }

    // Signal Summary (Stage 1 + Stage 2) — appended after all content sections.
    let summary = render_signal_summary(doc);
    out.push_str(&summary);

    // Hearing History (TSUMIKI-07) — last 5 entries, most recent first.
    let history = doc.hearing_history();
    if !history.is_empty() {
        out.push_str("## Hearing History\n");
        out.push('\n');
        out.push_str("| Date | Mode | Questions | Added | Modified |\n");
        out.push_str("|------|------|-----------|-------|----------|\n");
        for record in history.iter().rev().take(5) {
            out.push_str(&format!(
                "| {} | {} | {} | {} | {} |\n",
                record.date(),
                record.mode().as_str(),
                record.questions_asked(),
                record.items_added(),
                record.items_modified(),
            ));
        }
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

/// Renders a requirement as a bullet item with optional source and task_refs annotations.
///
/// Single source:  `- text [source: tag]`
/// Multiple:       `- text [source: tag1, tag2]`
/// With tasks:     `- text [source: tag] [tasks: T001, T002]`
/// No sources:     `- text`
fn render_requirement(req: &SpecRequirement) -> String {
    let mut line = format!("- {}", req.text());
    append_source_tag(&mut line, req);
    append_task_refs_tag(&mut line, req);
    line.push('\n');
    line
}

/// Renders an acceptance criterion as a checkbox bullet item.
///
/// Format: `- [ ] text [source: tag] [tasks: T001]`
fn render_acceptance_criterion(req: &SpecRequirement) -> String {
    let mut line = format!("- [ ] {}", req.text());
    append_source_tag(&mut line, req);
    append_task_refs_tag(&mut line, req);
    line.push('\n');
    line
}

fn append_source_tag(line: &mut String, req: &SpecRequirement) {
    let sources = req.sources();
    if !sources.is_empty() {
        line.push_str(&format!(" [source: {}]", sources.join(", ")));
    }
}

fn append_task_refs_tag(line: &mut String, req: &SpecRequirement) {
    let task_refs = req.task_refs();
    if !task_refs.is_empty() {
        let refs: Vec<&str> = task_refs.iter().map(|id| id.as_ref()).collect();
        line.push_str(&format!(" [tasks: {}]", refs.join(", ")));
    }
}

/// Renders the Signal Summary section for a spec document.
///
/// Produces a `## Signal Summary` markdown section with:
/// - `### Stage 1: Spec Signals` — when `doc.signals()` is `Some`
///
/// Returns an empty string when no signals have been evaluated yet.
#[must_use]
pub fn render_signal_summary(doc: &SpecDocument) -> String {
    let stage1 = doc.signals();

    if stage1.is_none() {
        return String::new();
    }

    let mut out = String::new();
    out.push_str("## Signal Summary\n");
    out.push('\n');

    if let Some(counts) = stage1 {
        out.push_str("### Stage 1: Spec Signals\n");
        out.push_str(&format!(
            "\u{1f535} {}  \u{1f7e1} {}  \u{1f534} {}\n",
            counts.blue(),
            counts.yellow(),
            counts.red()
        ));
        out.push('\n');
    }

    out
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use domain::{SignalCounts, SpecDocument, SpecRequirement, SpecScope, SpecSection};

    use super::*;

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn req(text: &str, sources: &[&str]) -> SpecRequirement {
        SpecRequirement::new(text, sources.iter().map(|s| s.to_string()).collect()).unwrap()
    }

    fn make_minimal_doc() -> SpecDocument {
        SpecDocument::new(
            "Feature X",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap()
    }

    fn make_full_doc() -> SpecDocument {
        SpecDocument::new(
            "Feature Title",
            domain::SpecStatus::Draft,
            "1.0",
            vec!["Goal paragraph line 1".into()],
            SpecScope::new(
                vec![req("Requirement text", &["PRD §3.2"])],
                vec![req("Excluded item", &["inference — not needed"])],
            ),
            vec![req("Constraint 1", &["convention — hex.md"])],
            vec![req("AC text", &["PRD §4.1"])],
            vec![
                SpecSection::new("Custom Section Title", vec!["Free-form line 1".into()]).unwrap(),
            ],
            vec!["knowledge/conventions/source-attribution.md".into()],
            Some(SignalCounts::new(15, 0, 0)),
            None,
            None,
        )
        .unwrap()
    }

    // ---------------------------------------------------------------------------
    // render_spec: header and frontmatter
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_first_line_is_generated_comment() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        let first_line = output.lines().next().unwrap();
        assert_eq!(first_line, "<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->");
    }

    #[test]
    fn test_render_spec_frontmatter_contains_status_and_version() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(output.contains("status: draft\n"));
        assert!(output.contains("version: \"1.0\"\n"));
    }

    #[test]
    fn test_render_spec_signals_present_when_set() {
        let mut doc = make_minimal_doc();
        doc.set_signals(SignalCounts::new(15, 0, 0));
        let output = render_spec(&doc);
        assert!(output.contains("signals: { blue: 15, yellow: 0, red: 0 }\n"));
    }

    #[test]
    fn test_render_spec_signals_absent_when_none() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("signals:"));
    }

    #[test]
    fn test_render_spec_frontmatter_delimited_by_triple_dashes() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        // Should have exactly two "---" lines (open and close of frontmatter)
        let dash_count = output.lines().filter(|l| *l == "---").count();
        assert_eq!(dash_count, 2);
    }

    // ---------------------------------------------------------------------------
    // render_spec: title
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_title_rendered_as_h1() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(output.contains("# Feature X\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: goal section
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_goal_section_rendered_when_present() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Goal\n"));
        assert!(output.contains("Goal paragraph line 1\n"));
    }

    #[test]
    fn test_render_spec_goal_section_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("## Goal\n"));
    }

    #[test]
    fn test_render_spec_goal_multiple_lines_each_on_own_line() {
        let doc = SpecDocument::new(
            "F",
            domain::SpecStatus::Draft,
            "1.0",
            vec!["Line A".into(), "Line B".into()],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("Line A\nLine B\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: scope section
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_scope_section_always_rendered() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Scope\n"));
        assert!(output.contains("### In Scope\n"));
    }

    #[test]
    fn test_render_spec_in_scope_requirement_with_single_source() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("- Requirement text [source: PRD §3.2]\n"));
    }

    #[test]
    fn test_render_spec_out_of_scope_rendered_when_present() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("### Out of Scope\n"));
        assert!(output.contains("- Excluded item [source: inference — not needed]\n"));
    }

    #[test]
    fn test_render_spec_out_of_scope_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("### Out of Scope\n"));
    }

    #[test]
    fn test_render_spec_requirement_with_no_sources_has_no_source_tag() {
        let doc = SpecDocument::new(
            "F",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![req("bare item", &[])], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("- bare item\n"));
        assert!(!output.contains("[source:"));
    }

    #[test]
    fn test_render_spec_requirement_with_multiple_sources_joined_by_comma() {
        let doc = SpecDocument::new(
            "F",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![req("multi", &["PRD §1", "discussion"])], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("- multi [source: PRD §1, discussion]\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: constraints
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_constraints_rendered_when_present() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Constraints\n"));
        assert!(output.contains("- Constraint 1 [source: convention — hex.md]\n"));
    }

    #[test]
    fn test_render_spec_constraints_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("## Constraints\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: acceptance criteria
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_acceptance_criteria_rendered_as_checkboxes() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Acceptance Criteria\n"));
        assert!(output.contains("- [ ] AC text [source: PRD §4.1]\n"));
    }

    #[test]
    fn test_render_spec_acceptance_criteria_no_source_has_no_tag() {
        let doc = SpecDocument::new(
            "F",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![req("plain AC", &[])],
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("- [ ] plain AC\n"));
        assert!(!output.contains("[source:"));
    }

    #[test]
    fn test_render_spec_acceptance_criteria_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("## Acceptance Criteria\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: additional sections
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_additional_section_rendered_with_h2_and_content() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Custom Section Title\n"));
        assert!(output.contains("Free-form line 1\n"));
    }

    #[test]
    fn test_render_spec_additional_sections_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        // Should only have fixed known sections, no extra ## headers from additional_sections
        let h2_count = output.lines().filter(|l| l.starts_with("## ")).count();
        // minimal doc: ## Scope only
        assert_eq!(h2_count, 1, "expected only ## Scope, got:\n{output}");
    }

    #[test]
    fn test_render_spec_multiple_additional_sections_all_rendered() {
        let doc = SpecDocument::new(
            "F",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![
                SpecSection::new("Alpha", vec!["line alpha".into()]).unwrap(),
                SpecSection::new("Beta", vec!["line beta".into()]).unwrap(),
            ],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("## Alpha\n"));
        assert!(output.contains("line alpha\n"));
        assert!(output.contains("## Beta\n"));
        assert!(output.contains("line beta\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: related conventions
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_related_conventions_rendered_when_present() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Related Conventions (Required Reading)\n"));
        assert!(output.contains("- knowledge/conventions/source-attribution.md\n"));
    }

    #[test]
    fn test_render_spec_related_conventions_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("## Related Conventions"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: trailing newline and full exact output
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_output_ends_with_trailing_newline() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(output.ends_with('\n'), "output must end with trailing newline");
    }

    #[test]
    fn test_render_spec_full_doc_exact_output() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        // make_full_doc() sets signals = Some(SignalCounts::new(15, 0, 0)) so
        // render_spec() appends the Stage 1 Signal Summary block at the end.
        let expected = "\
<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: \"1.0\"
signals: { blue: 15, yellow: 0, red: 0 }
---

# Feature Title

## Goal

Goal paragraph line 1

## Scope

### In Scope
- Requirement text [source: PRD §3.2]

### Out of Scope
- Excluded item [source: inference — not needed]

## Constraints
- Constraint 1 [source: convention — hex.md]

## Acceptance Criteria
- [ ] AC text [source: PRD §4.1]

## Custom Section Title

Free-form line 1

## Related Conventions (Required Reading)
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
\u{1f535} 15  \u{1f7e1} 0  \u{1f534} 0

";
        assert_eq!(output, expected);
    }

    #[test]
    fn test_render_spec_minimal_doc_exact_output() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        let expected = "\
<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: \"1.0\"
---

# Feature X

## Scope

### In Scope

";
        assert_eq!(output, expected);
    }

    // ---------------------------------------------------------------------------
    // render_spec: signals with non-zero yellow/red
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_signals_with_yellow_and_red() {
        let mut doc = make_minimal_doc();
        doc.set_signals(SignalCounts::new(3, 2, 1));
        let output = render_spec(&doc);
        assert!(output.contains("signals: { blue: 3, yellow: 2, red: 1 }\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: ordering — sections appear in defined order
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_section_order_is_canonical() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        let positions: Vec<usize> = [
            "## Goal",
            "## Scope",
            "### In Scope",
            "### Out of Scope",
            "## Constraints",
            "## Acceptance Criteria",
            "## Custom Section Title",
            "## Related Conventions",
        ]
        .iter()
        .map(|marker| output.find(marker).unwrap_or(usize::MAX))
        .collect();

        for window in positions.windows(2) {
            assert!(
                window.first().copied().unwrap_or(0) < window.get(1).copied().unwrap_or(0),
                "sections are out of order in rendered output"
            );
        }
    }

    // ---------------------------------------------------------------------------
    // render_signal_summary tests
    // ---------------------------------------------------------------------------

    fn make_doc_stage1_only() -> SpecDocument {
        let mut doc = SpecDocument::new(
            "Feature X",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        doc.set_signals(SignalCounts::new(12, 1, 0));
        doc
    }

    #[test]
    fn test_render_signal_summary_empty_when_no_signals() {
        let doc = SpecDocument::new(
            "Feature X",
            domain::SpecStatus::Draft,
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
            None,
            None,
        )
        .unwrap();
        let output = render_signal_summary(&doc);
        assert_eq!(output, "");
    }

    #[test]
    fn test_render_signal_summary_stage1_only() {
        let doc = make_doc_stage1_only();
        let output = render_signal_summary(&doc);
        assert!(output.contains("## Signal Summary\n"), "missing header");
        assert!(output.contains("### Stage 1: Spec Signals\n"), "missing stage1 header");
        assert!(output.contains("\u{1f535} 12"), "missing blue count");
        assert!(output.contains("\u{1f7e1} 1"), "missing yellow count");
        assert!(output.contains("\u{1f534} 0"), "missing red count");
        assert!(!output.contains("Stage 2"), "stage2 should be absent");
    }

    #[test]
    fn test_render_signal_summary_output_ends_with_trailing_newline() {
        let doc = make_doc_stage1_only();
        let output = render_signal_summary(&doc);
        assert!(output.ends_with('\n'), "output must end with trailing newline");
    }

    // --- task_refs rendering ---

    #[test]
    fn test_render_requirement_with_task_refs() {
        let req = SpecRequirement::with_task_refs(
            "Enable feature",
            vec!["PRD §1".into()],
            vec![
                domain::TaskId::try_new("T001").unwrap(),
                domain::TaskId::try_new("T002").unwrap(),
            ],
        )
        .unwrap();
        let line = render_requirement(&req);
        assert_eq!(line, "- Enable feature [source: PRD §1] [tasks: T001, T002]\n");
    }

    #[test]
    fn test_render_requirement_without_task_refs() {
        let req = SpecRequirement::new("Enable feature", vec!["PRD §1".into()]).unwrap();
        let line = render_requirement(&req);
        assert_eq!(line, "- Enable feature [source: PRD §1]\n");
        assert!(!line.contains("[tasks:"));
    }

    #[test]
    fn test_render_acceptance_criterion_with_task_refs() {
        let req = SpecRequirement::with_task_refs(
            "AC item",
            vec!["discussion".into()],
            vec![domain::TaskId::try_new("T003").unwrap()],
        )
        .unwrap();
        let line = render_acceptance_criterion(&req);
        assert_eq!(line, "- [ ] AC item [source: discussion] [tasks: T003]\n");
    }
}
