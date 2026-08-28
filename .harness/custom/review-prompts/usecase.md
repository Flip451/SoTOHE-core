# Usecase Layer Review: Severity Policy

The reviewer's role is **purity and orchestration correctness review** of
`libs/usecase/`. The usecase layer is a **pure orchestrator** — it composes
domain ports and usecase ports into application flows; it must never reach
out to the runtime. **Mechanical purity verification** (syn-AST detection of
banned imports / calls) is `sotp verify usecase-purity`; the reviewer
focuses on what the AST scanner cannot catch.

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

- **external-boundary assumptions**: What external boundaries does the diff touch (OS, process, encoding, concurrency, resource limits, time, and other versions of its own artifacts)? Enumerate operations directly reached from the changed behavior. If a depended-on assumption is in neither the spec nor the environment-assumption declaration the project owns (the convention listed under `Current Files` in `knowledge/conventions/README.md` whose purpose is declaring environment assumptions; resolve it through that index, not a fixed filename), report it as `未宣言の前提への依存`; treat unresolvable indirect boundaries the same way rather than searching exhaustively.
- **purity violation by trait or generic**: a `T: Reader` bound that effectively
  forces an `std::io::Read` dependency the syn scanner cannot see (e.g., via
  a re-export), or a generic constraint that lets infrastructure leak into
  usecase. Name the runtime dependency the bound admits and the re-export path
  that hides it from the scanner.
- **implicit time / env / process dependency**: a function that reads
  `SystemTime` / `env::var()` or spawns a process directly, or calls an
  undocumented abstraction that merely hides the same runtime access.
  User-supplied time belongs in the usecase entrypoint; execution time,
  randomness, and generated identifiers must be acquired through explicit
  usecase-owned secondary ports whose runtime adapters are injected.
- **business logic leak**: a calculation, branching, or decision that belongs
  in `domain` (e.g., a comparison that should be a domain method on a
  Newtype) executed in usecase. The boundary runs the other way here: usecase
  orchestrates, domain decides. Name the domain type the decision belongs on.
- **port placement mistake**: a port defined in usecase that should live in
  domain (a port abstracting a domain concept, not an infrastructure
  capability). Name the concept the port abstracts.
- **direct infrastructure reference**: any non-test code in usecase importing
  from `infrastructure::*` (even via re-export). Cite `architecture-rules.json`.
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
