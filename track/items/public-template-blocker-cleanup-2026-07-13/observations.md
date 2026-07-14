# Machine-path cleanup batch observations

## T015 — batch 1

- Target files: 21
- Resolved: 450 source-line findings
- Verifier command: `bin/sotp verify machine-paths --project-root .`
- Before: FAILED (91 file-level findings; 2,335 source-line findings).
- After: FAILED (1,885 source-line findings remain for the next deterministic batch).
- Normalization retained repository-local paths as repo-relative values and replaced the one already-redacted historical machine-home value with a generic machine-home notation.

## T016 — batch 2

- Target files: 22
- Resolved: 450 source-line findings
- Verifier command: `bin/sotp verify machine-paths --project-root .`
- Before: FAILED (1,885 source-line findings remain).
- After: FAILED (1,435 source-line findings remain for the next deterministic batch).

## T017 — batch 3

- Target files: 26
- Resolved: 450 source-line findings
- Verifier command: `bin/sotp verify machine-paths --project-root .`
- Before: FAILED (1,435 source-line findings remain).
- After: FAILED (985 source-line findings remain for the next deterministic batch).

## T018 — batch 4

- Target files: 16
- Resolved: 450 source-line findings
- Verifier command: `bin/sotp verify machine-paths --project-root .`
- Before: FAILED (985 source-line findings remain).
- After: FAILED (535 source-line findings remain for the next deterministic batch).

## T019 — batch 5

- Target files: 6
- Resolved: 450 source-line findings
- Verifier command: `bin/sotp verify machine-paths --project-root .`
- Before: FAILED (535 source-line findings remain).
- After: FAILED (85 source-line findings remain for the final batch).
- Note: the final inventory is one line above the planning estimate because the baseline included one historical redacted machine-home value in addition to the 2,334 canonical workspace-path lines.

## T020 — batch 6

- Target files: 5
- Resolved: 85 source-line findings
- Verifier command: `bin/sotp verify machine-paths --project-root .`
- Before: FAILED (85 source-line findings remain; one above the plan's approximate 84-line residual for the reason recorded in T019).
- After: PASSED (zero file-level violations and zero source-line findings).
