# Framework PR-Reviewer Methodology

This is the framework-owned methodology for every automated pull-request reviewer. It is
loaded through the project PR-review entry point; it is not root-orchestrator standing
instructions. The project-owned prompt at
`.harness/custom/review-prompts/pr-review.md` adds repository-specific focus and severity
policy and must be read together with this document.

## Review boundary

Review the complete PR as the final branch state against its base, and report only concrete,
actionable findings supported by that PR. A finding must include the affected `path:line`, the
impact or evidence, and a focused correction. Do not report style preferences, cosmetic issues,
or speculative concerns as findings. Do not change a reviewer verdict to hide an out-of-scope
finding, and do not treat an accepted deviation as the absence of a finding.

## Correctness and safety

- **No panics in library code**: `unwrap()`, `expect()`, `panic!()`, `todo!()`, and
  `unreachable!()` are forbidden outside `#[cfg(test)]`. Use `?` and `Result` types. The single
  bounded exception is a static literal secret-pattern regex at a secrecy boundary: construct
  it as `LazyLock<Regex>`, use an `expect` with a line-scoped allow annotation, and add a
  construction-validation test. An invalid static pattern is a programming error and must
  fail-stop; do not extend this exception to other `expect()` or `unwrap()` calls.
- **Make illegal states unrepresentable**: use validated domain types instead of raw primitives
  for domain concepts.
- **Error handling**: propagate errors with `Result<T, E>` and `?`. An explicit exhaustive
  `match` is also valid for typed conversion or error-context attachment when every branch
  returns a `Result` and no error is discarded. Do not silently swallow errors.
- **Trait-based abstraction**: keep infrastructure dependencies behind trait boundaries.
- **Module size**: use the `module_limits` in `architecture-rules.json` (`warn_lines` /
  `max_lines`) and judge production code only. Test code is exempt from the module limit.
- **Unsafe code**: keep it minimal, require a `// Safety:` justification, and review it.

## Testing

- Require happy-path and error-case tests for public APIs, unless the corresponding
  test-obligation record has a reasoned waiver that passes the repository's obligation gate.
- Tests must be independent and must not depend on execution order.
- Use deterministic fakes for external dependencies by default. Use a mock when the interaction
  itself is the specification, such as call count, order, arguments, retry, timeout, or
  cancellation behavior.
- Use the repository's test naming convention: `test_{target}_{condition}_{expected_result}`.

## Security

- Do not hardcode secrets; use environment variables with proper error propagation.
- Validate all external input through domain types.
- Use parameter binding for SQL queries; do not interpolate SQL strings.
- Do not expose internal details such as hosts, ports, or stack traces in error messages.
- Do not place sensitive information in logs.

## Whole-PR consistency

These checks require the whole PR branch and may be missed by a reviewer scoped to one file or
one commit.

- **Branch-wide consistency** — read all file creations, deletions, edits, renames, and reference
  updates as one change set. Confirm that the final branch state is coherent. Flag, for example,
  a new file whose reference was deleted elsewhere in the PR, a renamed function with a caller
  that still uses the old name, a removed configuration key that is still read, or a documentation
  link that disagrees with a move made elsewhere in the PR.
- **Cross-commit consistency** — walk the PR commit by commit. Confirm that later commits still
  honor constraints and assumptions introduced earlier, and that tests are updated when later
  commits change the behavior they assert.
- **Dead-reference check** — after applying the PR's deletions and renames, scan surviving files
  for references to paths, imports, commands, fixtures, or anchors that no longer exist. Do not
  classify references inside ADR, convention, commit-message, or git-note text that explicitly
  records what the PR changed as dead references; those are intentional historical records.

## Finding report

Report findings only when the evidence lies within the review boundary and the issue is actionable
at the configured project severity. For each finding, state the severity, `path:line`, concrete
behavior or risk, and the smallest reasonable correction. If no reportable finding remains, use
the reviewer's explicit no-findings signal. A user-approved Accepted Deviations outcome remains a
separate outcome and must not be relabeled as `zero findings`.
