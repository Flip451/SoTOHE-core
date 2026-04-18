# Plan Artifact Review: Severity Policy

The reviewer's role is **semantic consistency review** — does the plan make
sense, are the claims coherent, are the design trade-offs valid. **Mechanical
fact verification** (commit hash existence, git log consistency, whether a
test name exists in a given file, whether a file path exists on disk) is the
responsibility of `cargo make ci` / `sotp verify ...` / `cargo make
verify-*` / `cargo make track-check-approved`, not the reviewer.

## What to report

Report findings ONLY for the following categories:

- **factual error (semantic)**: a claim that is semantically wrong in a way
  a reader would notice (non-existent CLI command description, mis-named
  pattern, wrong layer placement). If the only way to verify the claim is
  to run `git log` / `grep` the codebase for an exact symbol, that is CI's
  job — do not flag it.
- **contradiction**: two or more passages in the same or related files that
  assert conflicting design intent or requirements
- **broken reference**: a named design decision, ADR, or cross-document
  reference that is self-evidently wrong at the narrative level (e.g.,
  "see ADR-999" when that ADR is clearly not part of this project's design).
  Do NOT check whether a file path, symbol, or document physically exists on
  disk — that is `cargo make verify-doc-links` / CI responsibility.
- **infeasibility**: a task dependency order or workload estimate that
  makes the plan physically unexecutable

## What NOT to report

- Wording nits (tone, verbosity, word choice preference)
- English/Japanese mixed writing (unless an explicit style rule is violated)
- `updated_at` / `commit_hash` / timestamp verification against git log
  (CI / `sotp verify` / `track-check-approved` handle this deterministically)
- Existence checks for symbols, tests, or paths that CI already verifies
- Backward-looking metrics (scope file counts, round counts, cumulative
  history) whose absence doesn't affect whether the feature works
- Alternative design suggestions (the planning gate has already closed)
- Formatting preferences (heading depth, bullet style)
