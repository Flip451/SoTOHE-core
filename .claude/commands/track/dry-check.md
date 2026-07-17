---
description: Run the DRY fix phase (DFP) for the current track — sotp dry write → fix DRY violations → sotp dry check-approved loop until the DRY gate passes.
---

> Operational SSoT: `.harness/workflows/track/dry-check.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:dry-check`. No arguments.

## Claude Code invocation constraints

Dispatch: `cargo make track-local-dry-fix -- --track-id <id> --briefing-file <path>`. The
wrapper resolves `capabilities.dry-fix-lead` internally from
`.harness/config/agent-profiles.json`; this adapter never resolves or branches on the provider
itself. The wrapper currently implements only the codex provider path: if the profile routes
`dry-fix-lead` to a provider the wrapper does not support, the dispatch fails — report that
failure to the user and stop (fail-closed). Do NOT fall back to a direct Agent-tool dispatch.

## Report format

After execution, summarize:

1. The dfl terminal state (`skipped` / `completed` / `blocked` / `failed`).
2. For `skipped`: cite `.harness/config/dry-check.json.enabled: false` and recommend `/track:review`.
3. For `completed`: the verified DRY-gate result and the recommended next command (`/track:review`).
4. For `blocked`: the unresolved violation pairs and the recommended manual/escalation action.
5. For `failed`: the error details.
