---
name: dry-fix-lead
description: Use this skill whenever you act as the dry-fix-lead for the DRY fix phase (DFP) in this repository (any task whose prompt assigns a track id and a briefing file via `$dry-fix-lead`). It defines the MANDATORY canonical DRY-gate loop you must follow.
---

# Dry-Fix-Lead (Codex) Skill

You own the DRY fix phase (DFP) for one track: drive the DRY gate to APPROVED by classifying
near-duplicate violations, fixing the genuine ones, and re-running until
`bin/sotp dry check-approved` exits 0 — then print the status line and stop.

## The one rule that matters most

The authoritative DRY verdict is **`bin/sotp dry check-approved` exiting 0 (APPROVED)**, backed
by the verdicts that `bin/sotp dry write` records in the per-track `dry-check.json` — NOT your
own judgment. Run the gate through the prebuilt `bin/sotp` binary, and do not print
`DRY_FIX_STATUS: completed` unless `bin/sotp dry check-approved` has actually exited 0 for this
track.

## Inputs (from the orchestrator prompt)

- Track ID, briefing file path.

## Protocol

### Pre-step: Know your modification boundary

The DFP fixes DRY violations across the track's changed **production / test code** (the diff
since the base commit). Your modification boundary is those changed `.rs` files. Do **NOT** edit,
per the briefing's fixer constraints:

- SoT / generated artifacts: `knowledge/adr/*.md`, `track/items/**` (spec.json / catalogues /
  impl-plan / task-coverage / review.json / dry-check.json / rendered `*.md`),
  `.harness/config/agent-profiles.json`, `.gitignore`.
- Any **other track** under `track/items/<other-track>/`.

If a genuine violation can only be resolved by editing an out-of-boundary file, stop and print
`DRY_FIX_STATUS: blocked` with the file list and rationale.

1. **Classify (records verdicts).** Run:
   ```
   bin/sotp dry write --track-id <track-id>
   ```
   This builds/updates the persistent index and records a verdict
   (`not_a_violation` / `accepted` / `violation`) for every above-threshold pair in the current
   diff into `dry-check.json`; already-recorded unchanged pairs are skipped. The first run builds
   the full corpus index and takes several minutes; once the manifest (`.semantic_index.manifest`)
   exists, later runs are incremental and finish quickly. Do not pass `--model` unless the
   orchestrator told you to (it resolves from `agent-profiles.json`).

2. **Gate.** Run:
   ```
   bin/sotp dry check-approved --track-id <track-id>
   ```
   - Exit 0 (`APPROVED — all pairs verified`) → done. Print `DRY_FIX_STATUS: completed` and stop.
   - Non-zero (`BLOCKED — N unresolved pair(s)`) → go to step 3.

3. **Inspect the current violations.**
   ```
   bin/sotp dry results --track-id <track-id> --filter violation
   ```
   `dry results` shows the full history; the gate's "N unresolved" count is what blocks. The pairs
   recorded most recently (by the `dry write` you just ran) are the current blockers; older
   records for fragments that have since changed are reconciled away by the gate. Verify each
   current blocking pair against the source, and apply the briefing's "Known Accepted Deviations"
   (do not try to "fix" an intentional pattern the briefing lists).

4. **Fix phase.** For each genuine, fixable violation within your modification boundary, apply the
   smallest change that removes the duplication (extract a shared helper, delete a redundant test,
   merge near-identical tests into a table-driven one, etc.). Between fix rounds run
   `cargo make ci-rust` to confirm the workspace still compiles and tests pass.

5. **Re-run.** Go back to step 1 (incremental now — only your changed files are re-classified).
   Repeat until step 2 reports APPROVED.

## Rules

- Run the DRY gate through `bin/sotp` (`dry write` / `dry check-approved` / `dry results`).
- Modify only the track's changed production/test `.rs` files. Never edit SoT / generated
  artifacts, `.gitignore`, `agent-profiles.json`, or another track.
- Do not run `git add` / `git commit` / `git push`. Do not edit `dry-check.json` directly.
- The final line of your output MUST be exactly one of:
  `DRY_FIX_STATUS: completed` / `DRY_FIX_STATUS: blocked` / `DRY_FIX_STATUS: failed`.
