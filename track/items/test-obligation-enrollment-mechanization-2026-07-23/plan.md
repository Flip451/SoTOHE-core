<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# テスト義務ゲートへの登録を機構化し、成果物不在による空振り合格を廃する

## Summary

GO-01: T001, T002, T003.

## Tasks (0/3 resolved)

### S1 — Fail-closed enrollment recognition

- [ ] **T001**: Update `libs/usecase/src/test_obligation/check.rs` (`CheckTestObligationsInteractor`) and `check_tests.rs`: add fail-closed enrollment validation and focused tests. IN-02; OUT-02, OUT-03; CN-01, CN-02, CN-03; AC-02, AC-03.

### S2 — Mandatory enrollment workflow

- [ ] **T002**: Update `.harness/workflows/track/type-design.md` and `.harness/capabilities/type-designer.md`: require the terminal derive and re-entry steps; retain `.claude/commands/track/type-design.md` and `.agents/skills/track-type-design/SKILL.md` as thin SSoT adapters. IN-01; CN-03; AC-01.

### S3 — Downstream workflow alignment

- [ ] **T003**: Update `.harness/workflows/track/{implement,full-cycle,obligation-fulfillment}.md`: normalize enrollment handoff instructions; keep provider adapters as SSoT references. IN-03; OUT-01, OUT-02; CN-02; AC-04.
