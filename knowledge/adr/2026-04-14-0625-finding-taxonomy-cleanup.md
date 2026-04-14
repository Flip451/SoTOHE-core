# Finding 型 Taxonomy クリーンアップ — 同名衝突の解消と hexagonal 分離の維持

## Status

Accepted (to be implemented by track `tddd-04-finding-taxonomy-cleanup-2026-04-14`)

See also:
- Parent: `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` (Phase 1 Completion Amendment §3.B defers this work to the present track; the §3.B Resolution subsection in that ADR points back here).
- Sibling: `knowledge/adr/2026-04-13-1813-tddd-taxonomy-expansion.md` (precedent for the `TraitPort` → `SecondaryPort` cascade rename followed here).

## Context

SoTOHE-core currently exposes **four distinct `Finding`-family types** across its layers:

| # | Type | Location | Role |
|---|---|---|---|
| 1 | `domain::review_v2::Finding` | `libs/domain/src/review_v2/types.rs:210` | Validated domain newtype — reviewer verdict finding. Private fields, `new()` enforces non-empty `message`. |
| 2 | `domain::verify::Finding` | `libs/domain/src/verify.rs:28` | Structured error / warning produced by `sotp verify` subcommands. Carries a `Severity` enum. |
| 3 | `usecase::review_workflow::ReviewFinding` | `libs/usecase/src/review_workflow/verdict.rs:102` | Serde DTO for reviewer JSON wire format (`#[serde(deny_unknown_fields)]`). |
| 4 | `usecase::pr_review::PrReviewFinding` | `libs/usecase/src/pr_review.rs:70` | Parsed Codex Cloud PR review result — different shape (severity string `"P0"/"P1"/…`). |

Types (1) and (2) share the last-segment name `Finding` within the `domain` crate. Since `libs/infrastructure/src/code_profile_builder.rs:54` builds a `HashMap<String, TypeNode>` keyed by last-segment name over the domain rustdoc JSON output, the two types collide non-deterministically — the second entry overwrites the first and a `warning: same-name type collision for Finding` line is emitted to stderr on every `build_type_graph` invocation.

TDDD-01 (`tddd-01-multilayer-2026-04-12`) mitigated the downstream noise by placing a suppression `"reference"` entry in `domain-types.json` (the 4th entry in the `type_definitions` array):

```json
{
  "name": "Finding",
  "description": "Reference entry for the pre-existing same-name type collision between domain::verify::Finding and domain::review_v2::types::Finding. The collision is documented as a known issue; this reference declaration suppresses the baseline_changed_type Red signal caused by non-deterministic HashMap ordering at TypeGraph build time",
  "approved": true,
  "action": "reference",
  "kind": "value_object"
}
```

This suppressed the Red signal but did not fix the underlying collision. The entry is documented as "a known issue" — i.e. technical debt, not a durable design decision.

The collision has four concrete impacts:

1. **Noisy stderr on every baseline capture**: every `sotp track baseline-capture <track-id> --layer domain` run prints the collision warning.
2. **Non-deterministic type graph**: depending on rustdoc JSON ordering, either `review_v2::Finding` or `verify::Finding` is dropped from the `TypeGraph` — the TDDD signal evaluator cannot reason about the dropped one.
3. **Ambiguous catalogue references**: TDDD L1 signals use last-segment names for `expected_methods` / `kind_tag` resolution. A catalogue entry naming `Finding` cannot disambiguate which of the two the author meant.
4. **Technical-debt suppression entry**: the `domain-types.json` `"Finding"` reference entry has no semantic value; it exists solely to shut up the `baseline_changed_type` Red signal caused by HashMap non-determinism.

TDDD-02 (`tddd-02-usecase-wiring-2026-04-14`) explicitly deferred the fix, citing three constraints (ADR 2026-04-11-0002 Phase 1 Completion Amendment §3.B):

