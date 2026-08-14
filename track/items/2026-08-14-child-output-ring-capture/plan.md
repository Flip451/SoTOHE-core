<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 子プロセスの診断出力を末尾リングで保持し、出力量による kill を廃止する

## Summary

GO-01 → T001, T002, T003.

## Tasks (3/3 resolved)

### S1 — Review-fix diagnostic capture

> Update review-fix runner diagnostic capture and focused infrastructure regressions. [D1; IN-01; IN-03; CN-02; AC-01; AC-03]

- [x] **T001**: Update the review-fix runner's diagnostic capture and focused infrastructure regressions. [D1; D2; IN-01; IN-03; OS-01; CN-02; CN-03; AC-01; AC-03] (`a700c6810f60a85c256919e37ef55924591bcb19`)

### S2 — Program-runner capture contract and adapter

> Update the program-runner outcome contract, infrastructure adapter, and related regressions. [IN-02; CN-01; CN-03; AC-02; AC-04]

- [x] **T002**: Apply the catalogued captured-stream outcome contract to the program runner and infrastructure adapter; update usecase and adapter regressions, including the ProgramRunnerPort contract obligation. [D1; IN-02; OS-02; OS-03; CN-01; CN-03; AC-02; AC-04]

### S3 — Phase-command propagation

> Update downstream outcome rendering and regression coverage after the program-runner contract is available. [IN-02; OS-02; CN-03; AC-02; AC-04]

- [x] **T003**: Adapt phase-command rendering and CLI/composition regressions to the captured-stream outcome. [D1; D2; IN-02; OS-02; CN-03; AC-02; AC-04]
