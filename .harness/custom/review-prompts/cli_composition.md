# CLI Composition Layer Review: Severity Policy

The reviewer's role is **pure-DI wiring correctness review** of
`apps/cli-composition/`. `cli_composition` is the composition root: it constructs
secondary adapters (from `infrastructure`), use-case interactors, and driving adapters
(from `cli_driver`), and hands the fully-wired drivers to `apps/cli`. It must **only
wire** — it must not invoke use cases, render output, or define adapter
implementations. Wiring errors (port-adapter mismatch, double-instantiation, panic on
config load) are in scope; application-logic and presentation concerns always belong
in `usecase` or `cli_driver` (D2/D7,
`2026-06-21-1328-cli-composition-split-presentation-layer.md`).

## What to report

Report findings ONLY for the following categories:

- **invoke leak**: a wiring function or module in `cli_composition` that directly
  calls a use-case interactor method (e.g., `.run(...)` / `.dispatch(...)` /
  `.execute(...)`) instead of constructing it and injecting it into a Driver.
  Composition root wires object graphs at startup; invoking a use case at
  wiring time is an invoke leak. Cite ADR D2 and `hexagonal-architecture.md`
  §CLI as Composition Root.
- **render leak**: a module in `cli_composition` that assembles user-facing
  strings, formats tables, or performs output templating. Rendering is the
  `cli_driver` layer's responsibility (ADR D3); string construction in the
  composition root leaks that responsibility. Cite ADR D2/D3.
- **`Result<_, String>` in public API**: a public function or method in
  `cli_composition` that returns `Result<_, String>` (stringly-typed error).
  All public wiring functions must return a typed error — use `CompositionError`
  or a bounded typed error enum. Cite ADR D2.
- **CliApp god-facade residue**: any `pub struct CliApp;` definition or
  `impl CliApp { ... }` block. The god-facade was superseded by bounded-context
  `CompositionRoot` structs (one per bounded context / command family). Cite
  ADR D2 (D2 explicitly abolishes `CliApp`).
- **adapter defined here**: a `struct` in `cli_composition` that `impl`s a domain
  or usecase port (secondary adapter implementation). Port implementations belong
  in `libs/infrastructure`; `cli_composition` only constructs and wires them.
  Cite ADR D7.
- **port-adapter pairing mistake**: a wiring function that constructs adapter `A`
  but binds it to a port that `A` does NOT implement (code may compile via a
  separate impl block). Cite `hexagonal-architecture.md` §Adapter Rules and
  `architecture-rules.json`.
- **panic in wiring**: `unwrap()` / `expect()` on a config-load or constructor
  call in production wiring. Wiring errors must propagate as `Result<_, CompositionError>`
  to the CLI caller. Cite `coding-principles.md` §No Panics in Library Code.
- **double-instantiation of stateful adapter**: a builder that creates two
  instances of an adapter holding shared mutable state (file handle, DB pool, lock)
  where one was intended. Cite `hexagonal-architecture.md` §Adapter Rules.
- **leaked test fixture in production wiring**: a `pub fn` reachable from real CLI
  commands that returns an adapter with a hard-coded test profile, fake path, or
  in-memory store. Cite `coding-principles.md` test-code exclusions.

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
