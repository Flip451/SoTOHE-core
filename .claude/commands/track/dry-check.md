---
description: Run the DRY fix phase (DFP) for the current track — sotp dry write → fix DRY violations → sotp dry check-approved loop until the DRY gate passes.
---

Canonical command for the **DRY fix phase (DFP)**. Runs the full DFP loop over the **whole codebase** (single scope, D13) for the current track branch, driving the `dry-fix-lead` (dfl) agent until the DRY gate passes.

This command is **loosely coupled** to review (D1/OS-01): it does NOT invoke `/track:review`. The DRY gate and the review gate are independent. The orchestrator (`/track:full-cycle`) or the user sequences DFP and RFP.

Requires being on a `track/<id>` branch. If on any other branch, stop and instruct the user to switch first.

## Step 0: Gather context

- Extract the track id from the current git branch (`track/<id>`). If the branch does not match this pattern, stop.
- Read the track's `spec.md` and `plan.md` for context.

## Step 1: Launch the dfl agent

Resolve `capabilities.dry-fix-lead` from `.harness/config/agent-profiles.json`:

- **`provider: claude`** — launch the `dry-fix-lead` subagent (`subagent_type: "dry-fix-lead"`, `run_in_background: true`) with the track id, a briefing generated from the most recent `sotp dry write` `DryCheckFinding` output, and the whole-codebase scope ownership (D13). Wait for its terminal status.
- **`provider: codex`** (default) — launch the T014 wrapper via Bash:
  `cargo make track-local-dry-fix -- --track-id <id> --briefing-file <path>`
  The wrapper runs the same `sotp dry write` → fix → `cargo make ci-rust` → `sotp dry write` → `sotp dry check-approved` loop inside a `workspace-write` sandbox with credential isolation, and prints one of the three terminal status lines.

## Step 2: Read the terminal state

The dfl reports exactly ONE of three **mutually-exclusive** outcomes:

- `completed` — the DRY gate is Approved (the loop ended with `sotp dry check-approved` exit 0 after a successful fix round).
- `blocked` — the DRY gate is still Blocked after the loop exhausted its fix attempts: violations remain that dfl could not resolve autonomously and require escalation / manual intervention. **This is a DRY-gate outcome, NOT a tooling error.**
- `failed` — an execution / tooling error prevented the loop from running correctly.

`blocked` and `failed` are different states and must NEVER be collapsed into one branch — they require different responses.

## Step 3: Handle the outcome

- **`completed`**: verify by running `cargo make track-... ` — i.e. run `bin/sotp dry check-approved --track-id <id>` directly and confirm exit 0. Report that DFP passed, and recommend `/track:review` for the RFP phase (a separate command — DFP does NOT run it).
- **`blocked`**: surface the unresolved DRY violation pairs (from `bin/sotp dry results --track-id <id> --filter violation`). Clearly state this is a DRY-gate block, NOT a tooling error. Do NOT proceed to review. Recommend the user manually resolve the remaining violations (or escalate).
- **`failed`**: report the execution / tooling error and stop. Do NOT proceed.

## Behavior

After execution, summarize:

1. The dfl terminal state (`completed` / `blocked` / `failed`).
2. For `completed`: the verified DRY-gate result and the recommended next command (`/track:review`).
3. For `blocked`: the unresolved violation pairs and the recommended manual/escalation action.
4. For `failed`: the error details.

`/track:dry-check` runs DFP only. It never invokes `/track:review`; the two phases are separate commands (D1/OS-01 loose coupling), sequenced by the full-cycle orchestrator or the user.
