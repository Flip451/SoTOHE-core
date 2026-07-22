---
name: adr-add
description: Use when Codex is asked to author or amend an Architecture Decision Record through an interactive hearing.
---

# ADR-Add (Codex skill)

**Operational SSoT:** read and follow `.harness/workflows/adr/add.md` — the provider-agnostic workflow contract for this skill.
Do not duplicate step sequence, gate conditions, state transitions, or failure-recovery procedures here.

## Codex-skill notes

### (1) Invocation surface

- Triggered via `$adr-add` in a Codex skill mention surface.
- Can also be force-loaded with `codex exec` by referencing this skill file.
- An optional topic phrase or ASCII kebab-case slug may follow the skill name.

### (2) Tool constraints

- Requires `--sandbox workspace-write`: the workflow writes an ADR after the required user hearing.
- Use Codex's user-question surface for the interactive hearing.
- Do not run `git add`, `git commit`, `git push`, or any direct Git state-changing command.

### (3) Reporting format

- On successful completion, print: `ADR_ADD_STATUS: completed — <adr-path>; included: <sections>; skipped: <sections>`.
- On failure or block, print: `ADR_ADD_STATUS: blocked — <reason>`.
