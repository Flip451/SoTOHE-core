# DRY Gate Evaluation (before/after census)

- **before** = `c4da67a4` (main immediately before the dry-checker gate, PR #148, merged 2026-06-02)
- **after**  = `9270de33` (latest main, 2026-06-19; gate active ~17 days / ~16 PRs)
- **Method**: an identical AI-based DRY-violation census on both checkouts (intra-unit + 10
  thematic finders → dedupe → adversarial verification). Independent of `sotp dry`'s own
  embedding detector.
- **Fairness controls**: same workflow script, rubric, model mix (Sonnet finders, Opus
  verification), and
  partition algorithm (~9k LOC/unit). Both runs had **0 rate-limit failures** and
  `unverifiedKept = 0` (every finding verified). Scope = `src/` of the 5 first-party crates
  (incl. inline `#[cfg(test)]`); excluded `vendor/**` and integration `tests/`.

## TL;DR

**Verdict: weak / mixed — the data does not support "the gate reduced DRY violations."**

Overall violation density fell (−10.6%), but that decline is almost entirely attributable to
the **cli / cli-composition crate-extraction cleanup** (an unrelated human refactor). In the
layers the gate actually governs for new code (`usecase`, `infrastructure`), density **rose**,
and the gate's own implementation module (`dry_check`) is among the most duplication-dense
areas. The flagship pre-existing violation (`validate_track_id` across 5 sites / all layers)
persists. The gate's value is marginal-preventive (semantic-dup density did fall ~21%), not a
codebase-wide DRY guarantee.

## 1. Overall comparison

| metric | before (`c4da67a4`) | after (`9270de33`) | Δ |
|---|---|---|---|
| totalLoc | 157,104 | 199,423 | +26.9% |
| total findings | 148 | 168 | +13.5% |
| density (/KLoc) | 0.942 | 0.842 | **−10.6%** |
| weighted density (/KLoc) | 1.776 | 1.539 | **−13.3%** |
| weighted score | 279 | 307 | +10.0% |
| high / medium / low | 23 / 85 / 40 | 26 / 87 / 55 | +3 / +2 / +15 |
| cross-layer | 26 | 21 | density −36% |
| knowledge-dup | 7 | 4 | density −55% |

Absolute counts rose (the codebase grew +27%), but density fell ~11% / weighted density ~13%.
Surface reading: positive.

## 2. Per-category density (per KLoc)

| category | before | after | Δ density |
|---|---|---|---|
| exact-clone | 0.108 | 0.090 | −16.6% |
| near-clone | 0.356 | 0.336 | −5.7% |
| **semantic-dup** (gate detector's target) | 0.095 | 0.075 | **−21.2%** |
| structural-dup | 0.261 | 0.251 | −3.9% |
| data-dup | 0.076 | 0.070 | −8.1% |
| knowledge-dup | 0.045 | 0.020 | −55.0% |

All categories' density fell. The class the embedding detector actually targets (semantic-dup)
fell ~21% — the single clearest pro-gate signal (subject to the confounds below + small n).

## 3. Per-layer density — the crux

| layer | before n/KLoc → density | after n/KLoc → density | Δ density | LOC growth |
|---|---|---|---|---|
| domain | 17 / 22.9 → 0.743 | 22 / 30.8 → 0.713 | −4.0% | +34.8% |
| **usecase** | 20 / 21.1 → 0.949 | 32 / 30.5 → **1.049** | **+10.6%** | +44.7% |
| **infrastructure** | 64 / 88.7 → 0.722 | 80 / 101.1 → **0.791** | **+9.6%** | +14.0% |
| cli | 22 / 13.3 → 1.648 | 14 / 15.6 → 0.896 | **−45.6%** | +17.1% |
| cli-composition | 25 / 11.1 → 2.246 | 20 / 21.4 → 0.936 | **−58.3%** | +92.0% |

- The overall density drop is **mostly concentrated in cli (−46%) and cli-composition (−58%)**.
  cli-composition grew +92% LOC (crate extraction) while halving its density — this is the
  documented cli↔cli-composition cleanup (the `before` scan found whole-file duplication from
  the incomplete split: PR-polling logic, reviewer dispatch, track-ID validation). Human
  refactor, not the gate.
- The **gate-governed growing layers (usecase +10.6%, infrastructure +9.6%) got denser.** If
  the gate kept new code clean, the layers receiving the most new code would fall or stay flat;
  instead they rose. New code under the gate accreted duplication.

→ The headline density decline is a composition artifact of one crate-split cleanup, not the
gate's ongoing enforcement.

## 4. Qualitative findings

### 4.1 The biggest `before` violation persists in `after`
The flagship `validate_track_id` / slug-validation duplication (domain `ids.rs` canonical +
3 usecase copies + 2 CLI copies) is still present in `after` (high/knowledge-dup + high/near-clone
+ exact-clone). The gate inspects only PR diffs, so it **cannot retroactively remove
pre-existing duplication**.

### 4.2 The gate's own module is duplication-dense
`dry_check` (written under the gate) carries 19 findings (~1.27/KLoc, above the `after` mean of
0.842), incl. high-severity clones shared with `review_v2` (Codex subprocess `spawn`/`drain`/
`tee` triplicates, exclusive-lock pattern, 4-source git-diff union, SHA-256 hex thrice). Duplicates
co-introduced within one PR evade the diff×corpus comparison.

### 4.3 Constant duplication grew under the gate
`POLL_INTERVAL = 50ms`: 4 → 5 sites; `"tmp/reviewer-runtime"`: 3 named const sites → 4
named const sites + 1 inline literal. The embedding
detector does not target data-dup.

## 5. Mechanism

`sotp dry` matches each PR diff fragment against the corpus by embedding similarity and blocks
new semantic duplication. Consequences: (1) **no retroactivity** — pre-existing dup is out of
scope; (2) **preventive only** — prevented duplications are unobservable (so net improvement
may be understated); (3) **blind spots** — same-PR structural/cross-layer clones and data-dup
slip through. The per-layer decomposition does not support the prevented-dup explanation
(governed growing layers got denser).

## 6. Threats to validity

- **Unobservable prevention**: duplications the gate blocked never appear in before/after, so
  "small net change ≠ gate ineffective."
- **Short window / small n**: ~17 days, ~16 PRs; knowledge-dup 7→4 is noise-prone.
- **Single run each**: stochastic; small per-category deltas may be within noise. The per-layer
  cli/cli-comp shift is large/structural and the qualitative persistence is robust.
- **Detector mismatch**: this census is AI-based and includes structural/data-dup; the gate is
  embedding-based and semantic-dup-centric. Some of what this census counts is out of the gate's
  design scope.

## 7. Recommendations

1. **Retroactive cleanup** (the gate won't do it): consolidate `validate_track_id` (5 sites) →
   `TrackId::try_new`, the `NonEmptyString` invariant (8+ sites), and the reviewer/dry-check
   twins via a dedicated track. (See the remediation ADR.)
2. **Widen gate coverage**: detect same-PR internal clones (not only diff-vs-corpus); add a
   periodic diff-independent full-corpus sweep for pre-existing / cross-layer duplication.
3. **Aggregate test-helper rule**: flag when the same helper shape appears in N≥3 modules so
   per-instance leniency doesn't let scaffolding proliferate.
4. **Separate data-dup gate**: constants/magic values need a clippy/grep gate (embedding won't
   catch them).
5. **Track over time**: re-run this census periodically and watch per-layer density of new code
   — time-series separates gate effect from one-off refactors better than a single before/after.
