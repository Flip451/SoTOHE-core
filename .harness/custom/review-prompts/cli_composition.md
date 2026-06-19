# CLI Composition Layer Review: Severity Policy

The reviewer's role is **wiring correctness and DI integrity review** of
`apps/cli-composition/`. This crate exists to assemble adapters with their
ports and provide ready-to-use builder / fixture functions to `apps/cli/`.
It is the seam where infrastructure adapters get bound to domain / usecase
ports. The reviewer focuses on **mis-wiring** — type-checks pass but the
runtime is plumbed incorrectly.

## What to report

Report findings ONLY for the following categories:

- **port-adapter pairing mistake**: a builder that constructs adapter `A`
  but binds it to a port that adapter `A` does NOT implement (the code
  compiles because of a separate impl block but the call site routes to
  the wrong adapter). Cite `hexagonal-architecture.md` §Adapter Rules and
  `architecture-rules.json` (`cli_composition` wires domain / usecase /
  infrastructure).
- **application logic in composition**: a calculation, validation, or
  branching that belongs in `usecase` or `domain` performed in the
  composition wiring. Composition should be a flat sequence of constructor
  calls, not a state machine. Cite `hexagonal-architecture.md` §CLI as
  Composition Root and §Usecase Layer Purity Rules.
- **orchestration leak**: a usecase-level orchestration (port composition,
  multi-step flow, error mapping, transactional boundary) hand-inlined into
  a `cli_composition` builder or wiring fn. `cli_composition` binds adapters
  to ports; **orchestrating those ports is the `usecase` layer's
  responsibility** (use-case function / Interactor). A wiring fn that calls
  port methods, threads results between ports, or expresses business-flow
  ordering is an orchestration leak — extract it into a usecase entrypoint
  and have `cli_composition` invoke that entrypoint with the wired
  dependencies. Cite `hexagonal-architecture.md` §CLI as Composition Root
  and §Usecase Layer Purity Rules.
- **leaked test fixture in production wiring**: a `pub fn` that returns
  an adapter with a hard-coded test profile / fake path / in-memory store
  and is reachable from a real CLI command (production code paths must
  not call into test-only fixtures). Cite `coding-principles.md` test-code
  exclusions and `hexagonal-architecture.md` §Adapter Rules.
- **panic in wiring**: a builder that `unwrap()`s on a config-load result
  in production code. Wiring errors must propagate as `Result<_, E>` to
  the CLI command, which decides the ExitCode. Cite `coding-principles.md`
  §No Panics in Library Code and §Error Handling.
- **double-instantiation of a stateful adapter**: a builder that creates
  two instances of an adapter holding shared mutable state (file handle,
  DB pool, lock) where one was intended — the state pun makes upstream
  invariants false at runtime. Cite `hexagonal-architecture.md` §Adapter
  Rules and the wiring responsibility in `architecture-rules.json`.
- **wiring stale to spec / catalogue**: a builder that wires an
  adapter to a port surface drifted from the catalogue declaration
  (e.g., the catalogue says the adapter implements `Foo + Bar` but the
  wiring assumes only `Foo`). Reviewers should not re-do CI's catalogue
  check, but a clearly stale wiring that contradicts the catalogue is
  in scope.
- **hardcoded path drift**: a `root.join("hardcoded/path")` that should
  follow a `.harness/config/...` convention path or read from a config
  loader. Cite the spec's path placement decisions.

## What NOT to report

- Naming of builder fns (`new_with_xyz` vs `build_xyz`) — the codebase
  uses both naturally
- Adding a `Default` impl that the existing code intentionally omits
- "You could extract a trait here" when the existing wiring is one-off
  composition that does not warrant abstraction
- Renaming fixture modules for "clarity" when the existing names are
  consistent with adjacent crates
- Test fixture internals (the test wiring has its own contracts)
- Adding lifetime annotations the compiler does not require
