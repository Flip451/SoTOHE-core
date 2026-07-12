---
name: researcher
sandbox: read-only
description: Use when Codex is assigned the SoTOHE researcher capability (crate research, codebase-wide analysis, external research). Read-only — never writes files; the orchestrator saves any output that must persist.
---

# Researcher (Codex skill)

**Operational SSoT:** read and follow `.harness/capabilities/researcher.md` — the
provider-agnostic contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `researcher` capability.
- Read-only posture: inspect with `cat` / `grep` / `rg`; do not write files, do not run
  build or state-changing commands. Report findings in your final message; the orchestrator
  persists anything that needs saving.
- Never run any git state-changing command.
