# Dry-Check Workflow SSoT

> Provider-agnostic workflow SSoT for the `dry-check` track workflow. Both the Claude adapter
> (`.claude/commands/track/dry-check.md`) and the Codex skill adapter
> (`.agents/skills/track-dry-check/SKILL.md`) reference this file. Provider-specific
> invocation framing lives in those adapters; the full workflow contract lives here.

## Mission

Run the DRY fix phase (DFP) for the current track branch. The workflow drives the
`dry-fix-lead` (dfl) capability through the whole-codebase DRY gate (single scope) until the
gate passes, is opted out, or the loop is exhausted. DFP is **loosely coupled** to review
(D1/OS-01): this workflow does NOT invoke the `review` workflow. The DRY gate and the review
gate are independent. The sequencing of DFP and RFP is owned by the caller (`full-cycle`
workflow) or the user.

Requires being on a `track/<id>` branch.

## Inputs

- **Current branch** — must match `track/<id>`. If not, stop before reading any DRY config.
- **`.harness/config/dry-check.json`** — the single source of truth for DRY enablement.
  Inspected in Step 0a. If the file is missing, treat as `enabled: false` (skip).
- **Track context** — `spec.md` and `plan.md` for context, used to prepare the dfl briefing.

## Sequence

**Step 0a: Opt-out pre-check (single SSoT for DRY enablement)**

Extract the track id from the current git branch (`track/<id>`). If the branch does not match
this pattern, stop before reading the DRY config.

Read `.harness/config/dry-check.json` and inspect the top-level `enabled` field:

- **`enabled: false`**: DRY is opted out. Skip the entire DFP loop; do not launch the dfl
  capability, do not run `sotp dry write` / `sotp dry check-approved`. Print:
  `DRY_FIX_STATUS: skipped (dry-check disabled by .harness/config/dry-check.json)` and stop
  with success. The caller treats `skipped` as a pass-through equivalent to `completed` and
  must NOT block the next phase.
- **`enabled: true`** or **field absent**: proceed to Step 0b.
- **File missing**: treat as `enabled: false` and skip (same as explicit opt-out).

This pre-check lives here (in the `dry-check` workflow) and NOT in `full-cycle` or `adr2pr`.
Upstream orchestrators call this workflow and rely on its skip / completed / blocked / failed
status without duplicating the config probe.

**Step 0b: Gather context**

Use the track id resolved in Step 0a. Read `spec.md` and `plan.md` for context.

**Step 1: Launch the dfl capability**

Resolve `capabilities.dry-fix-lead` from `.harness/config/agent-profiles.json` and dispatch
to the appropriate runner via the provider-agnostic wrapper:

```
cargo make track-local-dry-fix -- --track-id <id> --briefing-file <path>
```

The briefing file is generated from the most recent `sotp dry write` `DryCheckFinding` output
and the track context gathered in Step 0b. The `cargo make` wrapper resolves the provider
internally and runs the dfl loop: `sotp dry write` → fix DRY violations →
`cargo make ci-rust` → `sotp dry write` → `sotp dry check-approved`, iterating until the gate
passes or the loop is exhausted.

**Step 2: Read the terminal state**

The dfl reports exactly ONE of four mutually-exclusive terminal statuses. These statuses are
never collapsed into a single branch:

- **`skipped`** — Step 0a opt-out path; DFP did not run.
- **`completed`** — DRY gate is Approved (loop ended with `sotp dry check-approved` exit 0
  after a successful fix round).
- **`blocked`** — DRY gate is still Blocked after the loop exhausted its fix attempts.
  Violations remain that dfl could not resolve autonomously. This is a DRY-gate outcome, NOT
  a tooling error.
- **`failed`** — execution / tooling error prevented the loop from running correctly.

**Step 3: Handle the outcome**

- **`skipped`**: no further action. Recommend `review` workflow next. Do NOT probe the
  `sotp dry ...` CLI (the gate is disabled at the config level).
- **`completed`**: verify by running `bin/sotp dry check-approved --track-id <id>` directly
  and confirming exit 0. Report DFP passed and recommend `review` workflow for the RFP phase.
  DFP does NOT invoke the `review` workflow itself.
- **`blocked`**: surface the unresolved DRY violation pairs from
  `bin/sotp dry results --track-id <id> --filter violation`. Clearly state this is a DRY-gate
  block, NOT a tooling error. Do NOT proceed to review. Recommend manual resolution or escalation.
- **`failed`**: report the execution / tooling error and stop. Do NOT proceed.

## Gates

| Step | Gate | Verdict |
|------|------|---------|
| 0a | Branch matches `track/<id>` | OK / stop |
| 0a | `dry-check.json.enabled` | `true`/absent → proceed; `false`/missing → skipped |
| 3 | `bin/sotp dry check-approved --track-id <id>` exits 0 (completed path only) | pass / fail |

## Failure / recovery

- **Non-track branch**: stop before reading DRY config. Report the branch situation to the caller.
- **dfl `blocked`**: surface violation pairs. Stop the batch loop (the caller must not proceed
  to review). Recommend manual resolution. The commit wrapper enforces the DRY gate as a hard
  precondition: `cargo make track-commit-message` runs `sotp dry check-approved` and refuses
  to proceed while the DRY gate is Blocked.
- **dfl `failed`**: report the tooling error. Stop and do not proceed to review.
- **`bin/sotp dry check-approved` non-zero on `completed` path**: this is a post-loop
  verification failure. Re-run the dfl loop or investigate the discrepancy before declaring
  the gate passed.

## Outputs

- DRY gate terminal status: `skipped` / `completed` / `blocked` / `failed`
- For `skipped`: cite `dry-check.json.enabled: false`; recommend `review` workflow
- For `completed`: verified `check-approved` exit 0 and recommended next workflow (`review`)
- For `blocked`: unresolved violation pairs; recommended manual / escalation action
- For `failed`: error details
- No commit is created by this workflow; DRY gate must pass before the `commit` workflow runs
