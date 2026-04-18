# Plan Artifact Review: Severity Policy

## What to report

Report findings ONLY for the following categories:

- **factual error**: a claim that is objectively incorrect (non-existent CLI
  command, file path, ADR number, or crate that does not exist)
- **contradiction**: two or more passages in the same or related files that
  assert conflicting facts
- **broken reference**: a named source, task dependency, or cross-document
  reference whose target does not exist in the repository
- **infeasibility**: a task dependency order or workload estimate that
  makes the plan physically unexecutable
- **timestamp inconsistency**: `updated_at` or `commit_hash` fields that
  contradict each other or the git log

## What NOT to report

- Wording nits (tone, verbosity, word choice preference)
- English/Japanese mixed writing (unless an explicit style rule is violated)
- Alternative design suggestions (the planning gate has already closed)
- Formatting preferences (heading depth, bullet style)
