<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# rustdoc child-item exclusion by kind instead of sentinel id

## Summary

GO-01 is implemented by T001. The task is limited to structural matching in the infrastructure type-signal evaluator; it preserves ordinary fields regardless of id, excludes module-kind children including the crate root, and leaves unrelated Id(0) placeholder and dangling-id behavior unchanged.
All six type catalogues are intentionally empty because this internal correction changes no public surface, so task-contract.json records no catalogue-entry attribution.

## Tasks (0/1 resolved)

### S1 — Kind-based rustdoc child traversal

> Update libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs and its co-located tests for child-item traversal. IN-01; IN-02; OUT-01; OUT-02; OUT-03; CN-01; CN-02; AC-01; AC-02; AC-03.

- [ ] **T001**: Update libs/infrastructure/src/tddd/signal_evaluator_v2/structural_eq.rs and its co-located tests for child-item traversal. Keep libs/infrastructure/src/tddd/signal_evaluator_v2/phase1/builder/phase16_check.rs and unrelated Id(0) uses unchanged. IN-01; IN-02; OUT-01; OUT-02; OUT-03; CN-01; CN-02; AC-01; AC-02; AC-03.
