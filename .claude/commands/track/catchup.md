---
description: Set up the development environment and catch up on project state.
---

Onboard a new contributor or refresh the local development environment.

This command combines environment setup with project context loading.

## Execution

### Phase 1: Environment bootstrap

Run `cargo make bootstrap` and monitor its output step by step.

If any step fails:
- Diagnose the root cause from the error output
- Suggest a concrete fix (e.g., install missing tool, fix Docker config)
- After the user applies the fix, rerun `cargo make bootstrap` (idempotent — completed steps finish instantly)

### Phase 2: Track workflow setup

After bootstrap succeeds, execute the full `/track:setup` command.
This delegates all setup checks to setup.md — do not duplicate them here.

### Phase 3: Project state briefing

Summarize the current project state for the newcomer:
1. Read `track/registry.md` — list active and completed tracks
2. Resolve the current track: if the current git branch matches `track/<id>`, use that track. Otherwise, use the latest materialized active track (non-archived, non-done, `branch != null`). If no materialized active track exists, fall back to the latest branchless planning-only track.
3. Read the current track's `spec.md` and `plan.md` if one exists
4. Read `track/tech-stack.md` — highlight any unresolved `TODO:` items
5. Show recent git log (last 10 commits) for context
6. Read `knowledge/conventions/README.md` — list active convention docs

### Phase 4: External guides setup (optional)

If `knowledge/external/guides.json` has entries:
1. Run `cargo make guides-fetch` to download cached guides
2. Report which guides were fetched and their purpose

## Output format

1. Environment status: each bootstrap step (pass/fail)
2. Track workflow status: initialized / already set up
3. Project briefing:
   - Active tracks and their status
   - Current tech stack decisions
   - Active conventions
   - Recent commit history (1-line summary)
4. External guides: fetched / none configured
5. Suggested next actions for the newcomer
