Push the current branch and create (or reuse) a PR in one step.

## Execution

Run:

```bash
bin/sotp pr push
bin/sotp pr ensure-pr
```

This executes `sotp pr push` followed by `sotp pr ensure-pr`.

- On `track/<id>` branches: auto-resolves the track ID from the branch name.
- On any other branch: stop and report that `/track:pr` requires the current track branch.

## Behavior

After execution, report:
1. Push result
2. PR number and URL (created or reused)
3. Recommended next command: `/track:merge <pr>` or `/track:pr-review`
