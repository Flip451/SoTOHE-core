<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Spec signal evaluation: align implementation with ADR §D3.1 / §D3.2 (informal-priority)

## Summary

Fix two drifts within the same ADR 2026-04-19-1242: (a) evaluate_requirement_signal's adr_refs-priority contradicts §D3.1's informal-priority rule, (b) validate_track_snapshots' unconditional plan.md read contradicts §D0.0/§D1.4's Phase 0 identity-only responsibility.
Also codify three signal-evaluation decision criteria in .claude/agents/spec-designer.md to let spec-designer judge universal convention / per-element constraint / Yellow resolution autonomously (avoiding the round-trip observed in this track's Phase 1).
T001 fixes the signal logic in spec.rs + tests. T002 investigates the same drift in tddd/signals.rs (expected: no drift). T003 relaxes validate_track_snapshots to skip missing plan.md. T004 removes the now-incorrect max() lines from the ADR. T005 updates the spec-designer agent. T006 runs cargo make ci regression gate.

## Tasks (6/6 resolved)

### S1 — Domain signal logic fix

> Correct the informal-priority rule in evaluate_requirement_signal (spec.rs) and verify no analogous drift exists in the type catalogue signal evaluator. Tests are co-located with each fix.

- [x] **T001**: Fix evaluate_requirement_signal in spec.rs to implement informal-priority: return Yellow when adr_refs non-empty and informal_grounds non-empty; update and extend existing unit tests to cover the corrected logic (IN-01, IN-05) (`1c5011c1e2d8345e279b51c9451d343c591892df`)
- [-] **T002**: Investigate tddd/signals.rs for the same drift as IN-01: confirm whether evaluate_type_signals or any peer function ignores informal_grounds when adr_refs are non-empty, and apply the same informal-priority fix if found; record the outcome (IN-02 — expected: no drift, task closes as skipped or with a no-op commit)

### S2 — Infrastructure phase-boundary fix

> Make validate_track_snapshots resilient to Phase-0 tracks (metadata.json only, no plan.md yet) so that verify-track-metadata passes immediately after /track:init.

- [x] **T003**: Fix validate_track_snapshots in render.rs to skip plan.md content check when plan.md is absent (Phase 0 compatible); add a unit test that passes when the snapshot directory contains only metadata.json (IN-04, IN-05) (`a457865a0a42eff070bdab4d32a5fb074db54d89`)

### S3 — ADR text alignment

> Remove the now-incorrect max() composite-signal expressions from ADR §D3.1 and §D3.2, leaving all other definitions and anchors intact.

- [x] **T004**: Remove the max(adr_refs, informal_grounds) line from ADR §D3.1 and the max(spec_refs, informal_grounds) line from ADR §D3.2; preserve all other lines and anchors (IN-03) (`ba31bb802bb815b6e0007694171efe56e2e272fc`)

### S4 — Agent guidance update

> Codify three signal-evaluation decision criteria in the spec-designer agent definition so future spec sessions converge without back-and-forth.

- [x] **T005**: Append three signal-evaluation decision criteria to .claude/agents/spec-designer.md: (1) universal coding principles belong in related_conventions[] not per-element refs, (2) convention_refs[] do not contribute to signal evaluation so adr_refs[]+informal_grounds[] both empty means Red, (3) Yellow informal_grounds[] must be resolved before merge via one of three options (IN-06) (`9cb014ba176699219692012c4c44d34f63626c80`)

### S5 — CI regression gate

> Full cargo make ci run to confirm that all quality checks pass after the above changes.

- [x] **T006**: Run cargo make ci regression gate; confirm all checks pass (fmt-check, clippy, nextest, test-doc, deny, check-layers, verify-track-metadata, verify-plan-artifact-refs, all others) (CN-01, AC-04)
