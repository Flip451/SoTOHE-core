//! Presentation-only renderer for structured review-results output.

use std::fmt::Write as _;

use usecase::review_v2::{
    ReviewResultsOutput, ReviewRoundResultVerdict, ReviewRoundType, ReviewScopeResultState,
};

pub(super) fn render_review_results(
    output: ReviewResultsOutput,
    limit: u32,
    round_type: &str,
    no_hint: bool,
) -> String {
    let mut rendered = String::new();
    let _ = writeln!(rendered, "Review results (v2 scope-based):");
    let _ = writeln!(rendered, "Diff base: {}", output.base);
    let _ = writeln!(rendered);
    let mut approved_count = 0usize;
    let mut empty_count = 0usize;
    let mut required_count = 0usize;
    for scope in &output.scopes {
        let (indicator, state_label) = match scope.state {
            ReviewScopeResultState::RequiredNotStarted => {
                required_count += 1;
                ("[-]", "required (not started)")
            }
            ReviewScopeResultState::RequiredFindingsRemain => {
                required_count += 1;
                ("[-]", "required (findings remain)")
            }
            ReviewScopeResultState::RequiredStaleHash => {
                required_count += 1;
                ("[-]", "required (stale hash)")
            }
            ReviewScopeResultState::Empty => {
                empty_count += 1;
                ("[.]", "not required (empty)")
            }
            ReviewScopeResultState::Approved => {
                approved_count += 1;
                ("[+]", "approved")
            }
        };
        let latest_suffix = scope.rounds.last().map_or_else(String::new, |latest| {
            format!(
                "  {}@{} {}",
                round_type_label(&latest.round_type),
                latest.at,
                verdict_label(&latest.verdict)
            )
        });
        let _ = writeln!(
            rendered,
            "  {indicator} {}: {state_label}{latest_suffix}",
            scope.scope.as_str()
        );
        if limit != 0 {
            let mut selected: Vec<_> = scope
                .rounds
                .iter()
                .rev()
                .filter(|round| match round_type {
                    "fast" => matches!(round.round_type, ReviewRoundType::Fast),
                    "final" => matches!(round.round_type, ReviewRoundType::Final),
                    _ => true,
                })
                .collect();
            if limit != u32::MAX {
                selected.truncate(limit as usize);
            }
            if let Some((latest, history)) = selected.split_first() {
                render_findings(&mut rendered, &latest.verdict);
                if !history.is_empty() {
                    let _ = writeln!(rendered, "    history (newer first, up to --limit):");
                    for round in history {
                        let _ = writeln!(
                            rendered,
                            "      - {}@{} {}",
                            round_type_label(&round.round_type),
                            round.at,
                            verdict_label(&round.verdict)
                        );
                    }
                }
            }
        }
    }
    let _ = writeln!(rendered);
    let _ = writeln!(
        rendered,
        "Summary: {approved_count} approved, {empty_count} empty, {required_count} required, {} total",
        output.scopes.len()
    );
    if !no_hint && output.hint_should_emit {
        let _ = writeln!(
            rendered,
            "hint: review approved — run /track:commit <message> to record changes."
        );
    }
    rendered
}

fn round_type_label(round_type: &ReviewRoundType) -> &'static str {
    match round_type {
        ReviewRoundType::Fast => "fast",
        ReviewRoundType::Final => "final",
    }
}

fn verdict_label(verdict: &ReviewRoundResultVerdict) -> &'static str {
    match verdict {
        ReviewRoundResultVerdict::ZeroFindings => "zero_findings",
        ReviewRoundResultVerdict::FindingsRemain(_) => "findings_remain",
    }
}

fn render_findings(out: &mut String, verdict: &ReviewRoundResultVerdict) {
    let ReviewRoundResultVerdict::FindingsRemain(findings) = verdict else {
        let _ = writeln!(out, "    findings: zero_findings");
        return;
    };
    let _ = writeln!(out, "    findings:");
    for finding in findings.as_slice() {
        let severity = finding.severity.as_deref().unwrap_or("-");
        let location = match (finding.file.as_deref(), finding.line) {
            (Some(path), Some(line)) => format!(" ({path}:{line})"),
            (Some(path), None) => format!(" ({path})"),
            (None, _) => String::new(),
        };
        let _ = writeln!(
            out,
            "      - [{severity}] {message}{location}",
            message = finding.message.as_str()
        );
        if let Some(category) = finding.category.as_deref() {
            let _ = writeln!(out, "        category: {category}");
        }
    }
}
