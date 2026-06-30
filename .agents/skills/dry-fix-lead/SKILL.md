---
name: dry-fix-lead
description: Use this skill whenever you act as the dry-fix-lead for the DRY fix phase (DFP) in this repository (any task whose prompt assigns a track id and a briefing file). Follows the canonical DRY-gate loop defined in the capability SSoT.
---

# Dry-Fix-Lead (Codex) Skill

**Operational SSoT:** read and follow `.harness/capabilities/dry-fix-lead.md` — the
provider-agnostic contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `dry-fix-lead` capability.
- For file inspection, use `cat` / `grep` / `rg` shell idioms.
- The final line of your output MUST be exactly one of:
  `DRY_FIX_STATUS: completed` / `DRY_FIX_STATUS: blocked` / `DRY_FIX_STATUS: failed`.
