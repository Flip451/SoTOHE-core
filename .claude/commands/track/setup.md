---
description: Initialize the local track workflow foundation.
---

Run track workflow setup for this repository.

Execution rules:
- Verify that `bin/sotp` is available because track operations depend on it. If missing, run the repository's provisioning task: `cargo make build-sotp` in the SoTOHE-core source repository, or `cargo make install-sotp` in an exported scaffold (check `cargo make --list-all-steps` when unsure which one exists).
- Read `.harness/policies/branch-strategy.md`, `.harness/policies/track-lifecycle.md`, `.harness/policies/git-notes.md`, and `knowledge/adr/README.md` (pre-track ADR index).
- Do not create `track/registry.md`. It is a gitignored generated view whose only writer is the renderer behind `bin/sotp track views sync`, and initializing the first track writes it; its absence in a fresh checkout is the normal state, not a setup gap.
- Ensure the track convention includes `track/items/<id>/metadata.json` alongside `spec.md` and `plan.md`; `observations.md` is optional (created only when machine-non-verifiable observations need recording).
- Confirm the required top-level doc `CLAUDE.md` exists.
- Do not start implementation work in this command.
- Summarize what was initialized.

Output format:
1. Setup status (done / already initialized)
2. Commands checked or executed
3. Files checked or created
4. Next required user actions (e.g., authoring a pre-track ADR for the first feature)
