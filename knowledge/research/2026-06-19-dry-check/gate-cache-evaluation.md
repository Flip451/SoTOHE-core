# DRY Gate Quality — from the gate's own cached verdicts

Evaluated from the gate's own cache (`track/items/<id>/dry-check.json`, plus
`dry-check-coverage.json`) across the **16 tracks** that ran since the gate merged (PR #148,
2026-06-02). This is a separate line of evidence from the before/after AI census: here the
primary source is **what the gate actually decided**.

## How the gate works (observed from the cache)

Two stages:
1. **Embedding similarity** selects candidate pairs (`threshold = 0.85`, cosine).
2. An **LLM judge** (the Codex dry-checker) assigns a `verdict` + reasoning to each pair:
   - `verdict = {violation: {refactor_proposal}}` — a violation, with a concrete fix proposal.
   - `verdict = "not-a-violation"` — not a violation (legacy schema used `"accepted"`).

Each record: `low_path/high_path/changed_path`, `low_hash/high_hash`, `similarity_score`,
`threshold`, `base_commit`, `rationale`, `recorded_at`, `config_fingerprint`.

## Aggregate (16 tracks / 4,752 verdicts)

| verdict | count | share |
|---|---|---|
| violation (with proposal) | **273** | 5.7% |
| not-a-violation | 4,148 | 87.3% |
| accepted (= legacy not-a-violation) | 331 | 7.0% |

- Violations span **12 tracks** (largest: sot-chain 59, guard-process 57, dfp-rfp 45,
  telemetry 41, tddd 27).
- 273 violation records ↔ **67 distinct changed files / 124 distinct pairs** (fragment
  granularity multiplies records per real duplicate).
- Violation similarity: min 0.85 / mean 0.905 / max 1.0; **148 of 273 at sim < 0.90**.

## Assessment

### A. Judge quality — HIGH ✅
- Rationales are specific, behavioral, and divergence-aware (not generic). E.g.:
  - set_commit_hash dispatch: "both implement the same execution path; the dispatcher only adds
    arg parsing then duplicates existing logic → future changes could diverge" → "delegate to
    the existing handler."
  - CwdGuard: "both define the same test-only CWD guard; `review_v2/run.rs` already has the
    corresponding impl" (cross-references another copy) → "extract into a shared `test_support`."
- **Adds value beyond embedding**: 148/273 violations were flagged at sim < 0.90 (the judge
  confirms duplication that raw similarity rates only modestly).
- **Conservative on false positives** (correct true-negatives): waives layered error composition
  (domain port vs usecase error), distinct-semantics newtype constructors (trim vs preserve),
  incidental trait boilerplate (`Display`), header-only fragment matches (`impl X {` at sim 1.0),
  and tiny boundary-case test pairs.

### B. Coverage — STRUCTURALLY LIMITED ⚠️ (the main weakness)
- **Diff-scoped**: only fragments appearing in a track's diff are compared, so **pre-existing
  duplication that is never re-touched is invisible.**
- **Decisive evidence**: the census's top violation — the 5-way `validate_track_id` duplication —
  was **never evaluated head-to-head**. The 4 records touching `ids.rs` are incidental matches to
  unrelated newtypes/`Display` impls, all correctly waived. The highest-value violation is
  outside the gate's reach by construction.
- **Embedding gates the candidate set**: cross-layer copies have different surrounding context →
  lower similarity → never reach the judge. **Cross-layer knowledge-dup (the most valuable class)
  is systematically under-sampled.**
- **Data-dup blind**: constant/magic-value duplication is not embedding-detectable.

### C. Fragmentation & consistency — NOISY ⚠️
- 273 records ↔ ~67–124 real duplicates. The same duplicate splits into body-fragment =
  violation vs header-fragment = not-a-violation (96 not-a-violation at sim ≥ 0.985 are mostly
  header-only). The same `CwdGuard` is sometimes a violation, sometimes waived ("tiny local
  helper, no maintenance risk") — the "worth extracting?" judgment varies by fragment/run.

### D. Leniency bias — low FP, but lets aggregate dup grow ⚠️
- Many test-helper duplicates are waived as "small / test-only / not worth coupling." Defensible
  per instance, but in aggregate the codebase still accumulated `init_git_repo` ×6 and stub
  helpers ×3 (per the census). Per-instance leniency ≠ aggregate hygiene.

## Verdict

**A high-precision, low-recall, diff-scoped local-duplication detector.** The judge is excellent
(specific, conservative, actionable), but the design (diff-scope + embedding candidate-gating +
leniency) means it structurally misses the highest-value classes: (1) pre-existing duplication,
(2) cross-layer knowledge-dup, (3) data-dup. Engineering is solid (2-tier fast/final, config +
corpus fingerprinting, fail-closed on config drift).

This **explains the before/after census result**: density did not fall in gate-governed layers
and the flagship duplication persists — not because the judge is poor, but because the gate only
sees local duplication inside diffs. The gate is effective at **preventing new near-clones in
changed code** (it flagged 273) but is **not a codebase-wide DRY guarantee.**

## Recommendations (from the cache evidence)

1. **Dedup records per logical pair** (collapse body/header fragment splits) to cut noise and
   verdict flip-flop; judge once per pair.
2. **Raise cross-layer recall**: lower the candidate threshold for cross-layer pairs, and/or run
   a periodic **diff-independent full-corpus sweep** for pre-existing / cross-layer duplication.
3. **Curb test-helper leniency** with an aggregate rule (flag when the same helper shape appears
   in N≥3 modules).
4. **Add a data-dup gate** (clippy/grep) — embedding won't catch constants/magic values.

## Data basis

- Input: `track/items/*/dry-check.json` (16 files, 4,752 records), `dry-check-coverage.json`.
- Verdict vocabulary: `violation` (273) / `not-a-violation` (4,148) / `accepted` (331, legacy
  not-a-violation).
