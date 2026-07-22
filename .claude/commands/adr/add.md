---
description: Author or amend an Architecture Decision Record (ADR) through an interactive hearing.
---

> Operational SSoT: `.harness/workflows/adr/add.md` — provider-independent workflow behavior lives there.
> This file is the Claude Code adapter and defines only invocation, tool constraints, and report format.

## Invocation

User invokes this command as `/adr:add`.
`$ARGUMENTS` may be empty, a topic phrase, or an ASCII kebab-case slug.
Pass the supplied value to the workflow SSoT for resolution.

## Claude Code invocation constraints

- Use `AskUserQuestion` for the interactive hearing.
- Use Claude Code file tools to read and write the ADR only as directed by the workflow SSoT.
- Do not run `git add`, `git commit`, or `git push` directly.

## Report format

After execution, summarize:

1. The generated or amended ADR file path.
2. Included sections and explicitly skipped sections.
3. Whether the user requested an ADR-index update.
4. Suggested next commands: `/track:plan <feature>` or `/adr:add` in Focused mode.
