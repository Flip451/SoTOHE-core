Push the current branch and create (or reuse) a PR in one step.

Arguments:
- Use `$ARGUMENTS` as optional track-id (required on `plan/` branches, ignored on `track/` branches).

## Execution

Run:

```bash
cargo make track-pr $ARGUMENTS
```

This executes `sotp pr push` followed by `sotp pr ensure-pr`.

- On `track/<id>` branches: auto-resolves the track ID from the branch name. No argument needed.
- On `plan/<id>` branches: requires an explicit track-id argument (fail-closed).

## Behavior

After execution, report:
1. Push result
2. PR number and URL (created or reused)
3. Recommended next command:
   - For `track/` branches: `/track:merge <pr>` or `/track:pr-review`
   - For `plan/` branches: merge the PR, then `/track:activate <track-id>`
