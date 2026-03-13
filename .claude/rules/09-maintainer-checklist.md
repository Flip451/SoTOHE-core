# Maintainer Checklist

When changing workflow or architecture, update all affected layers together.

Host prerequisite:

- `python3` is required for `.claude/hooks/*.py` and `scripts/external_guides.py`

Always consider:

- user-facing docs:
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.claude/docs/WORKFLOW.md`
- track docs:
  - `track/workflow.md`
  - `track/tech-stack.md`
  - `track/registry.md`
  - `TAKT_TRACK_TRACEABILITY.md`
- enforcement:
  - `Makefile.toml`
  - `scripts/verify_*.py`
  - `scripts/track_schema.py` / `track_state_machine.py` / `track_markdown.py`
  - `.claude/settings.json`
  - `.claude/hooks/`
  - `scripts/external_guides.py`
- takt definitions:
  - `.takt/config.yaml`
  - `.takt/pieces/`
  - `.takt/personas/`

After such changes, run `cargo make ci`.
