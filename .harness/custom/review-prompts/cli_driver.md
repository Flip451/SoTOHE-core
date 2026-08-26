# CLI Driver Layer Review: Severity Policy

The reviewer's role is **primary-adapter correctness review** of `apps/cli-driver/`.
`cli_driver` is the primary (driving) adapter layer: it holds injected use-case
ports, with one trait and one execute method per use case; it translates typed `Input`
enums into validated use-case `Command` / `Query` values (one parse, usecase-owned boundary types), invokes exactly one single-purpose port per request (or, for a stateless operation modeled as a `UseCaseFunction` entrypoint under the consumer-owned role/layer convention resolved through the `Current Files` index in `knowledge/conventions/README.md`, calls that use-case function directly — such an entrypoint has no port trait or Interactor),
and renders the result into a `CommandOutcome`. A Driver may hold several
single-purpose ports for different requests; multi-step behavior belongs behind one
use-case/application-service port. DI belongs in `cli_composition`, not here. Both
invoke and render live in the same layer.

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

- **external-boundary assumptions**: What external boundaries does the diff touch (OS, process, encoding, concurrency, resource limits, time, and other versions of its own artifacts)? Enumerate operations directly reached from the changed behavior. If a depended-on assumption is in neither the spec nor the environment-assumption declaration the project owns (the convention listed under `Current Files` in `knowledge/conventions/README.md` whose purpose is declaring environment assumptions; resolve it through that index, not a fixed filename), report it as `未宣言の前提への依存`; treat unresolvable indirect boundaries the same way rather than searching exhaustively.
- **adapter performs DI**: a Driver constructor that calls `Arc::new(...)` /
  instantiates adapters / constructs use-case interactors itself, rather than
  receiving them via constructor injection. `cli_driver` is the _injected_ side;
  object-graph construction belongs in `cli_composition`. Name the collaborator the
  constructor builds instead of receiving.
- **business logic in adapter**: a `handle` method (or helper it calls) that
  contains validation rules, domain decisions, multi-step business orchestration
  beyond `input → invoke one required port → render`, or any calculation that
  belongs in `usecase` or `domain`. A Driver may hold several injected
  single-purpose ports for different requests, but each request invokes exactly one
  port; multi-step behavior belongs behind one use-case/application-service port.
  Report the business rule or decision that the adapter owns and the layer that
  should own it.
- **non-CommandOutcome return**: a public `handle` or equivalent method whose
  return type is anything other than `CommandOutcome`. Errors are part of the
  rendered output — map them to `CommandOutcome.stderr` with an appropriate
  exit-code signal rather than propagating `Result<_, _>` to the caller.
- **handle bypassing use-case ports**: a `handle` method that implements use-case or
  domain behavior itself, or reaches infrastructure directly, instead of delegating
  through its injected use-case port(s). A Driver may invoke multiple injected
  single-purpose ports for different requests, but each request invokes exactly one
  port; report multi-step orchestration or decisions that belong in the usecase
  layer. A direct call to a `UseCaseFunction` entrypoint is not a bypass: it is
  that operation's sanctioned dispatch path until a port trait is introduced
  for it. A Driver may call render-only helpers (formatters, table builders)
  freely.
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
- Only for track `scope-conditional-pre-review-gates-2026-07-31`: the PRE-EXISTING
  completion-tracing telemetry adapter surface that arrived via the develop→track merge
  (`apps/cli-driver/src/telemetry.rs`) — a user adjudication (2026-08-13) deferred its
  architectural remediation (the `begin_completion` second public driver operation
  returning `TelemetryCompletion`, the adapter-owned `completion_eligible` string
  admission table, `CompleteCommand` runtime routing between `TelemetryArchivedService`
  and `TelemetryEmitService`, and the silent discarding of completion-emission errors)
  to a separate track. The merge-caused admission gap (display-only `phase explain`
  falling through as eligible) was fixed in-track as part of the same adjudication. Do
  not report those four deferred concerns on this pre-existing surface when the track's
  diff makes no semantic change to it beyond that admission-table entry. NEW driver
  operations or admission logic added by this track remain fully reportable.
- Only for track `scope-conditional-pre-review-gates-2026-07-31`: claiming that
  `match scope.state` / `matches!(round.round_type, ...)` in the review-results renderer
  moves non-Copy enums out of borrowed values. All matched variants are unit variants;
  matching a borrowed place against unit-variant patterns inspects the discriminant and
  binds no payload, so no move occurs. The workspace passes `cargo make clippy`
  (`-D warnings`) and `cargo make ci-rust` with this code as written; a rewrite to
  reference matching is cosmetic, not a correctness fix.
