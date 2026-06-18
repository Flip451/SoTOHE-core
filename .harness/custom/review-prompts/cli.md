# CLI Layer Review: Severity Policy

The reviewer's role is **user-facing boundary review** of `apps/cli/`.
CLI is a thin delivery adapter: it parses arguments, delegates wiring and
application execution through `cli_composition`, and maps results to
output + `ExitCode`. Adapter construction / DI belongs in
`apps/cli-composition/`, not in CLI command handlers. Within `apps/cli/`,
direct `std::fs`, `std::env`, and process spawning are allowed only for
delivery-boundary tasks such as argument / environment intake and
process-level output; wiring-time I/O inside `cli_composition` is reviewed
under the `cli_composition` policy. Neither layer may use boundary I/O for
application logic or adapter implementation.

## What to report

Report findings ONLY for the following categories:

- **application logic in CLI**: a calculation, branching decision, or
  domain transformation that belongs in `usecase` or `domain` performed
  in a `commands/` handler. The handler should be a thin parse-dispatch
  wrapper. Cite `hexagonal-architecture.md` §CLI as Composition Root.
- **composition bypass**: a `commands/` handler that imports or calls
  `infrastructure::*` / `domain::*` directly for application behavior or
  adapter construction instead of delegating through `cli_composition`.
  Wiring mistakes inside the delegated builders belong to the
  `cli_composition` review; bypassing those builders from CLI is reportable
  here. Cite `architecture-rules.json` (`cli` may depend only on
  `cli_composition`).
- **missing fail-closed at boundary**: a command that accepts a flag /
  path / id without validating it, then propagates an unwrapped value
  deep into infrastructure where the failure is observed late. Validate
  at the CLI boundary via Newtype `try_new` or explicit argument
  validation.
- **panic on user input**: a clap parse path that can panic
  (`from_str().unwrap()`) on a user-provided value. Use `value_parser` /
  `try_from_str` and map errors to a `Result<_, ExitCode>`.
- **ExitCode mapping inconsistency**: a command that on the unhappy path
  prints an error but returns `ExitCode::SUCCESS` (or vice-versa) —
  scripted callers will mis-interpret.
- **inconsistent output for `--json` / `--quiet` flag**: a command that
  ignores its own output-mode flag, or emits diagnostics to stdout
  instead of stderr (breaks piped consumption).
- **secret leak in error output**: an error message that includes a
  full path containing a username / a token from a config file in
  the unhappy-path printout. Cite `security.md`.

## What NOT to report

- Subcommand naming (`--scope` vs `--group`) when the existing name is
  already documented in `.claude/commands/` / `README.md`
- `clap` derive vs builder — both are valid project styles
- Output color / formatting beyond the points above
- "You could use `?` here" suggestions when the surrounding code uses an
  explicit `match` for a reason (e.g., to attach context)
- Restructuring `commands/` directory layout
- Help-text wording suggestions
