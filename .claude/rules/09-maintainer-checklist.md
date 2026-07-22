# Maintainer Checklist

When changing workflow or architecture, update all affected layers together.

Always consider:

- user-facing docs:
  - `README.md`
- track docs:
  - `knowledge/conventions/branch-strategy.md`
  - `knowledge/conventions/track-lifecycle.md`
  - `knowledge/conventions/git-notes.md`
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
- applicable `knowledge/conventions/` references, especially `coding-principles.md`,
  `type-designer-kind-selection.md`, and `impl-delegation-arch-guard.md`

Keep workflow SSoT, thin command adapters, maintainer guidance, user guidance, and the affected
conventions synchronized. Do not change `.harness/config/signal-gates.json` or adr_user
evaluation when adding an independent ADR-baseline gate.

After such changes, run `cargo make ci` and, for a track-aware gate, `cargo make ci-track`.
