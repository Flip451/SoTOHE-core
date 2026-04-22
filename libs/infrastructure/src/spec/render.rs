//! Renders `spec.md` from a [`SpecDocument`].
//!
//! `spec.md` is a read-only rendered view generated from `spec.json` (the SSoT).
//! The first line of the rendered output is a machine-readable comment that marks
//! the file as generated, preventing accidental direct edits.
//!
//! ADR 2026-04-19-1242 §D1.2: `status` / `approved_at` removed from frontmatter;
//! `goal` items rendered as bullet requirements; `related_conventions` rendered as
//! structured refs; requirement annotations show typed refs (adr_refs /
//! convention_refs / informal_grounds) instead of the legacy `sources: Vec<String>`.

use domain::{SpecDocument, SpecRequirement, TaskCoverageDocument, TaskId};

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Renders the contents of `spec.md` from a [`SpecDocument`].
///
/// This is the backward-compatible entry point: requirements are rendered
/// without task-coverage annotations. Use [`render_spec_with_coverage`] when
/// a sibling `task-coverage.json` is available.
///
/// The output always ends with a trailing newline.
#[must_use]
pub fn render_spec(doc: &SpecDocument) -> String {
    render_spec_with_coverage(doc, None)
}

/// Renders the contents of `spec.md` from a [`SpecDocument`] and an optional
/// [`TaskCoverageDocument`].
///
/// When `coverage` is `Some`, each requirement in the `in_scope`,
/// `out_of_scope`, `constraints`, and `acceptance_criteria` sections is
/// annotated with its implementing task IDs from `task-coverage.json`.
/// When `coverage` is `None`, requirements are rendered without coverage
/// annotations (backward-compatible with pre-T004 tracks).
///
/// The output always ends with a trailing newline.
///
/// # Format
///
/// ```text
/// <!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
/// ---
/// version: "1.0"
/// signals: { blue: 15, yellow: 0, red: 0 }
/// ---
///
/// # Feature Title
///
/// ## Goal
///
/// - Goal item [adr: knowledge/adr/x.md#D1.2]
///
/// ## Scope
///
/// ### In Scope
/// - [IN-01] Requirement text [adr: knowledge/adr/x.md#D1.2] [tasks: T001, T002]
///
/// ### Out of Scope
/// - [OO-01] Excluded item [informal: discussion — agreed out of scope]
///
/// ## Constraints
/// - [CN-01] Constraint 1 [conv: .claude/rules/04-coding-principles.md#newtype-pattern]
///
/// ## Acceptance Criteria
/// - [ ] [AC-01] AC text [adr: knowledge/adr/x.md#D3.1] [tasks: T003]
///
/// ## Custom Section Title
///
/// Free-form line 1
///
/// ## Related Conventions (Required Reading)
/// - knowledge/conventions/source-attribution.md#intro
/// ```
#[must_use]
pub fn render_spec_with_coverage(
    doc: &SpecDocument,
    coverage: Option<&TaskCoverageDocument>,
) -> String {
    let mut out = String::new();

    // Header comment
    out.push_str("<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->\n");

    // YAML frontmatter — no status/approved_at in schema v2
    out.push_str("---\n");
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

    // Goal (now Vec<SpecRequirement>)
    let goal = doc.goal();
    if !goal.is_empty() {
        out.push_str("## Goal\n");
        out.push('\n');
        for req in goal {
            out.push_str(&render_requirement_with_tasks(req, None));
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
            let task_refs = coverage.and_then(|c| c.in_scope().get(req.id()));
            out.push_str(&render_requirement_with_tasks(req, task_refs));
        }
        out.push('\n');

        if !scope.out_of_scope().is_empty() {
            out.push_str("### Out of Scope\n");
            for req in scope.out_of_scope() {
                let task_refs = coverage.and_then(|c| c.out_of_scope().get(req.id()));
                out.push_str(&render_requirement_with_tasks(req, task_refs));
            }
            out.push('\n');
        }
    }

    // Constraints
    let constraints = doc.constraints();
    if !constraints.is_empty() {
        out.push_str("## Constraints\n");
        for req in constraints {
            let task_refs = coverage.and_then(|c| c.constraints().get(req.id()));
            out.push_str(&render_requirement_with_tasks(req, task_refs));
        }
        out.push('\n');
    }

    // Acceptance Criteria
    let ac = doc.acceptance_criteria();
    if !ac.is_empty() {
        out.push_str("## Acceptance Criteria\n");
        for req in ac {
            let task_refs = coverage.and_then(|c| c.acceptance_criteria().get(req.id()));
            out.push_str(&render_acceptance_criterion_with_tasks(req, task_refs));
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

    // Related Conventions — now Vec<ConventionRef>
    let conventions = doc.related_conventions();
    if !conventions.is_empty() {
        out.push_str("## Related Conventions (Required Reading)\n");
        for conv in conventions {
            let display = format!("{}#{}", conv.file.to_string_lossy(), conv.anchor.as_ref());
            out.push_str(&format!("- {display}\n"));
        }
        out.push('\n');
    }

    // Signal Summary — appended after all content sections.
    let summary = render_signal_summary(doc);
    out.push_str(&summary);

    // Hearing History — last 5 entries, most recent first.
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

/// Renders a requirement as a bullet item with typed ref annotations and optional
/// task coverage tags.
///
/// Format:
/// - `- [id] text [adr: file#anchor, ...] [conv: file#anchor, ...] [informal: kind — summary, ...] [tasks: T001, T002]`
/// - When `task_refs` is `None` or empty: no `[tasks: ...]` tag.
fn render_requirement_with_tasks(req: &SpecRequirement, task_refs: Option<&Vec<TaskId>>) -> String {
    let mut line = format!("- [{}] {}", req.id(), req.text());
    append_typed_refs(&mut line, req);
    append_task_refs(&mut line, task_refs);
    line.push('\n');
    line
}

/// Renders an acceptance criterion as a checkbox bullet item with optional task coverage.
///
/// Format: `- [ ] [id] text [adr: file#anchor] [tasks: T001]`
fn render_acceptance_criterion_with_tasks(
    req: &SpecRequirement,
    task_refs: Option<&Vec<TaskId>>,
) -> String {
    let mut line = format!("- [ ] [{}] {}", req.id(), req.text());
    append_typed_refs(&mut line, req);
    append_task_refs(&mut line, task_refs);
    line.push('\n');
    line
}

/// Appends `[tasks: T001, T002]` to `line` when `task_refs` is non-empty.
fn append_task_refs(line: &mut String, task_refs: Option<&Vec<TaskId>>) {
    if let Some(refs) = task_refs {
        if !refs.is_empty() {
            let tags: Vec<String> = refs.iter().map(|t| t.to_string()).collect();
            line.push_str(&format!(" [tasks: {}]", tags.join(", ")));
        }
    }
}

fn append_typed_refs(line: &mut String, req: &SpecRequirement) {
    let adr_refs = req.adr_refs();
    if !adr_refs.is_empty() {
        let tags: Vec<String> = adr_refs
            .iter()
            .map(|r| format!("{}#{}", r.file.to_string_lossy(), r.anchor.as_ref()))
            .collect();
        line.push_str(&format!(" [adr: {}]", tags.join(", ")));
    }

    let conv_refs = req.convention_refs();
    if !conv_refs.is_empty() {
        let tags: Vec<String> = conv_refs
            .iter()
            .map(|r| format!("{}#{}", r.file.to_string_lossy(), r.anchor.as_ref()))
            .collect();
        line.push_str(&format!(" [conv: {}]", tags.join(", ")));
    }

    let informals = req.informal_grounds();
    if !informals.is_empty() {
        let tags: Vec<String> =
            informals.iter().map(|r| format!("{} — {}", r.kind, r.summary.as_ref())).collect();
        line.push_str(&format!(" [informal: {}]", tags.join(", ")));
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
    use std::path::PathBuf;

    use domain::{
        AdrAnchor, AdrRef, ConventionAnchor, ConventionRef, InformalGroundKind, InformalGroundRef,
        InformalGroundSummary, SignalCounts, SpecDocument, SpecElementId, SpecRequirement,
        SpecScope, SpecSection,
    };

    use super::*;

    // ---------------------------------------------------------------------------
    // Helpers
    // ---------------------------------------------------------------------------

    fn id(s: &str) -> SpecElementId {
        SpecElementId::try_new(s).unwrap()
    }

    fn adr_ref(file: &str, anchor: &str) -> AdrRef {
        AdrRef::new(PathBuf::from(file), AdrAnchor::try_new(anchor).unwrap())
    }

    fn conv_ref(file: &str, anchor: &str) -> ConventionRef {
        ConventionRef::new(PathBuf::from(file), ConventionAnchor::try_new(anchor).unwrap())
    }

    fn informal(kind: InformalGroundKind, summary: &str) -> InformalGroundRef {
        InformalGroundRef::new(kind, InformalGroundSummary::try_new(summary).unwrap())
    }

    fn req_with_adr(id_s: &str, text: &str, file: &str, anchor: &str) -> SpecRequirement {
        SpecRequirement::new(id(id_s), text, vec![adr_ref(file, anchor)], vec![], vec![]).unwrap()
    }

    fn req_with_conv(id_s: &str, text: &str, file: &str, anchor: &str) -> SpecRequirement {
        SpecRequirement::new(id(id_s), text, vec![], vec![conv_ref(file, anchor)], vec![]).unwrap()
    }

    fn req_with_informal(
        id_s: &str,
        text: &str,
        kind: InformalGroundKind,
        summary: &str,
    ) -> SpecRequirement {
        SpecRequirement::new(id(id_s), text, vec![], vec![], vec![informal(kind, summary)]).unwrap()
    }

    fn req_bare(id_s: &str, text: &str) -> SpecRequirement {
        SpecRequirement::new(id(id_s), text, vec![], vec![], vec![]).unwrap()
    }

    fn make_minimal_doc() -> SpecDocument {
        SpecDocument::new(
            "Feature X",
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap()
    }

    fn make_full_doc() -> SpecDocument {
        SpecDocument::new(
            "Feature Title",
            "1.0",
            vec![req_with_adr("GL-01", "Goal paragraph line 1", "knowledge/adr/x.md", "D1.2")],
            SpecScope::new(
                vec![req_with_adr("IN-01", "Requirement text", "knowledge/adr/x.md", "D1.2")],
                vec![req_with_informal(
                    "OS-01",
                    "Excluded item",
                    InformalGroundKind::Discussion,
                    "agreed out of scope",
                )],
            ),
            vec![req_with_conv(
                "CO-01",
                "Constraint 1",
                ".claude/rules/04-coding-principles.md",
                "newtype-pattern",
            )],
            vec![req_with_adr("AC-01", "AC text", "knowledge/adr/x.md", "D3.1")],
            vec![
                SpecSection::new("Custom Section Title", vec!["Free-form line 1".into()]).unwrap(),
            ],
            vec![conv_ref("knowledge/conventions/source-attribution.md", "intro")],
            Some(SignalCounts::new(3, 1, 0)),
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
    fn test_render_spec_frontmatter_contains_version_but_not_status() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(output.contains("version: \"1.0\"\n"));
        assert!(!output.contains("status:"), "schema v2 must not emit status in frontmatter");
        assert!(!output.contains("approved_at:"), "schema v2 must not emit approved_at");
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
    // render_spec: goal section (now Vec<SpecRequirement>)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_goal_section_rendered_when_present() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Goal\n"));
        // Goal item has id GL-01 and adr ref
        assert!(output.contains("- [GL-01] Goal paragraph line 1"));
        assert!(output.contains("[adr: knowledge/adr/x.md#D1.2]"));
    }

    #[test]
    fn test_render_spec_goal_section_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("## Goal\n"));
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
    fn test_render_spec_in_scope_requirement_with_adr_ref() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("- [IN-01] Requirement text [adr: knowledge/adr/x.md#D1.2]\n"));
    }

    #[test]
    fn test_render_spec_out_of_scope_rendered_when_present() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("### Out of Scope\n"));
        assert!(
            output
                .contains("- [OS-01] Excluded item [informal: discussion — agreed out of scope]\n")
        );
    }

    #[test]
    fn test_render_spec_out_of_scope_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("### Out of Scope\n"));
    }

    #[test]
    fn test_render_spec_requirement_with_no_refs_has_no_ref_tags() {
        let doc = SpecDocument::new(
            "F",
            "1.0",
            vec![],
            SpecScope::new(vec![req_bare("IN-01", "bare item")], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("- [IN-01] bare item\n"));
        assert!(!output.contains("[adr:"));
        assert!(!output.contains("[conv:"));
        assert!(!output.contains("[informal:"));
    }

    #[test]
    fn test_render_spec_requirement_with_convention_ref() {
        let doc = SpecDocument::new(
            "F",
            "1.0",
            vec![],
            SpecScope::new(
                vec![req_with_conv("IN-01", "req with conv", ".claude/rules/04.md", "newtype")],
                vec![],
            ),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("- [IN-01] req with conv [conv: .claude/rules/04.md#newtype]\n"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: constraints
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_constraints_rendered_when_present() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Constraints\n"));
        assert!(output.contains(
            "- [CO-01] Constraint 1 [conv: .claude/rules/04-coding-principles.md#newtype-pattern]\n"
        ));
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
        assert!(output.contains("- [ ] [AC-01] AC text [adr: knowledge/adr/x.md#D3.1]\n"));
    }

    #[test]
    fn test_render_spec_acceptance_criteria_no_refs_has_no_tags() {
        let doc = SpecDocument::new(
            "F",
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![req_bare("AC-01", "plain AC")],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        let output = render_spec(&doc);
        assert!(output.contains("- [ ] [AC-01] plain AC\n"));
        assert!(!output.contains("[adr:"));
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
        let h2_count = output.lines().filter(|l| l.starts_with("## ")).count();
        // minimal doc: ## Scope only
        assert_eq!(h2_count, 1, "expected only ## Scope, got:\n{output}");
    }

    // ---------------------------------------------------------------------------
    // render_spec: related conventions (now Vec<ConventionRef>)
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_related_conventions_rendered_as_file_hash_anchor() {
        let doc = make_full_doc();
        let output = render_spec(&doc);
        assert!(output.contains("## Related Conventions (Required Reading)\n"));
        assert!(output.contains("- knowledge/conventions/source-attribution.md#intro\n"));
    }

    #[test]
    fn test_render_spec_related_conventions_omitted_when_empty() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(!output.contains("## Related Conventions"));
    }

    // ---------------------------------------------------------------------------
    // render_spec: trailing newline
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_spec_output_ends_with_trailing_newline() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        assert!(output.ends_with('\n'), "output must end with trailing newline");
    }

    #[test]
    fn test_render_spec_minimal_doc_exact_output() {
        let doc = make_minimal_doc();
        let output = render_spec(&doc);
        let expected = "\
<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: \"1.0\"
---

# Feature X

## Scope

### In Scope

";
        assert_eq!(output, expected);
    }

    // ---------------------------------------------------------------------------
    // render_spec: ordering
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

    fn make_doc_with_signals() -> SpecDocument {
        let mut doc = SpecDocument::new(
            "Feature X",
            "1.0",
            vec![],
            SpecScope::new(vec![], vec![]),
            vec![],
            vec![],
            vec![],
            vec![],
            None,
        )
        .unwrap();
        doc.set_signals(SignalCounts::new(12, 1, 0));
        doc
    }

    #[test]
    fn test_render_signal_summary_empty_when_no_signals() {
        let doc = make_minimal_doc();
        let output = render_signal_summary(&doc);
        assert_eq!(output, "");
    }

    #[test]
    fn test_render_signal_summary_stage1_only() {
        let doc = make_doc_with_signals();
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
        let doc = make_doc_with_signals();
        let output = render_signal_summary(&doc);
        assert!(output.ends_with('\n'), "output must end with trailing newline");
    }

    // ---------------------------------------------------------------------------
    // multiple typed refs on single requirement
    // ---------------------------------------------------------------------------

    #[test]
    fn test_render_requirement_with_multiple_adr_refs() {
        let req = SpecRequirement::new(
            id("IN-01"),
            "multi-ref req",
            vec![adr_ref("adr/a.md", "D1"), adr_ref("adr/b.md", "D2")],
            vec![],
            vec![],
        )
        .unwrap();
        let line = render_requirement_with_tasks(&req, None);
        assert_eq!(line, "- [IN-01] multi-ref req [adr: adr/a.md#D1, adr/b.md#D2]\n");
    }

    #[test]
    fn test_render_requirement_with_adr_and_informal_refs() {
        let req = SpecRequirement::new(
            id("IN-01"),
            "mixed refs",
            vec![adr_ref("adr/x.md", "D1")],
            vec![],
            vec![informal(InformalGroundKind::UserDirective, "user asked for it")],
        )
        .unwrap();
        let line = render_requirement_with_tasks(&req, None);
        assert!(line.contains("[adr: adr/x.md#D1]"));
        assert!(line.contains("[informal: user_directive — user asked for it]"));
    }
}
