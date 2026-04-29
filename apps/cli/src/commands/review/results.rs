//! `sotp review results` — read-only review state and round-history reporter.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::process::ExitCode;

use domain::review_v2::{
    NotRequiredReason, ReviewApprovalVerdict, ReviewExistsPort, ReviewReader, ReviewState,
    ReviewerFinding, RoundType, ScopeName, ScopeRound, Verdict,
};
use domain::{CommitHash, TrackId};

use super::compose_v2;
use super::{ResultsArgs, ResultsLimit, RoundTypeFilter};

const HEADER_LINE: &str = "Review results (v2 scope-based):";

pub(super) fn execute_results(args: &ResultsArgs) -> ExitCode {
    match run_results(args) {
        Ok(output) => {
            print!("{output}");
            ExitCode::SUCCESS
        }
        Err(msg) => {
            eprintln!("{msg}");
            ExitCode::FAILURE
        }
    }
}

fn run_results(args: &ResultsArgs) -> Result<String, String> {
    let track_id =
        TrackId::try_new(&args.track_id).map_err(|e| format!("invalid track id: {e}"))?;
    let comp = compose_v2::build_review_v2(&track_id, &args.items_dir)?;

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

    let scope_universe = sorted_scope_universe(&states);
    let displayed_scopes = filter_scopes(&scope_universe, args.scope.as_deref())?;

    // Summary mode (`--limit 0`) uses only the state map + approval verdict.
    // Skip historical round loading so a malformed legacy round cannot
    // hard-fail the status-equivalent default path.
    let rounds_per_scope: HashMap<ScopeName, Vec<ScopeRound>> =
        if matches!(args.limit, ResultsLimit::Zero) {
            HashMap::new()
        } else {
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

    Ok(render_results(
        &comp.base,
        &displayed_scopes,
        &states,
        &rounds_per_scope,
        &approval_verdict,
        review_json_exists,
        args.limit,
        args.round_type,
        args.no_hint,
    ))
}

fn sorted_scope_universe(states: &HashMap<ScopeName, ReviewState>) -> Vec<ScopeName> {
    let mut scopes: Vec<ScopeName> = states.keys().cloned().collect();
    scopes.sort_by_key(ToString::to_string);
    scopes
}

fn filter_scopes(
    universe: &[ScopeName],
    requested: Option<&str>,
) -> Result<Vec<ScopeName>, String> {
    let Some(name) = requested else {
        return Ok(universe.to_vec());
    };
    if let Some(scope) = universe.iter().find(|s| s.to_string() == name) {
        Ok(vec![scope.clone()])
    } else {
        Err(format!("scope '{name}' is not defined for this track"))
    }
}

#[allow(clippy::too_many_arguments)]
fn render_results(
    base: &CommitHash,
    displayed: &[ScopeName],
    states: &HashMap<ScopeName, ReviewState>,
    rounds_per_scope: &HashMap<ScopeName, Vec<ScopeRound>>,
    approval_verdict: &ReviewApprovalVerdict,
    review_json_exists: bool,
    limit: ResultsLimit,
    round_filter: RoundTypeFilter,
    no_hint: bool,
) -> String {
    let mut out = String::new();
    let _ = writeln!(out, "{HEADER_LINE}");
    let _ = writeln!(out, "Diff base: {base}");
    let _ = writeln!(out);

    let mut approved_count = 0usize;
    let mut empty_count = 0usize;
    let mut required_count = 0usize;

    for scope in displayed {
        let state = match states.get(scope) {
            Some(state) => state,
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

        let displayed_rounds = select_displayed_rounds(scope_rounds, limit, round_filter);
        if let Some((latest, history)) = displayed_rounds.split_first() {
            render_findings_block(&mut out, latest.findings.as_slice());
            if !history.is_empty() {
                render_history_block(&mut out, history);
            }
        }
    }

    let _ = writeln!(out);
    let _ = writeln!(
        out,
        "Summary: {approved_count} approved, {empty_count} empty, {required_count} required, {} total",
        displayed.len()
    );

    if !no_hint && hint_should_emit(approval_verdict, review_json_exists) {
        let _ =
            writeln!(out, "hint: review approved — run /track:commit <message> to record changes.");
    }
    out
}

fn state_line_suffix(rounds: &[ScopeRound]) -> String {
    rounds.last().map_or_else(String::new, |latest| {
        format!(
            "  {}@{} {}",
            round_type_label(latest.round_type),
            latest.at,
            verdict_label(&latest.verdict)
        )
    })
}

fn select_displayed_rounds(
    rounds: &[ScopeRound],
    limit: ResultsLimit,
    round_filter: RoundTypeFilter,
) -> Vec<&ScopeRound> {
    if matches!(limit, ResultsLimit::Zero) {
        return Vec::new();
    }
    let mut filtered: Vec<&ScopeRound> =
        rounds.iter().rev().filter(|r| filter_matches(r.round_type, round_filter)).collect();
    if let ResultsLimit::Count(n) = limit {
        filtered.truncate(n as usize);
    }
    filtered
}

fn filter_matches(round_type: RoundType, filter: RoundTypeFilter) -> bool {
    match filter {
        RoundTypeFilter::Any => true,
        RoundTypeFilter::Fast => matches!(round_type, RoundType::Fast),
        RoundTypeFilter::Final => matches!(round_type, RoundType::Final),
    }
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
        let _ =
            writeln!(out, "      - [{severity}] {message}{location}", message = finding.message());
        if let Some(category) = finding.category() {
            let _ = writeln!(out, "        category: {category}");
        }
    }
}

fn render_history_block(out: &mut String, history: &[&ScopeRound]) {
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

fn round_type_label(round_type: RoundType) -> &'static str {
    match round_type {
        RoundType::Fast => "fast",
        RoundType::Final => "final",
    }
}

fn verdict_label(verdict: &Verdict) -> &'static str {
    match verdict {
        Verdict::ZeroFindings => "zero_findings",
        Verdict::FindingsRemain(_) => "findings_remain",
    }
}

