# Maintainer Checklist

When changing workflow or architecture, update all affected layers together.

Host prerequisite:

- `python3` is required inside Docker for the `scripts/` Python helpers (architecture rules, atomic write, convention docs, external guides, track helpers); not directly required on the host because the workflow is invoked through Docker compose wrappers

Always consider:

- user-facing docs:
  - `DEVELOPER_AI_WORKFLOW.md`
  - `knowledge/WORKFLOW.md`
- track docs:
  - `track/workflow.md`
  - `track/tech-stack.md`
  - `track/registry.md`
  - `TRACK_TRACEABILITY.md`
- enforcement:
  - `Makefile.toml`
  - `sotp verify` subcommands (Rust CLI, replaces deleted `scripts/verify_*.py`)
  - `scripts/track_schema.py` / `track_state_machine.py` / `track_markdown.py`
  - `.claude/settings.json` (Rust hook entries: `skill-compliance`, `block-direct-git-ops`, `block-test-file-deletion` — dispatched via `bin/sotp hook dispatch ...`)
  - `scripts/external_guides.py`

After such changes, run `cargo make ci`.
