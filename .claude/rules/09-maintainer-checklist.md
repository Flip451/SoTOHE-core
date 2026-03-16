# Maintainer Checklist

When changing workflow or architecture, update all affected layers together.

Host prerequisite:

- `python3` is optional on host (advisory hooks gracefully skip when absent); required inside Docker for CI

Always consider:

- user-facing docs:
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.claude/docs/WORKFLOW.md`
- track docs:
  - `track/workflow.md`
  - `track/tech-stack.md`
  - `track/registry.md`
  - `TRACK_TRACEABILITY.md`
- enforcement:
  - `Makefile.toml`
  - `sotp verify` subcommands (Rust CLI, replaces deleted `scripts/verify_*.py`)
  - `scripts/track_schema.py` / `track_state_machine.py` / `track_markdown.py`
  - `.claude/settings.json`
  - `.claude/hooks/`
  - `scripts/external_guides.py`

After such changes, run `cargo make ci`.
