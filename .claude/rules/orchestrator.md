# Orchestrator Rules

This is the concise, always-applied Claude Code rule surface for the root orchestrator. Review
briefings are loaded by the review workflow and are not standing orchestrator instructions.

- Delegate implementation, planning, review-fix, and other specialist work through the capability
  CLI or provider wrapper; keep workflow control in the root session.
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
