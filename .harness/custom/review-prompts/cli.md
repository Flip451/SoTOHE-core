# CLI (Thin-Bin) Layer Review: Severity Policy

The reviewer's role is **thin-bin boundary review** of `apps/cli/`
(`src/main.rs` and `src/commands/*.rs`). The bin layer does exactly three things:
parse arguments into typed `Input` structs, obtain a wired Driver from
`cli_composition`, and call `driver.handle(input)` to receive a `CommandOutcome`
which it emits (stdout/stderr + `ExitCode`). It must not contain business logic,
perform direct I/O for application purposes, or import from `usecase`/`domain`
directly. Cite ADR D5/D8 (`2026-06-21-1328-cli-composition-split-presentation-layer.md`)
and `2026-05-27-0110-composition-root-dedicated-crate.md` D3.

## What to report

Report findings ONLY for the following categories:

- **business logic in bin**: a calculation, branching decision, domain
  transformation, or multi-step orchestration in `src/main.rs` or a
  `src/commands/*.rs` handler. The handler must be a thin parse-dispatch-emit
  wrapper. Computations that belong in `usecase` or `domain` must not be
  inlined here. Cite ADR D5 and `hexagonal-architecture.md` §CLI as
  Composition Root.
- **direct I/O in bin**: production code in `apps/cli/` that writes to `stdout`
  / files / the network for application purposes (e.g., telemetry persistence via
  `std::fs` / `serde_json` / `chrono::Utc::now()` inlined in `main.rs`).
  Permitted: `eprintln!` for fatal startup errors before a Driver is available,
  and `emit(outcome)` which routes `CommandOutcome.stdout` / `.stderr` to the
  appropriate streams. Application-level I/O must flow through a Driver. Cite
  ADR D8.
- **usecase/domain import in production**: non-test code in `apps/cli/` that
  imports from `usecase::*` or `domain::*` directly (bypassing `cli_composition`
  or `cli_driver` re-exports). The bin's only knowledge of the application must
  come through `cli_composition::*` (wiring) and `cli_driver::*` (typed Inputs /
  `CommandOutcome`). Test-only imports (`#[cfg(test)]`) are excluded. Cite ADR D5
  and `architecture-rules.json` (`cli` may depend only on `cli_composition` and
  `cli_driver`).
- **driver coordinate in bin violated**: a command handler that invokes a
  use-case interactor directly (bypassing the Driver) or reaches into
  `cli_composition` to call a wiring function and then invokes the resulting
  interactor itself. Argument parsing should produce a typed `Input`; the bin
  calls `driver.handle(input)` and emits the `CommandOutcome`. Cite ADR D5.
- **ExitCode mapping inconsistency**: a command that on the unhappy path prints
  an error but returns `ExitCode::SUCCESS` (or vice-versa). Scripted callers
  will misinterpret the result.
- **inconsistent output for `--json` / `--quiet` flag**: a command that ignores
  its own output-mode flag, or emits diagnostics to stdout instead of stderr
  (breaks piped consumption).

## What NOT to report

- Subcommand naming (`--scope` vs `--group`) when the existing name is documented
  in `.claude/commands/` or `README.md`
- `clap` derive vs builder — both are valid project styles
- Output color / formatting choices that do not affect correctness or flag
  consistency
- "You could use `?` here" suggestions when the surrounding code uses an explicit
  `match` for context attachment
- Restructuring the `commands/` directory layout
- Help-text wording suggestions
- `unwrap()` / `expect()` inside `#[cfg(test)]` blocks
