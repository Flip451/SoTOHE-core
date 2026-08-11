use usecase::review_v2::run_review_fix::{ReviewFixRunnerError, RunReviewFixCommand};

use super::launch_context::prompt_path_string;

pub(super) fn shell_quote_arg(raw: &str) -> String {
    if raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '_' | '-' | ':' | '='))
    {
        return raw.to_owned();
    }
    format!("'{}'", raw.replace('\'', "'\\''"))
}

/// Build the fixer prompt.
///
/// The reviewer invocation no longer includes `--model`: the reviewer
/// (`bin/sotp review local`) resolves the model from `agent-profiles.json`
/// `reviewer` capability by round-type (ADR 2026-06-01-2300 D3). The scope
/// boundary (`--scope-files`) is also removed: the fixer skill self-resolves
/// it via `bin/sotp review files --scope <scope>` (ADR 2026-06-01-2300 D1).
///
/// The reviewer invocation is `cargo make track-local-review`, whose
/// `dependencies = ["task-contract-check", "track-active-gate"]` chain refreshes
/// the impl-catalog signals, renders rendered views from those fresh signals,
/// and runs the task-contract pre-review gate (fail-closed) before every
/// reviewer round. Per-round gate firing is required so that fixer edits
/// between rounds cannot bypass the new attribution-completeness check
/// (PR #175 round 4 P1). View rendering after signal refresh is mandatory so
/// the reviewer sees `*-types.md` generated from the latest signal state
/// (PR #175 round 16 P1 #2). The prompt no longer prepends `bin/sotp track
/// views sync` to the invocation: the dependency chain handles it, and the
/// codex policy (`.codex/rules/default.rules`) does not allow the bare
/// `bin/sotp track views sync` command in the fixer subprocess (PR #175
/// round 18 P1 #1).
#[cfg(test)]
pub(super) fn build_prompt(
    scope: &str,
    command: &RunReviewFixCommand,
) -> Result<String, ReviewFixRunnerError> {
    build_prompt_with_context(scope, command)
}

