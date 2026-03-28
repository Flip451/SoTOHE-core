# Reinvention Check: workflow concern

## Context
The "workflow" concern category was triggered during review of TSUMIKI-03
differential hearing implementation in SKILL.md (a prompt/instruction file,
not executable Rust code).

## Survey
Not applicable — the concerns are about prompt text accuracy in a markdown
skill definition file, not about library functionality. There are no crates
on crates.io that provide SKILL.md prompt templates.

## Decision
`continue_self` — fix the prompt text directly. No crate adoption needed.