- The natural rename target `ReviewFinding` is **already taken** by `usecase::review_workflow::ReviewFinding` (the DTO).
- `domain::review_v2::Finding` is deeply embedded in `Verdict::FindingsRemain(NonEmptyFindings)`, `FindingError`, and associated types — a rename requires cascading updates across the entire `review_v2` module plus infrastructure consumers.
- A proper fix is taxonomy-level, not a single-file rename. Doing it during tddd-02 would have destabilized the merge gate work.

The deferral explicitly named the follow-up as **`tddd-04 finding-taxonomy-cleanup`**. The present ADR records the design decision for that follow-up.

## Decision

### D1: Option B — full rename, no integration

The four `Finding` types are **kept as four distinct Rust types**. The two colliding domain types are renamed to make their last-segment names unique. The usecase-layer types (`ReviewFinding` the serde DTO, `PrReviewFinding` the Codex Cloud parse result) are left unchanged because they already have distinct last-segment names.

Two alternatives were rejected; see [Rejected Alternatives](#rejected-alternatives).

### D2: New names

| Old symbol | New symbol | Rationale |
|---|---|---|
| `domain::review_v2::Finding` | `domain::review_v2::ReviewerFinding` | The type lives in `review_v2` and is the domain-validated counterpart to `usecase::review_workflow::ReviewFinding`. The `Reviewer` prefix pairs cleanly with the `CodexReviewer` adapter and establishes an explicit DTO/domain pairing: `ReviewFinding` (wire DTO) ↔ `ReviewerFinding` (validated domain). |
| `domain::verify::Finding` | `domain::verify::VerifyFinding` | The type is produced by `sotp verify` subcommands and carries a `Severity`. The `Verify` prefix mirrors the module path and is what a reader expects when tracing from `VerifyOutcome` down to its element. |

Both new last-segment names are **confirmed unique** workspace-wide (grep of all `.rs` files shows zero existing uses of `ReviewerFinding` or `VerifyFinding`). Re-evaluation is required if future crates adopt these names before this track lands.

Rejected name candidates:

- `CodexFinding` — ties the domain type to a single adapter (`CodexReviewer`). The domain type should be reviewer-provider-agnostic.
- `ReviewRemark`, `CritiqueItem` — non-idiomatic in this codebase; the existing vocabulary already uses `Finding` everywhere.
- `Diagnostic` — collides with compiler / linter terminology; ambiguous.
- `VerificationIssue` — verbose; `Verify` prefix is sufficient.
- `CheckResult` — implies pass/fail, not per-finding.

### D3: Cascade rename enumeration

The following symbols rename together with `domain::review_v2::Finding`:

| Old | New | Layer |
|---|---|---|
| `Finding` (struct) | `ReviewerFinding` | domain/review_v2 |
| `NonEmptyFindings` | `NonEmptyReviewerFindings` | domain/review_v2 |
| `FindingError` | `ReviewerFindingError` | domain/review_v2 |
| `FindingError::EmptyMessage` | `ReviewerFindingError::EmptyMessage` | domain/review_v2 |
| `FindingError` re-export in `mod.rs` | `ReviewerFindingError` | domain/review_v2 |
| `Finding` re-export in `mod.rs` | `ReviewerFinding` | domain/review_v2 |
| `NonEmptyFindings` re-export in `mod.rs` | `NonEmptyReviewerFindings` | domain/review_v2 |
| `test fn finding(msg)` / `fn finding_full()` return types | `ReviewerFinding` | domain/review_v2/tests |
| `Finding::new(…)` in test helper bodies | `ReviewerFinding::new(…)` | domain/review_v2/tests |
| `use super::error::{…, FindingError, …}` import in tests | `use super::error::{…, ReviewerFindingError, …}` | domain/review_v2/tests |
| `Err(FindingError::EmptyMessage)` in test assertions | `Err(ReviewerFindingError::EmptyMessage)` | domain/review_v2/tests |
| `Verdict::findings_remain(Vec<Finding>)` signature | `Vec<ReviewerFinding>` | domain/review_v2 |
| `FastVerdict::findings_remain(Vec<Finding>)` signature | `Vec<ReviewerFinding>` | domain/review_v2 |
| `NonEmptyFindings::{new, as_slice, into_vec}` signatures | `NonEmptyReviewerFindings::{…}` | domain/review_v2 |
| `convert_findings_to_domain` return type | `Vec<ReviewerFinding>` | infrastructure/review_v2 |
| `Finding::new(…)` call in `convert_findings_to_domain` | `ReviewerFinding::new(…)` | infrastructure/review_v2 |
| `findings: &[Finding]` in `persistence/review_store.rs` | `&[ReviewerFinding]` | infrastructure/review_v2/persistence |
| `Vec<Finding>` / `Finding::new(…)` in `persistence/review_store.rs` | `Vec<ReviewerFinding>` / `ReviewerFinding::new(…)` | infrastructure/review_v2/persistence |
| `fn sample_finding() -> Finding` in `persistence/tests.rs` | `fn sample_finding() -> ReviewerFinding` | infrastructure/review_v2/persistence |
| `use domain::review_v2::{..., Finding, ...}` in `libs/usecase/src/review_v2/tests.rs` | `ReviewerFinding` | usecase/review_v2 |
| `Finding::new(…)` in `libs/usecase/src/review_v2/tests.rs` | `ReviewerFinding::new(…)` | usecase/review_v2 |
| `finding_to_review_finding(f: &domain::review_v2::Finding)` | `f: &domain::review_v2::ReviewerFinding` | apps/cli |
| Doc comments in `types.rs`, `error.rs`, `codex_reviewer.rs`, `codex_local.rs` that name `Finding` in `# Errors` or `# Returns` sections | `ReviewerFinding` | All layers (load-bearing docs only) |

The following symbols rename together with `domain::verify::Finding`:

| Old | New | Layer |
|---|---|---|
| `Finding` (struct) | `VerifyFinding` | domain/verify |
| `Finding::new / error / warning` (constructors) | `VerifyFinding::{new, error, warning}` | domain/verify |
| `Finding::error(…)` / `Finding::warning(…)` in `#[cfg(test)] mod tests` (9 call sites at lines 146, 155, 162-165, 179) | `VerifyFinding::error(…)` / `VerifyFinding::warning(…)` | domain/verify (test module) |
| `Finding::severity / message` (methods) | `VerifyFinding::{severity, message}` | domain/verify |
| `impl fmt::Display for Finding` | `impl fmt::Display for VerifyFinding` | domain/verify |
| `VerifyOutcome { findings: Vec<Finding> }` field type | `Vec<VerifyFinding>` | domain/verify |
| `VerifyOutcome::pass()` / `is_ok()` / `has_errors()` / `error_count()` | No signature change — body accesses `self.findings` which changes type implicitly; listed for completeness to prevent missed updates | domain/verify |
| `VerifyOutcome::from_findings(Vec<Finding>)` / `findings() -> &[Finding]` / `add(Finding)` / `merge` | `VerifyFinding` throughout (explicit signature updates) | domain/verify |
| `use crate::verify::{Finding, …}` in domain consumers | `VerifyFinding` | domain/{tddd/consistency.rs, spec.rs} |
| `use domain::verify::{Finding, …}` in usecase consumers | `VerifyFinding` | usecase/{merge_gate.rs, task_completion.rs} |
| `use domain::verify::{Finding, …}` in 18 infra verify files | `VerifyFinding` | `libs/infrastructure/src/verify/*.rs` (18 files) |
| `domain::verify::Finding::error(…)` calls in CLI | `VerifyFinding::error(…)` | `apps/cli/src/commands/verify.rs` |
| Doc comment `/// A single verification finding.` in `verify.rs:26` | Update to reference `VerifyFinding` | domain/verify |
| `Finding::warning` prose in `source-attribution.md` | `VerifyFinding::warning` | knowledge/conventions |

### D4: Symbols explicitly NOT renamed

- **`VerdictError::EmptyFindings`** — the variant name accurately describes the error ("the findings collection is empty"). Renaming to `EmptyReviewerFindings` would be over-engineering. The containing enum `VerdictError` itself is unchanged.
- **`Verdict`, `FastVerdict`, `VerifyOutcome`, `Severity`** — unchanged; only their inner element types rename.
- **`REVIEW_OUTPUT_SCHEMA_JSON`** — the JSON schema defines `$defs/finding` as an internal identifier. `serde_json` serializes by struct field names, not Rust type names, so the wire format is invariant under the Rust rename. No schema update required.
- **`usecase::review_workflow::ReviewFinding`** — already distinct; DTO stays as-is.
- **`usecase::pr_review::PrReviewFinding`** — already distinct; stays.
- **`domain::auto_phase::FindingSeverity`** — an unrelated enum (`P1`/`P2`/`P3`). The `Finding` prefix here is a compound adjective, not a type reference. Targeted renames (`struct Finding`, `enum Finding`, `use.*Finding`) must avoid corrupting this symbol.
- **Historical documentation** under `track/items/tddd-02-*/` — not retroactively renamed. Historical ADRs and design snippets (including code blocks in past ADRs) keep the old names as documented history. This ADR establishes the rename as forward-only.
- **Exception**: `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` is the **live catalogue source** referenced from `architecture-rules.json`. It is NOT a frozen historical artifact — it is an active data file that the TDDD tooling reads on every signal evaluation. T006 explicitly updates this file as a required implementation step. Do not confuse it with the track's documentation files (spec.md, plan.md, verification.md) which are frozen historical records.

### D5: No backward compatibility (precedent maintained)

Following the tddd-01 / tddd-02 precedent (and user guidance 2026-04-13), no `pub use Finding = ReviewerFinding;` aliases, no deprecated shims, no migration path. The old names are removed outright. `domain-types.json` catalogues are re-approved by T006 below, which replaces the single `"Finding"` reference entry with three `declare` entries for the new names.

### D6: Domain purity preserved

`domain::review_v2::ReviewerFinding` does **not** gain `Serialize` / `Deserialize` derives. The `libs/domain` crate already carries a `serde` crate dependency (used by `catalogue.rs` and `schema.rs`), but the `ReviewerFinding` type itself stays serialization-free, preserving the DTO/domain separation. The hexagonal split is preserved:

- `usecase::review_workflow::ReviewFinding` = serde DTO (wire format).
- `domain::review_v2::ReviewerFinding` = validated domain newtype (non-empty message invariant, private fields, constructor returns `Result`).
- `infrastructure::review_v2::codex_reviewer::convert_findings_to_domain` = variant conversion DTO → domain, using `filter_map` to silently discard empty-message findings (the load-bearing invariant enforcement).
- `apps/cli/src/commands/review/codex_local::finding_to_review_finding` = variant conversion domain → DTO (for JSON stdout emission).

The non-empty-message invariant is **load-bearing**: it is the filter boundary between untrusted reviewer output and domain-trusted values. Integration options that dissolve this boundary (see Rejected Alternatives) are therefore rejected.

### D7: `domain-types.json` catalogue update

The suppression entry in `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` (the `"Finding"` reference entry — the 4th entry in the `type_definitions` array) is **deleted**. Three new `declare` entries are added:

```json
{
  "name": "ReviewerFinding",
  "description": "Domain-validated reviewer finding. Invariant: message is non-empty. Counterpart to usecase::review_workflow::ReviewFinding (serde DTO).",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
},
{
  "name": "NonEmptyReviewerFindings",
  "description": "Non-empty collection of ReviewerFinding values. Used as the inner payload of Verdict::FindingsRemain and FastVerdict::FindingsRemain.",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
},
{
  "name": "VerifyFinding",
  "description": "Structured error or warning produced by sotp verify subcommands. Has a Severity (Info/Warning/Error) and a message string.",
  "approved": true,
  "action": "declare",
  "kind": "value_object"
}
```

After catalogue update, `sotp track type-signals tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain` must be re-run (note: `type-signals` takes the track ID as a positional argument before `--layer`). Expected result: `yellow=0 red=0` and `blue` count increased by at least 2 (three new `declare` entries replace one `reference` entry; the old reference entry also counted as blue, so the net delta is +3 new − 1 removed = +2 blue minimum).

### D8: Task ordering and commit grouping

Implementation is split into 7 tasks (T001–T007) grouped into **4 atomic commits**. Each commit is <500 lines and must leave the workspace compile-clean. D5 (no-alias rule) forces cascade groups to land atomically — a domain rename alone leaves dependent crates uncompilable because old names no longer exist, and no compatibility alias is provided.

**Commit grouping:**

- **Commit 1 (~220 lines)**: T001 + T002 + T003 — `verify::Finding` → `VerifyFinding` full cascade.
  - T001: `libs/domain` (verify.rs struct/impl/Display/tests, plus domain consumers tddd/consistency.rs and spec.rs — including doc comment `Finding::error` / `Finding::warning` mentions in `# Rules` sections at spec.rs:517-521, 1638, 1652 and consistency.rs:356-361).
  - T002: `libs/usecase` (merge_gate.rs, task_completion.rs).
  - T003: `libs/infrastructure/src/verify/` (18 files, including doc comment mentions in spec_states.rs:32-36, 186), `apps/cli/src/commands/verify.rs`, and `knowledge/conventions/source-attribution.md` line 29 prose.
- **Commit 2 (~135 lines)**: T004 + T005 — `review_v2::Finding` → `ReviewerFinding` full cascade.
  - T004: `libs/domain/src/review_v2/` (types.rs, error.rs, mod.rs, tests.rs).
  - T005: `libs/infrastructure/src/review_v2/codex_reviewer.rs`, `libs/infrastructure/src/review_v2/persistence/review_store.rs`, `libs/infrastructure/src/review_v2/persistence/tests.rs`, `libs/usecase/src/review_v2/tests.rs`, and `apps/cli/src/commands/review/codex_local.rs`.
- **Commit 3 (~30 lines)**: T006 — `domain-types.json` catalogue update (delete reference entry, add three declare entries, regenerate signals).
- **Commit 4 (0-10 lines)**: T007 — Full CI gate (`cargo make ci`) plus explicit grep for residual `verify::Finding` / `review_v2::Finding` / `struct Finding` (expect zero). Confirm the `same-name type collision for Finding` stderr warning is gone from `sotp track baseline-capture tddd-04-finding-taxonomy-cleanup-2026-04-14 --layer domain --force` output.

**Ordering constraints:**

- The verify-side cascade (Commit 1) and the review-side cascade (Commit 2) are independent and could land in either order.
- Commit 3 (T006, catalogue update) must run **after** Commits 1 and 2 both land, because the regenerated rustdoc JSON must contain `VerifyFinding` and `ReviewerFinding` for `sotp track type-signals` to evaluate them as Blue.
- Commit 4 (T007) is the final gate.

**Why not one commit per task?** D5's no-alias rule means T001 alone would break `cargo build -p usecase` (usecase still imports `Finding` from domain but domain now exports `VerifyFinding`). Similarly T004 alone breaks `cargo build -p infrastructure`. The task numbering T001-T007 is retained for documentation, progress tracking, and task-level description granularity, but commit granularity is 4 atomic groups.

## Rejected Alternatives

### A1: Option A — integrate (1) and (3) into a single type

Add `Serialize` / `Deserialize` derives to `domain::review_v2::Finding` and delete `usecase::review_workflow::ReviewFinding`. Rename (2) to avoid the residual collision.

Rejected because:

- Adds `Serialize` / `Deserialize` derives to `domain::review_v2::Finding` (now `ReviewerFinding`), making a domain type serializable and dissolving the DTO/domain boundary. Note: `libs/domain` already has a `serde` crate dependency (used by `catalogue.rs` / `schema.rs`), so this option does not introduce a new crate dependency — but it does extend serde usage to the validated-newtype layer, violating the hexagonal intent in `knowledge/conventions/hexagonal-architecture.md` (validated domain types should not be wire-format types).
- Collapses the DTO/domain boundary. The non-empty-message invariant currently enforced by `Finding::new()` + `FindingError::EmptyMessage` + the `filter_map` in `convert_findings_to_domain` would either (a) be lost entirely, or (b) require domain-level serde validation hooks that are awkward to express. The invariant is load-bearing: empty-message reviewer output is silently discarded today, and upstream code assumes this.
- Sets a precedent that makes it harder to resist future "just add serde to this one domain type" requests.

### A2: Option C — delete (1) and use the DTO directly

Delete `domain::review_v2::Finding`. Have `convert_findings_to_domain` and `apps/cli/src/commands/review/codex_local::finding_to_review_finding` use `usecase::review_workflow::ReviewFinding` directly. Rename (2) to `VerifyFinding`.

Rejected because:

- Loses the non-empty-message invariant. `ReviewFinding.message: String` is an arbitrary string; nothing enforces non-emptiness. Downstream code (`Verdict::FindingsRemain`) would then carry DTO values directly, weakening the domain model.
- Forces `Verdict::FindingsRemain(NonEmptyFindings)` to either (a) wrap DTO values (awkward — domain aggregates wrap infrastructure types), or (b) disappear entirely, which breaks the review state machine's structural non-emptiness guarantee.
- The pretense of simplification actually increases cognitive load: callers must remember "this `ReviewFinding` came from reviewer JSON; don't trust its message non-emptiness" every time. The current validated-newtype pattern pushes that concern to the boundary.

### A3: Rename only the colliding type(s), leaving the naming inconsistent

Rename `domain::review_v2::Finding` → `ReviewerFinding` but leave `domain::verify::Finding` alone. Add a different suppression entry for `verify::Finding` in the catalogue.

Rejected because:

- The collision only exists because both types share the last-segment name. Renaming one solves the immediate collision, but leaves the catalogue with an unnecessary asymmetry (`ReviewerFinding` declared + `Finding` implied from `verify::Finding`).
- The TDDD last-segment name policy prefers all type names to be unique workspace-wide. Partial rename kicks the can on the other name; a future maintainer would eventually be forced to rename `verify::Finding` anyway.
- Consistency: if one domain `Finding` is renamed for clarity, the other should be too, for symmetric readability.

## Consequences

### Positive

- **Eliminates a known-debt suppression entry** from `domain-types.json`. The `"Finding"` reference entry's `description` field currently reads "documented as a known issue" — that debt is paid off by this rename.
- **Eliminates stderr noise** from `code_profile_builder.rs:54`. No more `warning: same-name type collision for Finding` on every baseline capture.
- **Preserves hexagonal purity** (Option B vs Option A). The DTO/domain split stays intact.
- **Preserves the non-empty-message invariant** (Option B vs Option C). The validated-newtype boundary continues to filter untrusted reviewer output.
- **Establishes a readable pairing** in the codebase: `usecase::review_workflow::ReviewFinding` (DTO) ↔ `domain::review_v2::ReviewerFinding` (validated). The old name `Finding` obscured this symmetry.
- **Enables explicit TDDD catalogue entries** for three previously-suppressed types (`ReviewerFinding`, `NonEmptyReviewerFindings`, `VerifyFinding`), increasing coverage of the domain catalogue.
- **Unblocks future TDDD analysis** of `verify::Finding` and `review_v2::Finding` independently — previously the TypeGraph could only see one of them per rustdoc run.

### Negative

- **Cascade rename cost**: approximately 40–60 files touched across domain, usecase, infrastructure, and CLI layers (18 of those are infra/verify files each with a few call sites). Mechanical but non-trivial. Estimated diff: ~395 lines across **4 atomic commits** (see D8 for the commit grouping). The 4-commit grouping is load-bearing under D5 (no-alias rule): each commit must be self-contained compile-clean, so intra-cascade task numbering (T001-T007) maps to 4 commits — Commit 1: T001+T002+T003 (verify::Finding cascade, ~220 lines), Commit 2: T004+T005 (review_v2::Finding cascade, ~135 lines), Commit 3: T006 (~30 lines), Commit 4: T007 (0-10 lines).
- **Historical ADR / design doc drift**: ADRs such as `2026-04-12-1200-strict-spec-signal-gate-v2.md` (33 `Finding::error` / `Finding::warning` pseudo-code snippets, verified 2026-04-14) and `2026-04-04-1456-review-system-v2-redesign.md` (8 `Finding` struct / `Vec<Finding>` Rust code block references, verified 2026-04-14) are historical design records and will be left as-is. They describe the code as it was when each ADR was written. A reader cross-referencing these ADRs and current code will see the names have changed.
- **`sotp verify arch-docs` scope**: based on the tddd-02 CI run that merged on 2026-04-14 with these historical ADRs already containing the old names, `arch-docs` does NOT lint Rust type references inside ADR code blocks. This track therefore expects `cargo make ci` to pass without any historical ADR edits. If a future `arch-docs` enhancement starts linting code blocks and the historical references trigger it, the remediation is deferred to a separate follow-up track (`historical-adr-lint-resolution`) — this track (`tddd-04`) keeps its scope strictly on the Finding rename and does not edit historical ADRs as part of T007.
- **Track-level catalogue storage**: `track/items/tddd-01-multilayer-2026-04-12/domain-types.json` is the live catalogue source (referenced from `architecture-rules.json`). Updating a completed track's artifact for a follow-up track is unusual but unavoidable given the current per-track catalogue layout. Future tracks that change domain types will face the same pattern.

### Neutral

- **`VerdictError::EmptyFindings` kept**: the variant name references "findings" but does so accurately ("the findings collection is empty"). Renaming to `EmptyReviewerFindings` would be over-engineering. This is the accepted trade-off.
- **Four Finding types persist**: Option B retains all four distinct types. This is intentional — each has a different role. The problem was the *collision*, not the multiplicity.
- **JSON wire format unchanged**: the reviewer JSON `$defs/finding` key is a schema identifier, not a Rust type name. No wire format changes, no reviewer prompt updates, no downstream tooling updates.

## Reassess When

- **Adoption project requests DTO/domain integration**: if a SoTOHE adopter explicitly prefers a single `Finding` type with serde and the corresponding loss of the non-empty invariant, revisit Option A.
- **`review_v2` restructuring**: if a future review system redesign obsoletes `domain::review_v2::Finding` entirely (e.g., replacing it with a more structured `ReviewerRemark` enum per severity), the rename becomes trivially reversed.
- **Third collision appears**: if a future track introduces a fifth `Finding`-family type, this ADR's naming rubric should be applied uniformly rather than ad-hoc.
- **TDDD catalogue migrates away from per-track storage**: if `domain-types.json` is relocated out of `track/items/tddd-01-*/`, the catalogue update task becomes a simple edit of the canonical file instead of touching a completed track's artifact.

## Related

- **ADR `2026-04-11-0002-tddd-multilayer-extension.md`** (Phase 1 Completion Amendment §3.B): the deferral notice naming this track.
- **ADR `2026-04-13-1813-tddd-taxonomy-expansion.md`**: the `TraitPort` → `SecondaryPort` cascade rename precedent. Same "no backward compatibility" policy.
- **`knowledge/conventions/hexagonal-architecture.md`**: the domain-purity rule that rejects Option A.
- **`.claude/rules/04-coding-principles.md`**: "Make Illegal States Unrepresentable" — the non-empty-message invariant is a concrete application of this principle, which Option C would weaken.
- **`libs/infrastructure/src/code_profile_builder.rs` (~line 54)**: the collision warning emitter.
- **`track/items/tddd-01-multilayer-2026-04-12/domain-types.json` (`"Finding"` reference entry, 4th in `type_definitions`)**: the suppression entry this track removes.
- **`knowledge/research/2026-04-14-0625-planner-tddd-04-finding-taxonomy.md`**: the full Claude Opus planner output (7 sections including Canonical Blocks, data-flow diagram, and rename table).
- **`tmp/handoff/tddd-04-finding-taxonomy-handoff-2026-04-14.md`**: the original handoff from the tddd-02 session that framed the problem.
