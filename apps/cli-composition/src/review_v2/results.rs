//! Review results rendering helper.

use std::path::Path;

use domain::TrackId;

use super::shared::build_review_v2;

/// Renders the `sotp review results` output as a string, given string-typed parameters.
///
/// Performs all domain operations (build composition, fetch states, evaluate approval,
/// read rounds) internally so that `commands/review/results.rs` never imports domain
/// types directly (CN-01 / AC-03).
///
/// # Parameters
/// - `scope_filter` — optional scope name to filter displayed scopes
/// - `limit` — `None` = state summary only (equivalent to `--limit 0`);
///   `Some(u32::MAX)` = all rounds; `Some(n)` = up to `n` rounds
/// - `round_type` — round-type filter string: `"any"` | `"fast"` | `"final"`
/// - `no_hint` — suppress the commit hint line
///
/// # Errors
/// Returns a human-readable error string on any I/O or domain failure.
pub(crate) fn render_review_results_str(
    track_id_str: &str,
    items_dir: &Path,
    scope_filter: Option<&str>,
    limit: Option<u32>,
    round_type: &str,
    no_hint: bool,
) -> Result<String, String> {
    use domain::review_v2::{
        NotRequiredReason, ReviewApprovalVerdict, ReviewExistsPort as _, ReviewReader, ReviewState,
        ReviewerFinding, RoundType, ScopeName, ScopeRound, Verdict,
    };
    use std::collections::HashMap;
    use std::fmt::Write as _;

    let track_id = TrackId::try_new(track_id_str).map_err(|e| format!("invalid track id: {e}"))?;
    let comp = build_review_v2(&track_id, items_dir)?;

    let states = comp
        .cycle
        .get_review_states(&comp.review_store)
        .map_err(|e| format!("failed to get review states: {e}"))?;

    let review_json_exists = comp
        .review_store
        .review_json_exists()
        .map_err(|e| format!("failed to check review.json existence: {e}"))?;

    let approval_verdict = comp
        .cycle
        .evaluate_approval(&comp.review_store, review_json_exists)
        .map_err(|e| format!("failed to evaluate approval: {e}"))?;

    // Sort scope universe alphabetically.
    let mut scope_universe: Vec<ScopeName> = states.keys().cloned().collect();
    scope_universe.sort_by_key(ToString::to_string);

    // Apply optional scope filter.
    let displayed_scopes: Vec<ScopeName> = if let Some(name) = scope_filter {
        if let Some(scope) = scope_universe.iter().find(|s| s.to_string() == name) {
            vec![scope.clone()]
        } else {
            return Err(format!("scope '{name}' is not defined for this track"));
        }
    } else {
        scope_universe.clone()
    };

    // Always load all rounds per scope so that state_line_suffix() can display
    // the latest round_type@timestamp verdict even in the summary-only path
    // (--limit 0, i.e. limit == None).  The `limit` parameter only controls how
    // many rounds are *expanded* in the findings block, not whether we load them.
    let rounds_per_scope: HashMap<ScopeName, Vec<ScopeRound>> = {
        let mut map = HashMap::new();
        for scope in &displayed_scopes {
            let rounds = comp
                .review_store
                .read_all_rounds(scope)
                .map_err(|e| format!("failed to read rounds for {scope}: {e}"))?;
            map.insert(scope.clone(), rounds);
        }
        map
    };

    // --- Rendering ---

    let is_round_type_fast = round_type == "fast";
    let is_round_type_final = round_type == "final";

    fn round_type_label(rt: RoundType) -> &'static str {
        match rt {
            RoundType::Fast => "fast",
            RoundType::Final => "final",
        }
    }

    fn verdict_label(v: &Verdict) -> &'static str {
        match v {
            Verdict::ZeroFindings => "zero_findings",
            Verdict::FindingsRemain(_) => "findings_remain",
        }
    }

    fn state_line_suffix(rounds: &[ScopeRound]) -> String {
        rounds.last().map_or_else(String::new, |latest| {
            format!(
                "  {}@{} {}",
                match latest.round_type {
                    RoundType::Fast => "fast",
                    RoundType::Final => "final",
                },
                latest.at,
                match &latest.verdict {
                    Verdict::ZeroFindings => "zero_findings",
                    Verdict::FindingsRemain(_) => "findings_remain",
                }
            )
        })
    }

    fn render_findings_block(out: &mut String, findings: &[ReviewerFinding]) {
        if findings.is_empty() {
            let _ = writeln!(out, "    findings: zero_findings");
            return;
        }
        let _ = writeln!(out, "    findings:");
        for finding in findings {
            let severity = finding.severity().unwrap_or("-");
            let location = match (finding.file(), finding.line()) {
                (Some(path), Some(line)) => format!(" ({path}:{line})"),
                (Some(path), None) => format!(" ({path})"),
                (None, _) => String::new(),
            };
            let _ = writeln!(
                out,
                "      - [{severity}] {message}{location}",
                message = finding.message()
            );
            if let Some(category) = finding.category() {
                let _ = writeln!(out, "        category: {category}");
            }
        }
    }

    // Selects which rounds to display based on limit and round_type filter.
    // Returns references into the provided `rounds` slice, newest first.
    fn select_rounds_inner<'a>(
        rounds: &'a [ScopeRound],
        limit: Option<u32>,
        is_fast: bool,
        is_final: bool,
    ) -> Vec<&'a ScopeRound> {
        let Some(n) = limit else {
            return Vec::new();
        };
        let mut filtered: Vec<&'a ScopeRound> = rounds
            .iter()
            .rev()
            .filter(|r| {
                if is_fast {
                    matches!(r.round_type, RoundType::Fast)
                } else if is_final {
                    matches!(r.round_type, RoundType::Final)
                } else {
                    true
                }
            })
            .collect();
        if n != u32::MAX {
            filtered.truncate(n as usize);
        }
        filtered
    }

    let mut out = String::new();
    let _ = writeln!(out, "Review results (v2 scope-based):");
    let _ = writeln!(out, "Diff base: {}", comp.base);
    let _ = writeln!(out);

    let mut approved_count = 0usize;
    let mut empty_count = 0usize;
    let mut required_count = 0usize;

    for scope in &displayed_scopes {
        let state = match states.get(scope) {
            Some(s) => s,
            None => continue,
        };
        let indicator = match state {
            ReviewState::Required(_) => {
                required_count += 1;
                "[-]"
            }
            ReviewState::NotRequired(NotRequiredReason::Empty) => {
                empty_count += 1;
                "[.]"
            }
            ReviewState::NotRequired(NotRequiredReason::ZeroFindings) => {
                approved_count += 1;
                "[+]"
            }
        };
        let scope_rounds = rounds_per_scope.get(scope).map(Vec::as_slice).unwrap_or(&[]);
        let suffix = state_line_suffix(scope_rounds);
        let _ = writeln!(out, "  {indicator} {scope}: {state}{suffix}");

        let displayed_rounds =
            select_rounds_inner(scope_rounds, limit, is_round_type_fast, is_round_type_final);
        if let Some((latest, history)) = displayed_rounds.split_first() {
            render_findings_block(&mut out, latest.findings.as_slice());
            if !history.is_empty() {
                let _ = writeln!(out, "    history (newer first, up to --limit):");
                for round in history {
                    let _ = writeln!(
                        out,
                        "      - {}@{} {}",
                        round_type_label(round.round_type),
                        round.at,
                        verdict_label(&round.verdict)
                    );
                }
            }
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Summary: {approved_count} approved, {empty_count} empty, {required_count} required, {} total",
        displayed_scopes.len()
    );

    let hint_should_emit =
        matches!(approval_verdict, ReviewApprovalVerdict::Approved) && review_json_exists;
    if !no_hint && hint_should_emit {
        let _ =
            writeln!(out, "hint: review approved — run /track:commit <message> to record changes.");
    }

    Ok(out)
}
