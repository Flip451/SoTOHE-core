<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# D15 実装 — test-obligation check の task-status 連動判定（todo 帰属 🟡 許容）

## Summary

Goal coverage note: task-coverage.json maps in_scope, out_of_scope, constraints, and acceptance_criteria only. GO-01 is carried by T002/T006; GO-02 is carried by T001/T002/T003/T004/T005/T006. Goal mappings are recorded here because task-coverage.json has no goal section.

## Tasks (6/6 resolved)

### S1 — Domain error vocabulary

> Target domain vocabulary definitions; update their implementation (IN-04/CN-02/AC-02).

- [x] **T001**: Target libs/domain/src/tddd/test_obligation/errors.rs and colocated unit tests. Update ObligationCheckError (IN-04/CN-02/AC-02). (`5141873e2eb9e20622e52c540b85d85adf84eac2`)

### S2 — Usecase status-lane interpretation

> Target usecase check and results modules; implement status-lane changes (IN-01/IN-02/IN-03/IN-04/IN-05/IN-06/IN-07/OUT-01/OUT-02/OUT-03/OUT-04/CN-01/CN-02/CN-03/CN-04/AC-01/AC-02/AC-03/AC-04/AC-05/AC-06/AC-07).

- [x] **T002**: Target libs/usecase/src/test_obligation/{mod.rs,check.rs}, libs/usecase/src/pre_review_gate.rs, and their unit tests. Implement explicit-constructor status-lane interpretation in CheckTestObligationsInteractor and PreReviewGate using TaskStatusKind (IN-01/IN-02/IN-03/IN-04/IN-06/IN-07/OUT-01/OUT-02/OUT-03/OUT-04/CN-01/CN-02/CN-03/CN-04/AC-01/AC-02/AC-03/AC-04/AC-06/AC-07). (`5141873e2eb9e20622e52c540b85d85adf84eac2`)
- [x] **T003**: Target libs/usecase/src/test_obligation/results.rs and its unit tests. Implement TestObligationResultsInteractor status-lane aggregation (IN-01/IN-05/IN-06/IN-07/OUT-01/OUT-02/OUT-04/CN-02/CN-03/AC-05/AC-07). (`5141873e2eb9e20622e52c540b85d85adf84eac2`)

### S3 — CLI-driver rendering

> Target CLI-driver check and results handlers; implement rendering changes (IN-02/IN-04/IN-05/IN-07/CN-01/CN-02/CN-04/AC-01/AC-02/AC-04/AC-05/AC-06/AC-07).

- [x] **T004**: Target apps/cli-driver/src/test_obligation/check.rs and its adapter tests. Implement TestObligationCheckHandler rendering (IN-02/IN-04/IN-07/CN-01/CN-02/CN-04/AC-01/AC-02/AC-04/AC-06/AC-07). (`5141873e2eb9e20622e52c540b85d85adf84eac2`)
- [x] **T005**: Target apps/cli-driver/src/test_obligation/results.rs and its adapter tests. Implement TestObligationResultsHandler rendering (IN-05/IN-07/CN-02/AC-05/AC-07). (`5141873e2eb9e20622e52c540b85d85adf84eac2`)

### S4 — Composition wiring and incremental obligation fulfillment

> Target TestObligationCompositionRoot and test-bindings.json; wire handlers and update test-obligation bindings (IN-01/IN-05/OUT-01/OUT-03/AC-05).

- [x] **T006**: Target apps/cli-composition/src/test_obligation/*, track/items/d15-task-status-check-gate-2026-07-11/test-bindings.json, and composition tests. Wire TestObligationCompositionRoot for the check and results handlers, and update implementation-batch test-obligation bindings (IN-01/IN-05/OUT-01/OUT-03/AC-05). (`5141873e2eb9e20622e52c540b85d85adf84eac2`)
