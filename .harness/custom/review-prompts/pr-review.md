# Project PR-Review Focus and Severity Policy

Apply the [framework PR-reviewer methodology](../../prompts/pr-reviewer.md) before applying the
project-specific policy below. The framework prompt is the authority for shared correctness,
safety, testing, security, whole-PR consistency, dead-reference, and finding-boundary checks.
This file deliberately keeps only project context and severity policy; it does not duplicate the
shared methodology.

## Project focus

- Apply the shared methodology against the repository's current source-of-truth contracts and
  the layer boundaries declared in `architecture-rules.json`.

## Severity Policy

Only report findings at these severity levels:

- **P0** (CRITICAL / HIGH): Logic errors, security vulnerabilities, data corruption risks,
  panics in library code.
- **P1** (MEDIUM): Missing error handling, test coverage gaps, architecture violations.

Do **not** report:

- **LOW**: Style preferences, minor naming suggestions.
- **INFO**: Cosmetic issues, documentation style.

Focus on the project contracts covered by this policy and be concise.
