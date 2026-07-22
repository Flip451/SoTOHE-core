# CLI Driver Layer Review: Severity Policy

The reviewer's role is **primary-adapter correctness review** of `apps/cli-driver/`.
`cli_driver` is the primary (driving) adapter layer: it holds injected use-case
interactors, translates typed `Input` enums into use-case commands, invokes exactly
one interactor per request, and renders the result into a `CommandOutcome`. DI belongs
in `cli_composition`, not here. Both invoke and render live in the same layer.

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

- **adapter performs DI**: a Driver constructor that calls `Arc::new(...)` /
  instantiates adapters / constructs use-case interactors itself, rather than
  receiving them via constructor injection. `cli_driver` is the _injected_ side;
  object-graph construction belongs in `cli_composition`. Cite
  `type-designer-kind-selection.md` R1 (`PrimaryAdapter`).
- **business logic in adapter**: a `handle` method (or helper it calls) that
  contains validation rules, domain decisions, multi-step orchestration beyond
  `input → invoke → render`, or any calculation that belongs in `usecase` or
  `domain`. Orchestrating multiple use cases is a composition/usecase concern;
  a Driver calls exactly one interactor per request. Cite
  `coding-principles.md` §Usecase Layer Purity.
- **non-CommandOutcome return**: a public `handle` or equivalent method whose
  return type is anything other than `CommandOutcome`. Errors are part of the
  rendered output — map them to `CommandOutcome.stderr` with an appropriate
  exit-code signal rather than propagating `Result<_, _>` to the caller.
- **handle not delegating to single use case**: a `handle` method that invokes
  two or more separate interactor calls in sequence or branches between interactors
  depending on runtime state. Multi-interactor orchestration must be extracted into
  a usecase application service, not inlined in the Driver. A Driver may call
  render-only helpers (formatters, table builders) freely — those are not
  interactor calls.

## What NOT to report

- Naming of Driver structs (`GuardDriver` vs `HookGuardDriver`) when the existing
  name is consistent with adjacent crates
- Adding a secondary `render_*` private helper to the same module — render helpers
  are explicitly in-layer
- Refactoring an existing `handle` into multiple private methods that together
  form one invoke→render flow
- `unwrap()` / `expect()` inside `#[cfg(test)]` blocks
- Output color / table formatting style choices that do not affect correctness
- Suggestions to split a Driver into sub-Drivers when the current structure
  is one use-case per Driver
