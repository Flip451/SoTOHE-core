---
name: review-fix-lead
description: Use this skill whenever you act as the review-fix-lead for a code/plan review scope in this repository (any task whose prompt assigns a scope, a round_type of fast or final, and a briefing file). It defines the MANDATORY canonical review + recording protocol you must follow.
---

# Review-Fix-Lead (Codex) Skill

You own a single review scope for a single `round_type` (`fast` or `final`) assigned by the
orchestrator. Loop: review → fix → re-review until the **canonical reviewer** reports
`zero_findings` for that round_type, then print the status line and stop.

## The one rule that matters most

The authoritative verdict is the one **recorded in `review.json` by the canonical reviewer
command**, NOT your own judgment. You MUST obtain the verdict by running the **Reviewer
invocation** command in step 1 below (the `bin/sotp review local ...` command given in
your orchestrator assignment) — not the `bin/sotp review files --scope` pre-step, which only
resolves your modification boundary and never produces a verdict. Do NOT substitute your own file
inspection, ad-hoc `grep`, or reproduction tests for it, and do NOT print
`REVIEW_FIX_STATUS: completed` unless a `zero_findings` round for your assigned `round_type` has
actually been **recorded** (verified in step 3). Skipping the Reviewer invocation leaves the round
unrecorded, which fails the downstream `bin/sotp review check-approved` gate.

## Inputs (from the orchestrator prompt)

- Track ID, scope name, `round_type` (fast | final), briefing file path.

## Protocol

### Pre-step: Self-resolve the modification boundary

Before entering the review loop, run:

```
bin/sotp review files --scope <scope>
```

This is your **modification boundary** — the files you are allowed to edit. If the command
returns an empty list or fails, make no edits and print `REVIEW_FIX_STATUS: failed` with a
description of the failure. Do not proceed to step 1.

1. **Review (records the round).** Run the **"Reviewer invocation"** command given in your
   orchestrator assignment, EXACTLY as provided, on every round. It is the canonical
   `bin/sotp review local ...` command (it dispatches the reviewer and writes the verdict to `review.json`). Run `bin/sotp track views sync` manually before invoking if you need up-to-date rendered views. Do not drop, add,
   or alter any of its arguments. The reviewer resolves its own model from `agent-profiles.json`
   — do not add or change `--model` on the reviewer invocation. Never decide the verdict by your
   own inspection, ad-hoc greps, or reproduction tests.

2. **Parse the verdict** from the command output:
   - `zero_findings` → go to step 3 (confirm it was recorded).
   - `findings_remain` → go to step 4 (fix).
   - error → print `REVIEW_FIX_STATUS: failed` with the error and stop.

3. **Confirm the recorded round (mandatory before `completed`).** Run:
   ```
   bin/sotp review results --track-id <track-id> --scope <scope> --round-type <round_type> --limit 1
   ```
   Read the **findings block** under the state-line (not the state-line itself). Only if it shows
   `findings: zero_findings` for your `round_type` may you print `REVIEW_FIX_STATUS: completed` and
   stop. If there is no matching recorded entry, you skipped or mis-ran step 1 — go back to step 1.
   (For `round_type == fast`, ignore a `[-] required (stale hash)` state-line; the findings block is
   authoritative. For `final`, the state-line should also be `[+]`/`approved`.)

4. **Fix phase.** Verify each finding against the source before acting. Fix P0/P1/P2 findings only
   within your modification boundary (resolved in the pre-step). If a fix needs files outside your
   boundary, print `REVIEW_FIX_STATUS: blocked_cross_scope` with the out-of-scope file list and
   stop. Between rounds run `cargo make ci-rust` to confirm compilation. After editing `spec.json`
   or `impl-plan.json`, also run `cargo make verify-plan-artifact-refs` (catalogue
   `spec_refs[].hash` can drift).

5. **Re-review.** Go back to step 1. Repeat until step 3 confirms a recorded `zero_findings` round.

## Rules

- Modify only files in your modification boundary (resolved in the pre-step via `bin/sotp review files --scope <scope>`). Silent out-of-scope edits are prohibited.
- Do not run `git add` / `git commit` / `git push`. Do not edit `review.json` directly.
- The final line of your output MUST be exactly one of:
  `REVIEW_FIX_STATUS: completed` / `REVIEW_FIX_STATUS: blocked_cross_scope` / `REVIEW_FIX_STATUS: failed`.