fn hint_should_emit(verdict: &ReviewApprovalVerdict, review_json_exists: bool) -> bool {
    matches!(verdict, ReviewApprovalVerdict::Approved) && review_json_exists
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::indexing_slicing)]
mod tests {
    use std::collections::HashMap;

    use domain::CommitHash;
    use domain::review_v2::{
        MainScopeName, NotRequiredReason, RequiredReason, ReviewApprovalVerdict, ReviewHash,
        ReviewState, ReviewerFinding, RoundType, ScopeName, ScopeRound, Verdict,
    };

    use super::{
        ResultsLimit, RoundTypeFilter, render_results, select_displayed_rounds, state_line_suffix,
    };

    fn base() -> CommitHash {
        CommitHash::try_new("abcdef0123456789").unwrap()
    }

    fn domain_scope() -> ScopeName {
        ScopeName::Main(MainScopeName::new("domain").unwrap())
    }

    fn usecase_scope() -> ScopeName {
        ScopeName::Main(MainScopeName::new("usecase").unwrap())
    }

    fn finding(message: &str, severity: Option<&str>) -> ReviewerFinding {
        ReviewerFinding::new(message, severity.map(str::to_owned), None, None, None).unwrap()
    }

    fn round(
        round_type: RoundType,
        verdict: Verdict,
        findings: Vec<ReviewerFinding>,
        at: &str,
    ) -> ScopeRound {
        ScopeRound {
            round_type,
            verdict,
            findings,
            hash: ReviewHash::computed("rvw1:sha256:deadbeef").unwrap(),
            at: at.to_owned(),
        }
    }

    #[test]
    fn render_results_state_summary_only_with_zero_limit() {
        let mut states = HashMap::new();
        states.insert(domain_scope(), ReviewState::NotRequired(NotRequiredReason::ZeroFindings));
        states.insert(usecase_scope(), ReviewState::Required(RequiredReason::FindingsRemain));

        let mut rounds = HashMap::new();
        rounds.insert(
            domain_scope(),
            vec![round(RoundType::Final, Verdict::ZeroFindings, vec![], "2026-04-29T10:00:00Z")],
        );
        rounds.insert(
            usecase_scope(),
            vec![round(
                RoundType::Final,
                Verdict::findings_remain(vec![finding("bug", Some("P1"))]).unwrap(),
                vec![finding("bug", Some("P1"))],
                "2026-04-29T11:00:00Z",
            )],
        );

        let displayed = vec![domain_scope(), usecase_scope()];
        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::Blocked { required_scopes: vec![usecase_scope()] },
            true,
            ResultsLimit::Zero,
            RoundTypeFilter::Any,
            false,
        );

