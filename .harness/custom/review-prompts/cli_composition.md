# CLI Composition Layer Review: Severity Policy

The reviewer's role is **pure-DI wiring correctness review** of
`apps/cli-composition/`. `cli_composition` is the composition root: it constructs
secondary adapters (from `infrastructure`), use-case interactors, and driving adapters
(from `cli_driver`), and hands the fully-wired drivers to `apps/cli`. It must **only
wire** — it must not invoke use cases, render output, or define adapter
implementations. Wiring errors (port-adapter mismatch, double-instantiation, panic on
config load) are in scope; application-logic and presentation concerns always belong
in `usecase` or `cli_driver`.

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

- **external-boundary assumptions**: What external boundaries does the diff touch (OS, process, encoding, concurrency, resource limits, time, and other versions of its own artifacts)? Enumerate operations directly reached from the changed behavior. If a depended-on assumption is in neither the spec nor the environment-assumption declaration the project owns (the convention listed under `Current Files` in `knowledge/conventions/README.md` whose purpose is declaring environment assumptions; resolve it through that index, not a fixed filename), report it as `未宣言の前提への依存`; treat unresolvable indirect boundaries the same way rather than searching exhaustively.
- **invoke leak**: a composition entry point that executes application flow,
  whether it directly calls a use-case interactor (e.g., `.run(...)` /
  `.dispatch(...)` / `.execute(...)`) or delegates to a `PrimaryAdapter` / Driver
  method (e.g., `.handle(...)` / `.run(...)`). This includes a `CompositionRoot`
  method and any public free function in `cli_composition`. A composition root may
  construct and inject those collaborators, but it must not accept a request and
  return an application result itself. Name the entry point and the call through
  which the application flow leaves it.
- **public free-function composition entry point**: a public free function in
  `cli_composition`, including one that only returns a fully wired adapter.
  Composition wiring belongs on a `CompositionRoot` method; a free function is not
  a permitted handoff surface.
- **prohibited public-surface exposure**: a `CompositionRoot` public field,
  generic bound, implemented application trait, or method signature that exposes
  a domain type, use-case type (including its error types), or an internal role
  type. Keep those details behind the composition boundary; a composition-local
  typed wiring error remains permitted. Name the exposed type and the layer it
  belongs to.
- **`PrimaryAdapter` wiring allowance**: do not report a composition method
  solely because it constructs the object graph and returns a fully wired
  `PrimaryAdapter`. That return value is the permitted pure-DI handoff to
  `apps/cli`; it does not permit the composition method to expose prohibited
  role types or invoke the adapter's application flow.
- **render leak**: a module in `cli_composition` that assembles user-facing
  strings, formats tables, or performs output templating. Rendering is the
  `cli_driver` layer's responsibility; string construction in the composition
  root leaks that responsibility.
- **`Result<_, String>` in public API**: a public function or method in
  `cli_composition` that returns `Result<_, String>` (stringly-typed error).
  All public wiring functions must return a typed error — use `CompositionError`
  or a bounded typed error enum.
- **CliApp god-facade residue**: any `pub struct CliApp;` definition or
  `impl CliApp { ... }` block. The god-facade was superseded by bounded-context
  `CompositionRoot` structs (one per bounded context / command family).
- **adapter defined here**: a `struct` in `cli_composition` that `impl`s a domain
  or usecase port (secondary adapter implementation). Port implementations belong
  in `libs/infrastructure`; `cli_composition` only constructs and wires them.
- **port-adapter pairing mistake**: a wiring function that constructs adapter `A`
  but binds it to a port that `A` does NOT implement (code may compile via a
  separate impl block). Name the adapter, the port it was bound to, and the
  impl block that makes the mismatch compile.
- **panic in wiring**: `unwrap()` / `expect()` on a config-load or constructor
  call in production wiring. Wiring errors must propagate as `Result<_, CompositionError>`
  to the CLI caller.
- **double-instantiation of stateful adapter**: a builder that creates two
  instances of an adapter holding shared mutable state (file handle, DB pool, lock)
  where one was intended. Name the shared state the two instances would contend for.
- **leaked test fixture in production wiring**: a `pub fn` reachable from real CLI
  commands that returns an adapter with a hard-coded test profile, fake path, or
  in-memory store. Show the production call path that reaches it; a fixture reachable
  only from `#[cfg(test)]` is not this finding.

## What NOT to report

- Naming of wiring functions (`new_with_xyz` vs `build_xyz`) when consistent
  with adjacent crates
- Adding a `Default` impl the existing code intentionally omits
- "You could extract a trait here" suggestions for one-off compositions
- Renaming `CompositionRoot` structs for "clarity" when names match the bounded
  context they wire
- Test fixture internals (test wiring has its own contracts)
- Adding lifetime annotations the compiler does not require
- Suggestions to inline or merge two `CompositionRoot` structs when the current
  split follows bounded-context lines
- Only for track `scope-conditional-pre-review-gates-2026-07-31`: the PRE-EXISTING
  review_v2 gated-entry pattern (`ReviewCompositionRoot::review_run_local` and the
  interim `ReviewServiceImpl` shim in `apps/cli-composition/src/review_v2/`) — a user
  adjudication (2026-07-22) deferred the review_v2 composition wire-only/render
  remediation to a separate track. Do not report invoke-leak findings on that
  pre-existing surface when the track's diff makes no semantic change to it (mechanical
  constructor/wiring adjustments included). NEW invoke paths added by this track remain
  fully reportable. A rollback-diagnoser adjudication on this track additionally
  recorded that rerouting the run-lane warnings from `eprintln!` into
  `RunReviewOutput.diagnostics` is a transport-only change that does NOT void the
  deferral: the run-lane findings (`render_verdict_payload` presentation in
  composition, composition-built `[WARN]` diagnostic text pending a typed diagnostic
  condition, and the `FindingsCountReviewer` decorator placement) belong to the
  deferred remediation track, not to this track. Do not report them here.
