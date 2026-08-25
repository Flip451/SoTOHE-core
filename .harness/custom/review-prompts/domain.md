# Domain Layer Review: Severity Policy

The reviewer's role is **type-level / invariant-level correctness review** of
`libs/domain/`. The domain layer is the innermost layer with **zero
dependencies on any other crate** (`architecture-rules.json`), so violations
of its type-safety and purity rules cascade upward. **Mechanical checks**
(layer dependency and configured lint / verification checks)
are handled by `cargo make check-layers` / `cargo make clippy` / `cargo make
verify-*`. Public API documentation presence is reviewed under
`knowledge/conventions/coding-principles.md`; wording remains outside the
reviewer's remit.

## Priority categories

Violations of the role statement above are always reportable. The following priority categories focus the review and guide severity assessment; they are not an exhaustive list of reportable design deviations. The exclusions in **What NOT to report** still apply:

- **external-boundary assumptions**: What external boundaries does the diff touch (OS, process, encoding, concurrency, resource limits, time, and other versions of its own artifacts)? Enumerate operations directly reached from the changed behavior. If a depended-on assumption is in neither the spec nor `knowledge/conventions/environment-assumptions.md`, report it as `未宣言の前提への依存`; treat unresolvable indirect boundaries the same way rather than searching exhaustively.
- **public API documentation gap**: a public API is missing its required `///`
  documentation or `# Errors` section. Report missing documentation, not wording
  or style choices.
- **primitive obsession**: raw `String` / `u64` / `i32` used where a domain
  Newtype should encode invariants (`UserId`, `EmailAddress`, `SimilarityThreshold`).
  Name the invariant the raw type leaves unenforced.
- **enum-first violation**: boolean flags or string discriminants used where
  an `enum` would make illegal states unrepresentable. Name a combination the
  current representation admits and the type should not.
- **typestate / parse-don't-validate gap**: a function returning `Result<T, E>`
  whose `T` does not encode the validation it just performed (the caller can
  still reach an invalid state). Name the reachable invalid state.
- **panic-able production code**: `.unwrap()` / `.expect()` / `panic!()` /
  index-access (`slice[i]`) / `assert!()` / `unreachable!()` / `todo!()` in
  any code path NOT under `#[cfg(test)]`.
- **port misplaced**: a trait that abstracts an infrastructure capability
  (git hash, two-phase commit, HTTP client) placed in domain instead of
  usecase. Name the capability the trait abstracts.
- **purity violation**: `std::fs::*`, `std::net::*`, `std::process::*`,
  `std::io::*`, `std::env::*`, `chrono::Utc::now()`, `std::time::SystemTime`,
  `std::time::Instant`, `println!` / `eprintln!` reaching domain (these are
  caught by `usecase-purity` for usecase but should never appear in domain
  either by extension).
- **broken invariant in newtype constructor**: a `try_new` / `new` that
  accepts inputs the type's documented invariant says it should reject (e.g.,
  empty string for a Newtype documented as "non-empty"), or accepts inputs
  silently when fail-closed validation is required.

## What NOT to report

- Doc string wording suggestions when the required documentation is present
  (phrasing is author's call)
- Adding derives (`Clone` / `Hash` / `Display`) that the catalogue contract
  intentionally omits — verify catalogue first via `<track>/domain-types.json`
- Renaming to "better" identifiers when the existing name already matches
  Rust naming conventions
- Performance micro-optimization unless the panic / correctness boundary is at risk
- Adding error variants the spec does not require
- Restructuring module layout when the existing layout passes
  `cargo make check-layers` and respects the convention
- Only for track `scope-conditional-pre-review-gates-2026-07-31`: the layer placement of
  the PRE-EXISTING `CommitHashReader` / `CommitHashWriter` ports in
  `libs/domain/src/review_v2/ports.rs` (introduced by an earlier track under the
  review-system-v2 redesign ADR; see the ADR index).
  A rollback-diagnoser verdict (2026-08-09) adjudicated relocation as out-of-diff for
  this track: the placement conflict with the current Port-placement tie-break is to be
  resolved by a dedicated ADR-routed track (recorded in the harness-policy briefing's
  Known Accepted Deviations). Do not report the ports' domain placement as a finding in
  this track; doc-only touches to the file remain reviewable.
