---
name: implementer
sandbox: workspace-write
grok-sandbox: workspace
description: Use when Codex is assigned the SoTOHE implementer capability (plan-task implementation on a track branch — source edits, tests, and test-obligation binding authoring). Follows the implementation contract defined in the capability SSoT.
---

# Implementer (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/implementer.md` — the
provider-agnostic contract for this capability (mission, invocation contract, scope
ownership, internal pipeline, architecture guard, output contract, rules). Do not duplicate
it here.

## Codex-skill notes

- Invoked when Codex is assigned the `implementer` capability.
- For file inspection, use `cat` / `grep` / `rg` shell idioms.
- Never run `git add` / `git commit` / `git push` or any git state-changing command; the
  orchestrator owns staging, commits, and task commit-hash recording.
- Do not run `bin/sotp test-obligation evaluate` — evaluation is orchestrator-host-owned.
- End your final message with the status line required by the capability contract
  (`completed` / `blocked` / `failed`) and the report items it lists.

## Session resume conformance

- If your dispatch is a resumed session (orchestrator opt-in continuation), follow the
  "Session resume" section of the capability SSoT: check whether your upstream artifacts
  changed since the prior session and re-read any that did before continuing.
