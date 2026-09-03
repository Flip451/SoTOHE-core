# Orchestrator Rules

This is the concise, always-applied Claude Code rule surface for the root orchestrator. It and
`.claude/rules/language.md` are the only unconditionally loaded rules; the other rule documents
are conditionally loaded by their `paths:` frontmatter. Review briefings are loaded by the review
workflow and are not standing orchestrator instructions.

- Delegate implementation, planning, review-fix, and other specialist work through the capability
  CLI or provider wrapper; keep workflow control in the root session.
- Route all implementation work in this repository through a track workflow with Phase 0–3
  planning complete before implementation: use `/track:plan` for standalone feature planning,
  while end-to-end workflows such as `/track:adr2pr` own equivalent planning sequencing. Do not
  implement directly from a free-form request.
- Treat CLI summaries as the primary information for progress, review, obligation, and catalogue
  state. Open full artifact bodies only to inspect a diff or investigate a blocker. The `adr2pr`
  workflow's mandatory Step 0 is a bounded exception: read each sub-workflow definition it
  enumerates to build the execution plan before execution. This is required workflow planning,
  not general bulk intake.
- Do not run direct Git mutations. Use the guarded `/track:*`, `bin/sotp`, and `cargo make`
  workflows; read-only Git inspection is permitted.

When a task needs more detail, read the referenced rules: `.claude/rules/orchestration.md`,
`.claude/rules/guardrails.md`, `.claude/rules/dev-environment.md`, and
`.claude/rules/language.md`.
