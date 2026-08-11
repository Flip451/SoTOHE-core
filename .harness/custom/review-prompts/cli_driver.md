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
  object-graph construction belongs in `cli_composition`. Name the collaborator the
  constructor builds instead of receiving.
- **business logic in adapter**: a `handle` method (or helper it calls) that
  contains validation rules, domain decisions, multi-step orchestration beyond
  `input → invoke → render`, or any calculation that belongs in `usecase` or
  `domain`. Orchestrating multiple use cases is a composition/usecase concern;
  a Driver calls exactly one interactor per request. Name the rule or decision the
  method carries and the layer that owns it.
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
- **boundary exposure violation**: a Driver may use usecase `Command`, `Query`,
  boundary DTO, and usecase `ValueObject` types in its public signatures for
  transport translation. Report direct domain `ValueObject` / `Entity` /
  `AggregateRoot` exposure,
  infrastructure type exposure, or transport-specific types leaking into the
  usecase boundary. Whether a given `ValueObject` is a domain type or a
  usecase-boundary type is a semantic judgment: state the evidence for the
  classification you assert. The role-only catalogue lint intentionally cannot make
  that distinction, so its silence is not a defence and its verdict is not the
  ground for this finding.

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
- Only for track `scope-conditional-pre-review-gates-2026-07-31`: claiming that
  `match scope.state` / `matches!(round.round_type, ...)` in the review-results renderer
  moves non-Copy enums out of borrowed values. All matched variants are unit variants;
  matching a borrowed place against unit-variant patterns inspects the discriminant and
  binds no payload, so no move occurs. The workspace passes `cargo make clippy`
  (`-D warnings`) and `cargo make ci-rust` with this code as written; a rewrite to
  reference matching is cosmetic, not a correctness fix.
