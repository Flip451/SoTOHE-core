---
description: Run review for current track implementation.
---

> Operational SSoT: `.harness/workflows/track/review.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:review`. No arguments.

## Claude Code invocation constraints

- **Scope discovery**: `bin/sotp review results`
- **Briefing files**: write to `tmp/reviewer-runtime/briefing-{scope}.md`; use Read + Edit tools for existing files.
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
3. CI + `check-approved` result.
4. Commit readiness and the recommended next command (`/track:commit <message>`).
