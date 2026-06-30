---
name: review-fix-lead
description: Use this skill whenever you act as the review-fix-lead for a code/plan review scope in this repository (any task whose prompt assigns a scope, a round_type of fast or final, and a briefing file). Follows the canonical review + fix + re-review loop defined in the capability SSoT.
---

# Review-Fix-Lead (Codex) Skill

**Operational SSoT:** read and follow `.harness/capabilities/review-fix-lead.md` — the
provider-agnostic contract for this capability. Do not duplicate it here.

## Codex-skill notes

- Invoked when Codex is assigned the `review-fix-lead` capability.
- For file inspection, use `cat` / `grep` / `rg` shell idioms.
- The final line of your output MUST be exactly one of:
  `REVIEW_FIX_STATUS: completed` / `REVIEW_FIX_STATUS: blocked_cross_scope` /
  `REVIEW_FIX_STATUS: failed`.
