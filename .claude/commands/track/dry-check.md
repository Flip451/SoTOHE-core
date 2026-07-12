---
description: Run the DRY fix phase (DFP) for the current track — sotp dry write → fix DRY violations → sotp dry check-approved loop until the DRY gate passes.
---

> Operational SSoT: `.harness/workflows/track/dry-check.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:dry-check`. No arguments.

## Claude Code invocation constraints

Provider routing is defined by `.harness/config/agent-profiles.json` (`capabilities.dry-fix-lead.provider`); the CLI wrapper below reads it internally. Adapter-specific invocation is required because the wrapper currently implements only the codex path:

- **`provider: codex`**: `cargo make track-local-dry-fix -- --track-id <id> --briefing-file <path>` (the wrapper resolves the provider/model internally).
- **`provider: claude`**: Agent tool (`subagent_type: "dry-fix-lead"`, `run_in_background: true`). Include the track id and the most recent `sotp dry write` findings in the briefing. The wrapper does not yet cover claude; use this direct Agent-tool path.

## Report format

After execution, summarize:

1. The dfl terminal state (`skipped` / `completed` / `blocked` / `failed`).
2. For `skipped`: cite `.harness/config/dry-check.json.enabled: false` and recommend `/track:review`.
3. For `completed`: the verified DRY-gate result and the recommended next command (`/track:review`).
4. For `blocked`: the unresolved violation pairs and the recommended manual/escalation action.
5. For `failed`: the error details.
