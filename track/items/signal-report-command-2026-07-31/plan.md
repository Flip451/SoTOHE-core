<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Signal Report Command

## Summary

GO-01 → T001–T006.
IN-02 / CN-02 / AC-02 → T001, T004.
IN-03 / AC-03 → T001, T003, T005.
IN-04 / CN-01 / AC-01 / AC-04 → T001, T002, T005, T006.
OUT-01 / AC-05 → T005.

## Tasks (0/6 resolved)

### S1 — Usecase boundary — typed report query

> Implement `libs/usecase/src/signal_report/`. GO-01, IN-01–IN-04, CN-01, CN-02, AC-01–AC-04.

- [ ] **T001**: Implement the catalogue-defined typed report-query boundary in `libs/usecase/src/signal_report/`. GO-01, IN-01–IN-04, CN-01, CN-02, AC-01–AC-04.

### S2 — Infrastructure — report source adapter

> Implement `libs/infrastructure/src/signal_report/`. IN-04, CN-01, AC-01, AC-04.

- [ ] **T002**: After T001, implement the catalogue-defined report source adapter in `libs/infrastructure/src/signal_report/`. IN-04, CN-01, AC-01, AC-04.

### S3 — CLI driver — query conversion and rendering

> Implement filter conversion and rendering in `apps/cli-driver/src/signal_report/`. IN-02, IN-03, CN-02, AC-02, AC-03.

- [ ] **T003**: After T001, implement filter-to-query conversion in `apps/cli-driver/src/signal_report/`. IN-03, AC-03.
- [ ] **T004**: After T001 and T003, implement occurrence rendering in `apps/cli-driver/src/signal_report/`. IN-02, CN-02, AC-02.

### S4 — CLI transport and composition root

> Wire and verify the composition-root report driver before CLI parsing and dispatch consume it; retain the `sotp track resolve` non-regression validation with the CLI task. IN-01, IN-03, OUT-01, CN-01, AC-01, AC-03–AC-05.

- [ ] **T006**: After T001, T002, and T004, implement and independently verify `SignalCompositionRoot::signal_report_driver` wiring in `apps/cli-composition/`, before CLI report dispatch consumes it. CN-01, AC-01, AC-04.
- [ ] **T005**: After T003, T004, and T006, add signal-report CLI parsing and dispatch through the composition-root report driver, plus `sotp track resolve` regression validation in `apps/cli/`. IN-01, IN-03, OUT-01, AC-01, AC-03, AC-05.
