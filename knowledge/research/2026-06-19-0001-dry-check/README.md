# DRY Census & Gate Evaluation — 2026-06-19

Independent, AI-based census of DRY-principle violations across the SoTOHE Rust
workspace, run to evaluate the `sotp dry` (dry-checker) gate. Promoted from
`tmp/research/` to this tracked location at the maintainer's request.

## Provenance & status

- **Method**: an independent AI census (intra-unit + thematic finders → adversarial
  verification), run identically against two checkouts. This is **not** the gate's own
  embedding detector; it is a tool-independent second opinion.
- **Snapshots**:
  - `before` = `c4da67a4` — main immediately before PR #148 (dry-checker gate) merged (2026-06-02)
  - `after`  = `9270de33` — latest main (2026-06-19); gate active ~17 days / ~16 PRs
- **Confidence**: findings are model-generated (claude-sonnet-4-6 finders, adversarially
  verified, `unverifiedKept = 0` on both runs). Treat counts as well-grounded estimates,
  not exhaustive ground truth; a single run per snapshot implies stochastic noise on small
  sub-counts. The per-layer and qualitative conclusions are robust; small per-category
  deltas are not.

## Contents

| Path | What |
|---|---|
| `evaluation.md` | before/after gate evaluation (density decomposition + verdict) |
| `gate-cache-evaluation.md` | gate quality assessed from its own cached verdicts |
| `before/findings.json` | 148 confirmed findings @ `c4da67a4` |
| `before/summary.json` | aggregate stats @ `c4da67a4` |
| `before/report.md` | human-readable enumeration @ `c4da67a4` |
| `after/findings.json` | 168 confirmed findings @ `9270de33` |
| `after/summary.json` | aggregate stats @ `9270de33` |
| `after/report.md` | human-readable enumeration @ `9270de33` |
| `methodology/partition.awk` | deterministic unit partitioner (~9k LOC/unit) |
| `methodology/dry-scan.workflow.js` | the scan workflow (load → finders → verify → synth) |
| `methodology/report-gen.jq` | `report.md` generator |

## Headline numbers

| metric | before | after |
|---|---|---|
| LOC (5 crates, `src/`) | 157,104 | 199,423 |
| confirmed findings | 148 | 168 |
| density (per KLoc) | 0.942 | 0.842 |
| weighted density (high×3/med×2/low×1 per KLoc) | 1.776 | 1.539 |
| cross-layer findings | 26 | 21 |
| knowledge-dup | 7 | 4 |

## Scope

- **Covered**: `src/` of the 5 first-party crates (`libs/{domain,usecase,infrastructure}`,
  `apps/{cli,cli-composition}`), including inline `#[cfg(test)]` modules.
- **Excluded**: `vendor/**` (vendored third-party) and integration `tests/` directories.

## Related

- `knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md` — the remediation ADR
  for which this census is the evidentiary basis.
- DRY gate ADRs under `knowledge/adr/` (dry-checker / DFP⇄RFP family) — the gate being evaluated.
