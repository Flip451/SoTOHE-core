# Maintainer Checklist

When changing workflow or architecture, update all affected layers together.

Host prerequisite:

- `python3` is required inside Docker for the `scripts/` Python helpers (architecture rules, convention docs, make wrappers); not directly required on the host because the workflow is invoked through Docker compose wrappers

Always consider:

- user-facing docs:
  - `DEVELOPER_AI_WORKFLOW.md`
- track docs:
  - `track/workflow.md`
  - `track/tech-stack.md`
  - `track/registry.md`
- enforcement:
  - `Makefile.toml`
  - `sotp verify` subcommands (Rust CLI, replaces deleted `scripts/verify_*.py`)
  - `.claude/settings.json` (Rust hook entries: `skill-compliance`, `block-direct-git-ops`, `block-test-file-deletion` — dispatched via `bin/sotp hook dispatch ...`)

After such changes, run `cargo make ci`.
