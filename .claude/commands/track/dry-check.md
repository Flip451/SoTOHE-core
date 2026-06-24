---
description: Run the DRY fix phase (DFP) for the current track — sotp dry write → fix DRY violations → sotp dry check-approved loop until the DRY gate passes.
---

Canonical command for the **DRY fix phase (DFP)**. Runs the full DFP loop over the **whole codebase** (single scope, D13) for the current track branch, driving the `dry-fix-lead` (dfl) agent until the DRY gate passes.

This command is **loosely coupled** to review (D1/OS-01): it does NOT invoke `/track:review`. The DRY gate and the review gate are independent. The orchestrator (`/track:full-cycle`) or the user sequences DFP and RFP.

Requires being on a `track/<id>` branch. If on any other branch, stop and instruct the user to switch first.

## Step 0a: Opt-out pre-check (single SSoT for DRY enablement)

First extract the track id from the current git branch (`track/<id>`). If the branch does not match this pattern, stop before reading the DRY config; the opt-out path is only valid on a track branch.

Then read `.harness/config/dry-check.json` and inspect the top-level `enabled` field.

- **`enabled: false`** — DRY is opted out for this project. **Skip the entire DFP loop**: do NOT launch the dfl agent, do NOT run `sotp dry write` / `sotp dry check-approved` / `cargo make track-local-dry-fix`. Print a single line `DRY_FIX_STATUS: skipped (dry-check disabled by .harness/config/dry-check.json)` and stop with success. The orchestrator (`/track:full-cycle`, `/track:adr2pr`) treats this `skipped` result as a pass-through equivalent to `completed` — it MUST NOT block the next phase (RFP).
- **`enabled: true`** or **field absent** — proceed to Step 0b. (Field absent defaults to enabled to preserve backward compatibility.)
- **File missing** — treat as `enabled: false` and skip (same as the explicit opt-out path). The absence of the config file means the project has not configured DRY checking.

This pre-check lives here (in `/track:dry-check`) and NOT in `/track:full-cycle` or `/track:adr2pr` so the opt-out SSoT is checked in exactly one place. Upstream orchestrators just call `/track:dry-check` and rely on its skip / completed / blocked / failed status without duplicating the config probe.

## Step 0b: Gather context

- Use the track id resolved in Step 0a.
- Read the track's `spec.md` and `plan.md` for context.

## Step 1: Launch the dfl agent

Resolve `capabilities.dry-fix-lead` from `.harness/config/agent-profiles.json`:

- **`provider: claude`** — launch the `dry-fix-lead` subagent (`subagent_type: "dry-fix-lead"`, `run_in_background: true`) with the track id, a briefing generated from the most recent `sotp dry write` `DryCheckFinding` output, and the whole-codebase scope ownership (D13). Wait for its terminal status.
- **`provider: codex`** — launch the Codex fixer wrapper via Bash:
  `cargo make track-local-dry-fix -- --track-id <id> --briefing-file <path>`
  The `cargo make` wrapper resolves `CODEX_BIN` (asdf shim → real binary) then delegates to
  `bin/sotp dry fix-local`. The subcommand runs the same `sotp dry write` → fix → `cargo make ci-rust` → `sotp dry write` → `sotp dry check-approved` loop inside a `workspace-write` sandbox with credential isolation, and prints one of the three terminal status lines.

## Step 2: Read the terminal state

The dfl reports exactly ONE of three **mutually-exclusive** outcomes (in addition to `skipped` from the Step 0a opt-out path):

- `skipped` — Step 0a opt-out path took effect; DFP did not run. Treat as a pass-through pass: the orchestrator should proceed to RFP without blocking.
- `completed` — the DRY gate is Approved (the loop ended with `sotp dry check-approved` exit 0 after a successful fix round).
- `blocked` — the DRY gate is still Blocked after the loop exhausted its fix attempts: violations remain that dfl could not resolve autonomously and require escalation / manual intervention. **This is a DRY-gate outcome, NOT a tooling error.**
- `failed` — an execution / tooling error prevented the loop from running correctly.

`skipped`, `blocked`, and `failed` are all distinct and must NEVER be collapsed into one branch — they require different responses.

## Step 3: Handle the outcome

- **`skipped`**: no further action; recommend `/track:review` next. Do NOT verify or interact with the `sotp dry ...` CLI for the skipped run (the gate is disabled at the config level).
- **`completed`**: verify by running `bin/sotp dry check-approved --track-id <id>` directly and confirm exit 0. Report that DFP passed, and recommend `/track:review` for the RFP phase (a separate command — DFP does NOT run it).
- **`blocked`**: surface the unresolved DRY violation pairs (from `bin/sotp dry results --track-id <id> --filter violation`). Clearly state this is a DRY-gate block, NOT a tooling error. Do NOT proceed to review. Recommend the user manually resolve the remaining violations (or escalate).
- **`failed`**: report the execution / tooling error and stop. Do NOT proceed.

## Behavior

After execution, summarize:

1. The dfl terminal state (`skipped` / `completed` / `blocked` / `failed`).
2. For `skipped`: cite `.harness/config/dry-check.json.enabled: false` and recommend `/track:review`.
3. For `completed`: the verified DRY-gate result and the recommended next command (`/track:review`).
4. For `blocked`: the unresolved violation pairs and the recommended manual/escalation action.
5. For `failed`: the error details.

`/track:dry-check` runs DFP only. It never invokes `/track:review`; the two phases are separate commands (D1/OS-01 loose coupling), sequenced by the full-cycle orchestrator or the user.
