# Usecase Layer Review: Severity Policy

The reviewer's role is **purity and orchestration correctness review** of
`libs/usecase/`. The usecase layer is a **pure orchestrator** — it composes
domain ports and usecase ports into application flows; it must never reach
out to the runtime. **Mechanical purity verification** (syn-AST detection of
banned imports / calls) is `sotp verify usecase-purity`; the reviewer
focuses on what the AST scanner cannot catch.

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

- **purity violation by trait or generic**: a `T: Reader` bound that effectively
  forces an `std::io::Read` dependency the syn scanner cannot see (e.g., via
  a re-export), or a generic constraint that lets infrastructure leak into
  usecase. Cite `coding-principles.md` §Usecase Layer Purity.
- **implicit time / env / process dependency**: a function that reads
  `SystemTime` / `env::var()` or spawns a process directly, or calls an
  undocumented abstraction that merely hides the same runtime access.
  User-supplied time belongs in the usecase entrypoint; execution time,
  randomness, and generated identifiers must be acquired through explicit
  usecase-owned secondary ports whose runtime adapters are injected.
- **business logic leak**: a calculation, branching, or decision that belongs
  in `domain` (e.g., a comparison that should be a domain method on a
  Newtype) executed in usecase. Cite `coding-principles.md` §Usecase Layer
  Purity for the inverse boundary: usecase orchestrates, domain decides.
- **port placement mistake**: a port defined in usecase that should live in
  domain (a port abstracting a domain concept, not an infrastructure
  capability). Cite `type-designer-kind-selection.md` R1.
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
