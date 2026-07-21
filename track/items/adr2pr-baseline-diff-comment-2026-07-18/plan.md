<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# adr2pr 終端に ADR baseline diff の PR コメント投稿フェーズを追加

## Summary

GO-01: T001, T002, T003

## Tasks (3/3 resolved)

### S1 — Terminal workflow phase

> `.harness/workflows/track/adr2pr.md`: terminal-phase SSoT update, anchored to ADR D1/D2 (IN-01/IN-02/CN-01/CN-02/CN-03/CN-04/AC-01/AC-02/AC-03/AC-04/AC-05/AC-06).

- [x] **T001**: `.harness/workflows/track/adr2pr.md`: add the terminal ADR-baseline comment phase after Step 10, including its gate, recovery, constraint, and output references, anchored to `knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md` D1/D2 (IN-01/IN-02/CN-01/CN-02/CN-03/CN-04/AC-01/AC-02/AC-03/AC-04/AC-05/AC-06). (`5e124454cb22f4f719e0e5d30cb0dd07298820b9`)

### S2 — Provider adapter reports

> `.claude/commands/track/adr2pr.md`; `.agents/skills/track-adr2pr/SKILL.md`: reporting-format alignment with the workflow SSoT, anchored to ADR D1 (IN-01/CN-04/AC-01/AC-06).

- [x] **T002**: `.claude/commands/track/adr2pr.md`; `.agents/skills/track-adr2pr/SKILL.md`: align the terminal completion reports with the workflow SSoT's added phase, without duplicating workflow logic, anchored to `knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md` D1 and `knowledge/conventions/no-upstream-restatement.md` Rules (IN-01/CN-04/AC-01/AC-06). (`5e124454cb22f4f719e0e5d30cb0dd07298820b9`)

### S3 — Escalation provenance convention

> `knowledge/conventions/pre-track-adr-authoring.md` 機構との整合: D3 clarification (IN-03/AC-07).

- [x] **T003**: `knowledge/conventions/pre-track-adr-authoring.md` 機構との整合: add the D3 escalation-reason clarification sentence, anchored to `knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md` D3 (IN-03/AC-07). (`5e124454cb22f4f719e0e5d30cb0dd07298820b9`)