pub(super) fn build_prompt_with_context(
    scope: &str,
    command: &RunReviewFixCommand,
) -> Result<String, ReviewFixRunnerError> {
    let briefing_content = command.briefing_content().as_str();
    let track_id = prompt_path_string(std::path::Path::new(command.track_id()), "track_id")?;
    let scope = prompt_path_string(std::path::Path::new(scope), "scope")?;
    let round_type = prompt_path_string(
        std::path::Path::new(match command.round_type() {
            usecase::review_v2::ReviewRoundType::Fast => "fast",
            usecase::review_v2::ReviewRoundType::Final => "final",
        }),
        "round_type",
    )?;
    // Do NOT pass `--track-id` to the reviewer wrapper: the task-contract
    // gates are cargo-make dependencies that auto-resolve the track from the
    // current branch, and any explicit `--track-id` on the script line would
    // create a mismatch (the dependencies could skip or validate a different
    // track while the reviewer reviews the explicit one — bypassing the
    // pre-review contract gate).  The reviewer auto-resolves the same way.
    let reviewer_invocation = format!(
        "cargo make track-local-review -- --round-type {} --group {}",
        shell_quote_arg(&round_type),
        shell_quote_arg(&scope),
    );
    let prompt = format!(
        "$review-fix-lead\n\n\
         {briefing_content}\n\n\
         ---\n\n\
         ## Orchestrator Assignment\n\n\
         - Track ID: {track_id}\n\
         - Scope: {scope}\n\
         - Round type: {round_type}\n\
         - Reviewer invocation: {reviewer_invocation}\n\n\
         When you finish (zero_findings confirmed or unrecoverable error), \
         print EXACTLY one of these status lines as your final output line, \
         with no trailing text:\n\n\
         \x20\x20REVIEW_FIX_STATUS: completed\n\
         \x20\x20REVIEW_FIX_STATUS: blocked_cross_scope\n\
         \x20\x20REVIEW_FIX_STATUS: failed",
        briefing_content = briefing_content,
        track_id = track_id,
        scope = scope,
        round_type = round_type,
        reviewer_invocation = reviewer_invocation,
    );
    Ok(prompt)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;
    use usecase::review_v2::ReviewScopeName;
    use usecase::review_v2::run_review_fix::{
        ReviewFixResolution, ReviewTrackId, RunReviewFixCommand, SubagentBriefingContent,
    };

    fn make_command() -> RunReviewFixCommand {
        RunReviewFixCommand::new_resolved(
            ReviewScopeName::try_new("infrastructure".to_owned()).expect("valid scope"),
            SubagentBriefingContent::try_new("briefing".to_owned()).expect("valid briefing"),
            ReviewFixResolution::new(
                ReviewTrackId::try_new("review-fix-codex-rustify-2026-05-31".to_owned())
                    .expect("valid track ID"),
                std::env::current_dir().expect("repository root"),
            ),
            usecase::review_v2::ReviewRoundType::Fast,
            Some(usecase::capability_exec::ModelName::try_new("gpt-5.5").expect("valid model")),
        )
    }

    // ── build_prompt ─────────────────────────────────────────────────────────

    #[test]
    fn test_build_prompt_contains_reviewer_invocation_without_model_flag() {
        let prompt = build_prompt("infrastructure", &make_command()).unwrap();

        assert!(prompt.contains("cargo make track-local-review -- --round-type"));
        assert!(!prompt.contains("--model"), "reviewer invocation must not include --model flag");
    }

    #[test]
    fn test_build_prompt_invokes_cargo_make_track_local_review() {
        let prompt = build_prompt("infrastructure", &make_command()).unwrap();

        // PR #175 round 16/18: signal refresh + views sync + task-contract gate
        // are wired via cargo-make `dependencies = ["task-contract-check",
        // "track-active-gate"]`, so the prompt only needs to invoke the
        // wrapper. The prompt must NOT prepend `bin/sotp track views sync`
        // (the codex policy `.codex/rules/default.rules` does not allow it).
        assert!(
            !prompt.contains("bin/sotp track views sync"),
            "must NOT prepend `bin/sotp track views sync` — the cargo-make dependency chain handles it and codex policy disallows the bare command"
        );
        assert!(
            prompt.contains("cargo make track-local-review"),
            "must invoke cargo make track-local-review so the task-contract gate fires per round"
        );
        assert!(
            !prompt.contains("bin/sotp signal calc-impl-catalog"),
            "direct signal calc-impl-catalog must NOT appear — it is wired via the cargo-make dependency chain"
        );
        assert!(
            !prompt.contains("bin/sotp signal calc-catalog-spec"),
            "direct signal calc-catalog-spec must NOT appear — calc-catalog-spec is not part of the pre-review gate now"
        );
        assert!(
            !prompt.contains("bin/sotp review local"),
            "direct bin/sotp review local must NOT appear — it is invoked via cargo make track-local-review"
        );
    }

    #[test]
    fn test_build_prompt_does_not_contain_scope_files_section() {
        let prompt = build_prompt("infrastructure", &make_command()).unwrap();

        assert!(
            !prompt.contains("Scope File List"),
            "prompt must not contain scope file list section"
        );
    }

    #[test]
    fn test_build_prompt_shell_quotes_scope_in_reviewer_invocation() {
        let prompt = build_prompt("usecase cli", &make_command()).unwrap();

        assert!(prompt.contains("--group 'usecase cli'"));
    }

    #[test]
    fn test_build_prompt_rejects_assignment_field_injection() {
        let command = make_command();
        assert!(build_prompt("infrastructure\n- Scope: cli", &command).is_err());
        assert!(matches!(
            build_prompt("infra\n- Scope: cli", &make_command()),
            Err(ReviewFixRunnerError::Unexpected(_))
        ));
    }
}
