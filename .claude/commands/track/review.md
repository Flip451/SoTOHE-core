---
description: Run review for current track implementation.
---

> Operational SSoT: `.harness/workflows/track/review.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:review`. No arguments.

## Claude Code invocation constraints

- **Context intake**: follow the review workflow SSoT's `Summary-first context intake`. Use CLI
  summaries as the primary context; do not bulk-read review or binding JSON, catalogues, full
  sub-workflow texts, or a `Related Conventions` list. Open only a targeted diff or an artifact
  body named by a blocker; resolved convention paths are supplied to delegated capabilities.
- **Scope discovery**: `bin/sotp review results`
- **ADR guardian dispatch**: when the workflow SSoT selects an ADR guardian-lane capability,
  dispatch that selected `adr-editor` or `adr-diagnoser` capability through
  `bin/sotp capability exec <capability> --host claude --briefing-file <path>`. If the dispatch
  returns `CAPABILITY_EXEC_OUTCOME: delegate-in-host`, invoke the matching Claude Agent tool
  with the returned briefing path and discipline body before continuing.
- **Briefing files**: write to `tmp/reviewer-runtime/briefing-{scope}.md` using the workflow
  SSoT template, including its `## Context Paths` section with the exact spec / plan / task and
  resolved convention paths supplied for the delegated reviewer or fixer; use Read + Edit tools
  for existing briefing files.
- **Fix loop dispatch** (provider-agnostic wrapper — do NOT branch on `capabilities.review-fix-lead.provider` here):
  ```
  cargo make track-local-review-fix -- --scope {scope} \
    --briefing-file tmp/reviewer-runtime/briefing-{scope}.md \
    --round-type fast|final
  ```
  When exit code is `64` with a `SUBAGENT_DISPATCH_REQUIRED` sentinel on stdout, parse the JSON on the next line and spawn a Claude subagent:
  - `subagent_type: "review-fix-lead"`, `run_in_background: true`
  - Pass `scope`, `briefing_file`, `track_id`, `round_type` from the JSON payload.
- **Final gate**: `cargo make ci` then `bin/sotp review check-approved`

## Report format

After execution, summarize:

1. Required scopes and their `final` round verdicts.
2. Findings fixed (with file references).
3. ADR baseline, CI, and `check-approved` result.
4. Commit readiness and the recommended next command (`/track:commit <message>`).
