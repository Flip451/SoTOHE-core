# AGENTS.md

Guidelines for automated code reviewers (e.g., Codex Cloud `@codex review`).

## Prerequisites

- **Codex Cloud GitHub App** must be installed on this repository.

## Review Guidelines

When reviewing pull requests, focus on the following areas:

### Coding Principles

- **No panics in library code**: `unwrap()`, `expect()`, `panic!()`, `todo!()`, `unreachable!()` are forbidden outside `#[cfg(test)]`. Use `?` operator and `Result` types.
- **Make illegal states unrepresentable**: Use validated domain types (e.g., `Email(String)`) instead of raw primitives.
- **Error handling**: All errors must propagate via `Result<T, E>` with the `?` operator. No silent error swallowing.
- **Trait-based abstraction**: Infrastructure dependencies must be behind trait boundaries (hexagonal architecture).
- **Module size**: Aim for 200-400 lines per module, 700 max. The size limit applies to **production code only**; test code (`#[cfg(test)] mod tests` blocks, `*_tests.rs` files, `tests/` integration tests) is **exempt** and must be excluded when measuring a module against the limit. Count only the non-test lines (the code above the `#[cfg(test)]` block).
- **Unsafe code**: Must be minimal, commented with `// Safety:` justification, and reviewed.

### Testing

- Happy path and error case tests required for all public APIs.
- Tests must be independent (no execution order dependency).
- External dependencies (DB, API) must be mocked.
- Test naming: `test_{target}_{condition}_{expected_result}`.

### Security

- No hardcoded secrets. Use environment variables with proper error propagation.
- All external input must be validated via domain types.
- SQL queries must use parameter binding (no string interpolation).
- Error messages must not leak internal details (host, port, stack traces).
- Logs must not contain sensitive information.

### PR-Specific Review Angles

These angles only become visible when reviewing the whole PR branch against the base. Local
reviewers that scope to a single file or commit cannot catch them; surface findings here when
the PR-level view shows a problem the per-commit view missed.

- **Branch-wide consistency** — Read the full set of file deletions, creations, edits, and
  reference updates across the PR as a single change set. Ask: does the final state of the
  branch (HEAD vs. base) form a coherent whole? Examples to flag:
  - A new file is created and referenced from elsewhere, but a different commit moved or
    deleted the referencing file.
  - A function is renamed in one commit and a different commit adds a new caller using the
    old name.
  - A configuration key is removed in one commit and another commit still reads it.
  - A documentation file says "see X" where X is created or renamed elsewhere in the PR
    inconsistently.
- **Cross-commit consistency** — Walk the PR commit by commit. Ask: do changes introduced in
  earlier commits remain consistent with the assumptions of later commits? Examples to flag:
  - An early commit changes a public API signature; a later commit's code still passes the
    old shape (compiles only because the later commit also adjusts something accidentally
    masking the mismatch).
  - An early commit's test asserts behavior X; a later commit changes the implementation to
    behavior Y without updating the test (the test happens to still pass because of an
    unrelated condition).
  - An early commit introduces a constraint or invariant that a later commit silently
    violates.
- **Dead-reference check** — After the PR's deletions and renames are applied, scan the
  surviving files (those still present at HEAD) for references to files that no longer
  exist. Includes: source file paths in docs, import paths in code, command invocations in
  scripts, anchor links in markdown. Examples to flag:
  - A markdown file says "see `path/to/old.md`" but `old.md` is deleted in this PR.
  - A `cargo make` task references a script that this PR removed.
  - A test fixture path points at a file the PR moved without updating the test.
  - The PR description or commit message says "as documented in X" where X was deleted.

  Exclusion: references to deleted files inside ADR / convention / commit-message / git-note
  text that describe *what this PR did* (e.g., "this PR deletes `old.md` because ...") are
  intentional historical records, not dead references.

### Severity Policy

Only report findings at these severity levels:

- **P0** (CRITICAL / HIGH): Logic errors, security vulnerabilities, data corruption risks, panics in library code.
- **P1** (MEDIUM): Missing error handling, test coverage gaps, architecture violations.

Do **not** report:

- **LOW**: Style preferences, minor naming suggestions.
- **INFO**: Cosmetic issues, documentation style.

Focus on correctness and safety. Be concise.
