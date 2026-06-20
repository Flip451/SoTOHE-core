# Domain Layer Review: Severity Policy

The reviewer's role is **type-level / invariant-level correctness review** of
`libs/domain/`. The domain layer is the innermost layer with **zero
dependencies on any other crate** (`architecture-rules.json`), so violations
of its type-safety and purity rules cascade upward. **Mechanical checks**
(layer dependency, no `std::fs`/`std::env` in domain, doc string presence)
are handled by `cargo make check-layers` / `cargo make clippy` / `cargo make
verify-*`, not the reviewer.

## What to report

Report findings ONLY for the following categories:

- **primitive obsession**: raw `String` / `u64` / `i32` used where a domain
  Newtype should encode invariants (`UserId`, `EmailAddress`, `SimilarityThreshold`).
  Cite `knowledge/conventions/prefer-type-safe-abstractions.md` §Newtype.
- **enum-first violation**: boolean flags or string discriminants used where
  an `enum` would make illegal states unrepresentable. Cite
  `prefer-type-safe-abstractions.md` §Enum-first.
- **typestate / parse-don't-validate gap**: a function returning `Result<T, E>`
  whose `T` does not encode the validation it just performed (the caller can
  still reach an invalid state). Cite §Parse-don't-validate.
- **panic-able production code**: `.unwrap()` / `.expect()` / `panic!()` /
  index-access (`slice[i]`) / `assert!()` / `unreachable!()` / `todo!()` in
  any code path NOT under `#[cfg(test)]`. Cite `coding-principles.md`
  §No Panics in Library Code.
- **port misplaced**: a trait that abstracts an infrastructure capability
  (git hash, two-phase commit, HTTP client) placed in domain instead of
  usecase. Cite `hexagonal-architecture.md` §Port Placement Rules.
- **purity violation**: `std::fs::*`, `std::net::*`, `std::process::*`,
  `std::io::*`, `std::env::*`, `chrono::Utc::now()`, `std::time::SystemTime`,
  `std::time::Instant`, `println!` / `eprintln!` reaching domain (these are
  caught by `usecase-purity` for usecase but should never appear in domain
  either by extension).
- **broken invariant in newtype constructor**: a `try_new` / `new` that
  accepts inputs the type's documented invariant says it should reject (e.g.,
  empty string for a Newtype documented as "non-empty"), or accepts inputs
  silently when fail-closed is required (cite CN-04 patterns).

## What NOT to report

- Doc string wording suggestions (CI checks doc presence; phrasing is author's call)
- Adding derives (`Clone` / `Hash` / `Display`) that the catalogue contract
  intentionally omits — verify catalogue first via `<track>/domain-types.json`
- Renaming to "better" identifiers when the existing name already matches
  Rust naming conventions
- Performance micro-optimization unless the panic / correctness boundary is at risk
- Adding error variants the spec does not require
- Restructuring module layout when the existing layout passes
  `cargo make check-layers` and respects the convention
