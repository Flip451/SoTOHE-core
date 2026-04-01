---
description: Add a new project convention document and update the conventions index.
---

Create a new document under `knowledge/conventions/` and keep the conventions index in sync.

Execution rules:
- First read `knowledge/conventions/README.md`.
- Ask only for missing values. Minimum required input is the convention name.
- If the user gives a free-form title or non-ASCII name, suggest and confirm an ASCII kebab-case slug separately.
- Suggest a concise title and one-line purpose summary if the user did not provide them.
- Prefer `cargo make conventions-add -- <name> --slug <slug> --title <title> --summary <summary>` when a separate slug is needed.
- After creation, immediately read the generated convention document and identify remaining `TODO:` placeholders.
- Ask focused follow-up questions to fill the initial body sections (`Applies to`, `Does not apply to`, rules, examples, exceptions, review checklist, references).
- Replace the generated `TODO:` placeholders in the same turn whenever the user provides enough information. Do not stop right after file creation if the body is still a raw scaffold.
- After creation, run:
  - `cargo make conventions-verify-index`
  - `cargo make verify-arch-docs`
- If the new convention changes user-facing workflow guidance, update:
  - `CLAUDE.md`
  - `DEVELOPER_AI_WORKFLOW.md`
  - `.codex/instructions.md`
  only when actually needed.

Output format:
1. Added file
2. README index update
3. Validation results
4. Next suggested step
