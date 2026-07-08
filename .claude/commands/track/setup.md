---
description: Initialize the local track workflow foundation.
---

Run track workflow setup for this repository.

Execution rules:
- Verify that `bin/sotp` is available because track operations depend on it (run `cargo make build-sotp` if missing).
- Read `knowledge/conventions/branch-strategy.md`, `knowledge/conventions/track-lifecycle.md`, `knowledge/conventions/git-notes.md`, and `knowledge/adr/README.md` (pre-track ADR index).
- Ensure `track/registry.md` exists; if missing, create it with a minimal template list section.
- Ensure the track convention includes `track/items/<id>/metadata.json` alongside `spec.md` and `plan.md`; `observations.md` is optional (created only when machine-non-verifiable observations need recording).
- Confirm the required top-level doc `CLAUDE.md` exists.
- Do not start implementation work in this command.
- Summarize what was initialized.

Output format:
1. Setup status (done / already initialized)
2. Commands checked or executed
3. Files checked or created
4. Next required user actions (e.g., authoring a pre-track ADR for the first feature)
