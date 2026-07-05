---
description: Switch to the configured base branch, attempt sync, and show track completion summary.
---

> Operational SSoT: `.harness/workflows/track/done.md` — provider 非依存 workflow logic はそちらを参照。本ファイルは Claude Code 固有 adapter として、起動形態 / Tool 制約 / 報告形式のみを残す。

## Invocation

User invokes this command as `/track:done`. `$ARGUMENTS` is unused.

## Claude Code invocation constraints

- Bash wrappers used:
  - `cargo make track-switch-base` (switches to the configured base branch and then invokes `cargo make sync` internally for the ff-only pull step)
- Read tool used to surface `track/registry.md` content for the completion summary.

## Report format

After execution, summarize:

1. Confirmation that the working tree is on the configured base branch, plus the wrapper's sync-result line verbatim (`[OK] On <base>, up to date.` = confirmed up to date with origin, or `[WARN] Pull failed ...` = sync attempted only; do not restate the WARN case as "up to date").
2. Latest completed track (name + date) from `track/registry.md`.
3. Count of remaining active tracks.
4. Recommended next command (`/track:implement`, `/track:full-cycle <task>`, or `/track:plan <feature>`).
