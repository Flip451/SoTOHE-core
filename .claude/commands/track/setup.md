---
description: Initialize the local track workflow foundation.
---

Run track workflow setup for this repository.

Execution rules:
- Verify that `python3` is available because Claude hooks and external guide tasks depend on it.
- Read `track/workflow.md`, `track/tech-stack.md`, `track/product.md`, `track/product-guidelines.md`.
- Read `knowledge/external/POLICY.md` and `knowledge/external/guides.json`.
- Ensure `track/registry.md` exists; if missing, create it with a minimal template list section.
- Ensure the track convention includes `track/items/<id>/metadata.json` and `verification.md` alongside `spec.md` and `plan.md`.
- Confirm required top-level docs exist (`CLAUDE.md`, `knowledge/WORKFLOW.md`, `knowledge/DESIGN.md`).
- Strict tech-stack guardrails are on by default. Only template maintainers may disable them locally for template work via `TRACK_TEMPLATE_DEV=1` or an untracked `.track-template-dev` marker.
- Do not start implementation work in this command.
- Summarize what was initialized and what TODO items must be filled next.

Output format:
1. Setup status (done / already initialized)
2. Commands checked or executed
3. Files checked or created
4. Next required user actions (especially unresolved `TODO:` in `track/tech-stack.md`)
