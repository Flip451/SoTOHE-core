---
description: Plan a feature via the canonical track planning workflow.
---

Canonical wrapper for feature planning in this template.

Arguments:
- Use `$ARGUMENTS` as the feature name.
- If empty, ask for a feature name and stop.

Execution:
- Perform a 3-phase planning workflow for `$ARGUMENTS`:
  1. Understand: codebase and version baseline
  2. Research & design: Gemini/Codex collaboration as needed
  3. Plan & approve: produce plan and request user approval
- If `$ARGUMENTS` matches `docs/external-guides.json` `trigger_keywords`, use the injected guide summaries before opening cached raw documents.
- Keep `.claude/docs/DESIGN.md` in English for cross-provider compatibility, but keep crate/module names aligned with `plan.md`.
- When the `planner` capability response contains a `## Canonical Blocks` section, copy every block in that section verbatim into `plan.md` or `DESIGN.md`. Do not summarize, translate, or rewrite those blocks. Surrounding explanation text may be summarized in Japanese for `plan.md`.

After approval — create track artifacts:
- Remind the user that unresolved `track/tech-stack.md` `TODO:` entries will fail CI.
- Create a new directory under `track/items/` using a safe slug and timestamp/id as needed to avoid collisions.
- Create `metadata.json` (SSoT) with schema_version 3 and all required fields:
  - `schema_version`: 3
  - `id` (must exactly match the created track directory name)
  - `branch`: `"track/<track-id>"`
  - `title`
  - `status`: `"planned"`
  - `created_at` (ISO 8601)
  - `updated_at` (ISO 8601)
  - `tasks`: array of task objects (`{id, description, status, commit_hash}`)
  - `plan`: `{summary, sections}` where sections reference task IDs
- After writing metadata.json, create and switch to the track branch:
  - Run `cargo make track-branch-create '<track-id>'` to create branch `track/<track-id>` and switch to it.
  - If branch creation fails (e.g. branch already exists), switch to it with `cargo make track-branch-switch '<track-id>'` instead.
- Generate `plan.md` from `metadata.json` via `scripts/track_markdown.py` `render_plan()`. Do NOT write `plan.md` directly — it is a read-only view rendered from metadata.json.
- Include a `## Related Conventions (Required Reading)` section in `plan.md` listing repo-relative paths to relevant `project-docs/conventions/*.md` files. If none apply, write `None`. Do not use `- [ ]` checkbox format (conflicts with task parser).
- If `plan.md` needs an architecture or dependency diagram, use Mermaid `flowchart TD`.
- Do not use ASCII box drawings in `plan.md`.
- Initialize `spec.md` with feature goal, scope, constraints, and acceptance criteria sections.
- Initialize `verification.md` with sections for:
  - scope verified
  - manual verification steps
  - result / open issues
  - verified_at
- Update `track/registry.md`:
  - add or refresh the active track row
  - set `Current Focus`
  - refresh `Last updated`
- Do not implement code in this command.

Behavior:
- Treat this command as the planning gate before implementation.
- After creating track artifacts, present:
  1. Plan summary
  2. Created track id/path
  3. Created/updated files
  4. Suggested next command (`/track:implement` or `/track:full-cycle <task>`)
  5. Alternative: use `/track:plan-only <feature>` to create planning artifacts without a branch, then `/track:activate <track-id>` when ready to implement
