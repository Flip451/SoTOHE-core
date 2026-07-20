<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 入力決定と pipeline 産決定の二箱分離への運用文書整合

## Summary

GO-01 is covered by T001, T002, and T003; enforced element mappings are recorded in task-coverage.json.
This documentation-only plan has no type-catalogue implementation entries; task-contract.json records empty attribution lists for every task.

## Tasks (3/3 resolved)

### S1 — Phase 0 convention boundary

> Execute T001.

- [x] **T001**: `knowledge/conventions/pre-track-adr-authoring.md`: revise the in-track meaning-change authority section, then perform the AC-01 acceptance check (IN-01/CN-01/CN-02/AC-01).

### S2 — ADR capability contract alignment

> Execute T002 before T003.

- [x] **T002**: The `adr-diagnoser` and `adr-editor` capability contracts and corresponding `.agents/skills` descriptions: align the capability contracts, then perform the AC-02 and AC-04 acceptance checks (IN-02/IN-04/CN-01/CN-02/AC-02/AC-04).

### S3 — Workflow and adapter realignment

> Execute T003 after T001 and T002.

- [x] **T003**: The track-plan, track-review, track-adr2pr, and merge workflow SSoTs with their thin adapters: realign the workflow documents, then perform the AC-03, AC-04, and AC-05 acceptance checks (IN-03/IN-04/CN-01/CN-02/AC-03/AC-04/AC-05).
