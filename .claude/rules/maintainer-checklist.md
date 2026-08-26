---
paths:
  - ".harness/**"
  - ".claude/**"
  - ".agents/**"
  - ".codex/**"
  - ".gemini/**"
  - "knowledge/conventions/**"
  - "architecture-rules.json"
  - "README.md"
  - "CLAUDE.md"
  - "AGENTS.md"
---

# Maintainer Checklist

This conditionally loaded checklist is for maintainers only; it is not an always-applied
orchestrator rule or PR-review briefing.

When changing workflow or architecture, update all affected layers together.

Always consider:

- user-facing docs:
  - `README.md`
- track docs:
  - `.harness/policies/branch-strategy.md`
  - `.harness/policies/track-lifecycle.md`
  - `.harness/policies/git-notes.md`
  - `track/registry.md`
- enforcement:
  - `Makefile.toml`
  - ADR-baseline entry points: init snapshot, review `check-review`, guarded-commit and
    `ci-track` `check-commit`
  - `sotp verify` subcommands (Rust CLI, replaces deleted `scripts/verify_*.py`)
  - `.claude/settings.json` (Rust hook entries: `skill-compliance`, `block-direct-git-ops`, `block-test-file-deletion` — dispatched via `bin/sotp hook dispatch ...`)

When revising a reviewer briefing, verify that every event prohibited by its role statement has a reporting route: a reportable category, or, for a half-open briefing, an explicit rule that role-statement violations are always reportable.

When changing workspace architecture, synchronize this same live architecture-document set:

- `CLAUDE.md`, `AGENTS.md`, this checklist, and `.claude/skills/architecture-customizer/SKILL.md`
- `.harness/capabilities/{implementer,dry-fix-lead,review-fix-lead,rollback-diagnoser}.md`
- `.harness/custom/review-prompts/{cli,cli_composition,cli_driver,domain,infrastructure,types,usecase}.md`
- survey prompts: `.gemini/GEMINI.md`, `.claude/skills/{gemini-system,repomix-snapshot}/SKILL.md`
- applicable `knowledge/conventions/` references, especially `coding-principles.md` and
  `type-designer-kind-selection.md`
- `.harness/catalogue-lint/config.json` and `.harness/catalogue-lint/presets/ddd-strict.json`
  when layer ids change: every role's `KindLayerConstraint` matches layer ids literally, and
  the two files must stay structurally equal

Keep workflow SSoT, thin command adapters, maintainer guidance, user guidance, and the affected
conventions synchronized. Do not change `.harness/config/signal-gates.json` or adr_user
evaluation when adding an independent ADR-baseline gate.

After such changes, run `cargo make ci` and, for a track-aware gate, `cargo make ci-track`.