        assert!(out.starts_with("Review results (v2 scope-based):\n"));
        assert!(out.contains("Diff base: abcdef0123456789\n"));
        assert!(out.contains("[+] domain: approved  final@2026-04-29T10:00:00Z zero_findings"));
        assert!(out.contains(
            "[-] usecase: required (findings remain)  final@2026-04-29T11:00:00Z findings_remain"
        ));
        assert!(out.contains("Summary: 1 approved, 0 empty, 1 required, 2 total"));
        // No findings expansion (--limit 0)
        assert!(!out.contains("findings:"));
        // No hint (Blocked verdict)
        assert!(!out.contains("hint:"));
    }

    #[test]
    fn render_results_findings_expansion_for_latest_round() {
        let mut states = HashMap::new();
        states.insert(domain_scope(), ReviewState::Required(RequiredReason::FindingsRemain));
        let findings = vec![finding("bug A", Some("P1")), finding("bug B", Some("P2"))];
        let mut rounds = HashMap::new();
        rounds.insert(
            domain_scope(),
            vec![
                round(RoundType::Final, Verdict::ZeroFindings, vec![], "2026-04-29T09:00:00Z"),
                round(
                    RoundType::Final,
                    Verdict::findings_remain(findings.clone()).unwrap(),
                    findings,
                    "2026-04-29T10:00:00Z",
                ),
            ],
        );
        let displayed = vec![domain_scope()];

        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::Blocked { required_scopes: vec![domain_scope()] },
            true,
            ResultsLimit::Count(2),
            RoundTypeFilter::Any,
            false,
        );

        assert!(out.contains("findings:"));
        assert!(out.contains("- [P1] bug A"));
        assert!(out.contains("- [P2] bug B"));
        assert!(out.contains("history (newer first, up to --limit):"));
        assert!(out.contains("- final@2026-04-29T09:00:00Z zero_findings"));
    }

    #[test]
    fn render_results_round_type_filter_excludes_non_matching_rounds() {
        let mut states = HashMap::new();
        states.insert(domain_scope(), ReviewState::Required(RequiredReason::FindingsRemain));
        let mut rounds = HashMap::new();
        rounds.insert(
            domain_scope(),
            vec![
                round(RoundType::Fast, Verdict::ZeroFindings, vec![], "2026-04-29T08:00:00Z"),
                round(RoundType::Final, Verdict::ZeroFindings, vec![], "2026-04-29T09:00:00Z"),
                round(RoundType::Final, Verdict::ZeroFindings, vec![], "2026-04-29T10:00:00Z"),
                round(RoundType::Fast, Verdict::ZeroFindings, vec![], "2026-04-29T11:00:00Z"),
            ],
        );
        let displayed = vec![domain_scope()];

        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::Blocked { required_scopes: vec![domain_scope()] },
            true,
            ResultsLimit::All,
            RoundTypeFilter::Final,
            false,
        );
        // After --round-type final filter, the displayed rounds (newer first) are:
        //   [final@10:00 (latest → findings expansion), final@09:00 (history)]
        // Fast rounds at 08:00 and 11:00 are filtered out of the displayed set.
        assert!(out.contains("history (newer first, up to --limit):"));
        assert!(out.contains("- final@2026-04-29T09:00:00Z zero_findings"));
        assert!(!out.contains("- fast@2026-04-29T08:00:00Z"));
        assert!(!out.contains("- fast@2026-04-29T11:00:00Z"));
        // State-line suffix always uses the actual latest round regardless of filter
        // (here the actual latest is fast@11:00).
        assert!(out.contains("fast@2026-04-29T11:00:00Z zero_findings"));
    }

    #[test]
    fn render_results_emits_hint_when_approved_and_review_json_present() {
        let mut states = HashMap::new();
        states.insert(domain_scope(), ReviewState::NotRequired(NotRequiredReason::ZeroFindings));
        let mut rounds = HashMap::new();
        rounds.insert(domain_scope(), vec![]);
        let displayed = vec![domain_scope()];

        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::Approved,
            true,
            ResultsLimit::Zero,
            RoundTypeFilter::Any,
            false,
        );
        assert!(out.contains("hint: review approved"));
    }

    #[test]
    fn render_results_no_hint_flag_suppresses_hint() {
        let mut states = HashMap::new();
        states.insert(domain_scope(), ReviewState::NotRequired(NotRequiredReason::ZeroFindings));
        let mut rounds = HashMap::new();
        rounds.insert(domain_scope(), vec![]);
        let displayed = vec![domain_scope()];

        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::Approved,
            true,
            ResultsLimit::Zero,
            RoundTypeFilter::Any,
            true,
        );
        assert!(!out.contains("hint:"));
    }

    #[test]
    fn render_results_no_hint_when_review_json_absent() {
        let mut states = HashMap::new();
        states.insert(domain_scope(), ReviewState::NotRequired(NotRequiredReason::ZeroFindings));
        let mut rounds = HashMap::new();
        rounds.insert(domain_scope(), vec![]);
        let displayed = vec![domain_scope()];

        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::Approved,
            false,
            ResultsLimit::Zero,
            RoundTypeFilter::Any,
            false,
        );
        assert!(!out.contains("hint:"));
    }

    #[test]
    fn render_results_no_hint_when_approved_with_bypass() {
        let mut states = HashMap::new();
        states.insert(domain_scope(), ReviewState::Required(RequiredReason::NotStarted));
        let mut rounds = HashMap::new();
        rounds.insert(domain_scope(), vec![]);
        let displayed = vec![domain_scope()];

        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::ApprovedWithBypass { not_started_count: 1 },
            false,
            ResultsLimit::Zero,
            RoundTypeFilter::Any,
            false,
        );
        assert!(!out.contains("hint:"));
    }

    #[test]
    fn render_results_includes_other_scope_in_universe() {
        let mut states = HashMap::new();
        states.insert(ScopeName::Other, ReviewState::NotRequired(NotRequiredReason::Empty));
        states.insert(domain_scope(), ReviewState::NotRequired(NotRequiredReason::ZeroFindings));
        let mut rounds = HashMap::new();
        rounds.insert(ScopeName::Other, vec![]);
        rounds.insert(domain_scope(), vec![]);

        // Sorted alphabetically: domain, other
        let displayed = vec![domain_scope(), ScopeName::Other];

        let out = render_results(
            &base(),
            &displayed,
            &states,
            &rounds,
            &ReviewApprovalVerdict::Approved,
            true,
            ResultsLimit::Zero,
            RoundTypeFilter::Any,
            false,
        );
        assert!(out.contains("[+] domain: approved"));
        assert!(out.contains("[.] other: not required (empty)"));
    }

    #[test]
    fn select_displayed_rounds_zero_returns_empty() {
        let rounds = vec![round(RoundType::Final, Verdict::ZeroFindings, vec![], "t")];
        let result = select_displayed_rounds(&rounds, ResultsLimit::Zero, RoundTypeFilter::Any);
        assert!(result.is_empty());
    }

    #[test]
    fn select_displayed_rounds_returns_newest_first() {
        let rounds = vec![
            round(RoundType::Final, Verdict::ZeroFindings, vec![], "t1"),
            round(RoundType::Final, Verdict::ZeroFindings, vec![], "t2"),
            round(RoundType::Final, Verdict::ZeroFindings, vec![], "t3"),
        ];
        let result = select_displayed_rounds(&rounds, ResultsLimit::All, RoundTypeFilter::Any);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].at, "t3");
        assert_eq!(result[1].at, "t2");
        assert_eq!(result[2].at, "t1");
    }

    #[test]
    fn select_displayed_rounds_truncates_to_count() {
        let rounds = vec![
            round(RoundType::Final, Verdict::ZeroFindings, vec![], "t1"),
            round(RoundType::Final, Verdict::ZeroFindings, vec![], "t2"),
            round(RoundType::Final, Verdict::ZeroFindings, vec![], "t3"),
        ];
        let result = select_displayed_rounds(&rounds, ResultsLimit::Count(2), RoundTypeFilter::Any);
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].at, "t3");
        assert_eq!(result[1].at, "t2");
    }

    #[test]
    fn state_line_suffix_empty_when_no_rounds() {
        let suffix = state_line_suffix(&[]);
        assert_eq!(suffix, "");
    }

    #[test]
    fn state_line_suffix_uses_latest_round_regardless_of_filter() {
        let rounds = vec![
            round(RoundType::Final, Verdict::ZeroFindings, vec![], "t1"),
            round(RoundType::Fast, Verdict::ZeroFindings, vec![], "t2"),
        ];
        let suffix = state_line_suffix(&rounds);
        assert!(suffix.contains("fast@t2 zero_findings"));
    }
}
