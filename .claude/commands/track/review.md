---
description: Run review for current track implementation.
---

Canonical command for review in the track workflow.

Arguments:
- Use `$ARGUMENTS` as optional review scope (files/modules/concerns).

Execution:
- Read the latest active track's `spec.md`, `plan.md`, and `metadata.json` before review.
- Read every convention file listed in the `## Related Conventions (Required Reading)` section of `plan.md` before reviewing code.
- For exact type signatures, trait definitions, module trees, and Mermaid diagrams, use `## Canonical Blocks` in `plan.md` and `.claude/docs/DESIGN.md` as the source of truth when reviewing implementation correctness.
- Use any auto-injected external guide summaries from `docs/external-guides.json` before opening cached raw guide documents.
- If `$ARGUMENTS` is provided, scope the review to the specified files/modules/concerns.
- Run review workflow for current track implementation.
- Prefer parallel review style when applicable.

Behavior:
- After execution, summarize:
  1. Major findings (if any)
  2. Required fixes
  3. Merge/commit readiness
