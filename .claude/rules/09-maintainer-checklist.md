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

Keep workflow SSoT, thin command adapters, maintainer guidance, user guidance, and the affected
conventions synchronized. Do not change `.harness/config/signal-gates.json` or adr_user
evaluation when adding an independent ADR-baseline gate.

After such changes, run `cargo make ci` and, for a track-aware gate, `cargo make ci-track`.
