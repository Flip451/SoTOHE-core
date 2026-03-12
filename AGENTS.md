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
- **Module size**: Aim for 200-400 lines per module, 700 max.
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

### Severity Policy

Only report findings at these severity levels:

- **P0** (CRITICAL / HIGH): Logic errors, security vulnerabilities, data corruption risks, panics in library code.
- **P1** (MEDIUM): Missing error handling, test coverage gaps, architecture violations.

Do **not** report:

- **LOW**: Style preferences, minor naming suggestions.
- **INFO**: Cosmetic issues, documentation style.

Focus on correctness and safety. Be concise.
