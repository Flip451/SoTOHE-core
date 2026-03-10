---
description: Add a new external long-form guide entry interactively.
---

Register a new external guide in `docs/external-guides.json` with minimal user effort.

Execution rules:
- First read `docs/EXTERNAL_GUIDES.md` and `docs/external-guides.json`.
- Ask the user only for missing values. Minimum required fields are:
  - source URL
  - title
  - license
- Suggest defaults for:
  - `id`: kebab-case short identifier derived from title or source path
  - `raw_url`: convert GitHub `blob` URL to `raw.githubusercontent.com` when possible
  - `cache_path`: `.cache/external-guides/<id>.<ext>`
- Ask for optional metadata:
  - trigger keywords
  - summary bullets
  - project usage bullets
- Before writing, show the final normalized entry to the user and get confirmation.
- After confirmation, update `docs/external-guides.json`.
- Then run:
  - `cargo make guides-list`
- Offer `cargo make guides-fetch <id>` as the next step, but do not require it if network is unavailable.

Suggested implementation path:
- Prefer `cargo make guides-add ...` to write the normalized entry.
- When showing a shell example, quote multi-word scalar values such as title, summary, and project usage.

Example:

```bash
cargo make guides-add -- \
  --id pg-guide \
  --title "PostgreSQL Guide" \
  --source-url "https://github.com/example/repo/blob/main/docs/postgres.md" \
  --license "CC-BY-4.0" \
  --trigger postgres \
  --summary "Use for schema review" \
  --project-usage "Check before changing SQL conventions"
```

Output format:
1. Added guide id
2. Fields written
3. Validation run
4. Next suggested step
