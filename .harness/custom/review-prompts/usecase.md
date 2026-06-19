# Usecase Layer Review: Severity Policy

The reviewer's role is **purity and orchestration correctness review** of
`libs/usecase/`. The usecase layer is a **pure orchestrator** — it composes
domain ports and usecase ports into application flows; it must never reach
out to the runtime. **Mechanical purity verification** (syn-AST detection of
banned imports / calls) is `sotp verify usecase-purity`; the reviewer
focuses on what the AST scanner cannot catch.

## What to report

Report findings ONLY for the following categories:

- **purity violation by trait or generic**: a `T: Reader` bound that effectively
  forces an `std::io::Read` dependency the syn scanner cannot see (e.g., via
  a re-export), or a generic constraint that lets infrastructure leak into
  usecase. Cite `hexagonal-architecture.md` §Usecase Layer Purity Rules.
- **implicit time / env / process dependency**: a function that calls a port
  whose only implementation reads `SystemTime` / `env::var()` / spawns a
  process without that being a documented port contract (the port is
  effectively a fig leaf over an impure call). Time / env / process values
  must be parameters to the usecase entrypoint, not retrieved inside.
- **business logic leak**: a calculation, branching, or decision that belongs
  in `domain` (e.g., a comparison that should be a domain method on a
  Newtype) executed in usecase. Cite §CLI as Composition Root for the
  inverse boundary: usecase orchestrates, domain decides.
- **port placement mistake**: a port defined in usecase that should live in
  domain (a port abstracting a domain concept, not an infrastructure
  capability). Cite `hexagonal-architecture.md` §Port Placement Rules.
- **direct infrastructure reference**: any non-test code in usecase importing
  from `infrastructure::*` (even via re-export). Cite §Layer Dependencies.
- **error type confusion**: the usecase error enum re-exposes infrastructure
  error variants (e.g., `io::Error`) instead of mapping them to a
  usecase-level concept, breaking the abstraction. The interactor's
  callers should not need to know an `io::Error` could happen.
- **output side-effect in usecase**: `println!` / `eprintln!` / file write /
  TCP send inside a usecase function. Outputs belong in the CLI mapping.

## What NOT to report

- The shape of test helpers (`unwrap()` in `#[cfg(test)]` is permitted)
- Adding new ports "for symmetry" when the spec does not require them
- Refactoring an existing interactor to be more "type-state-y" if the
  current code passes purity + has correct port boundaries
- Wording of `# Errors` doc sections beyond presence
- Suggested input validation that domain already enforces via Newtypes
- Performance suggestions unless they cross the purity boundary
