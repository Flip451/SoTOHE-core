# Maintainer Checklist

When changing workflow or architecture, update all affected layers together.

Always consider:

- user-facing docs:
  - `README.md`
- track docs:
  - `knowledge/conventions/branch-strategy.md`
  - `knowledge/conventions/track-lifecycle.md`
  - `knowledge/conventions/git-notes.md`
  - `track/tech-stack.md`
  - `track/registry.md`
- enforcement:
  - `Makefile.toml`
  - `sotp verify` subcommands (Rust CLI, replaces deleted `scripts/verify_*.py`)
  - `.claude/settings.json` (Rust hook entries: `skill-compliance`, `block-direct-git-ops`, `block-test-file-deletion` — dispatched via `bin/sotp hook dispatch ...`)

After such changes, run `cargo make ci`.
