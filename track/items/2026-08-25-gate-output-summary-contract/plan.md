<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# ゲートの標準出力をサマリ契約にする

## Summary

T001-T003: apply gate-output changes across shared, leaf, and aggregate paths (GO-01).
T001-T003: update gate-output handling in place (GO-02).
T004: libs/usecase/src/gate_output.rs, libs/infrastructure/src/gate_output.rs, and apps/cli-driver/src/gate_output.rs reservation/preparation symbols and focused regressions — add and test; IN-04; CN-04; CN-05; AC-05.
T005: libs/usecase/src/gate_output.rs and libs/infrastructure/src/gate_output.rs reservation-consumption/no-reclaim targets plus focused regressions — add and test; IN-04; CN-05; CN-07; AC-08.
T006: libs/usecase/src/gate_output.rs and libs/infrastructure/src/gate_output.rs final-publish containment targets plus focused regressions — add and test; IN-05; CN-06; AC-07.
T007: libs/usecase/src/gate_output.rs, apps/cli-driver/src/gate_output.rs, and conditional CLI/composition integration targets for post-execution write-outcome/result rendering plus focused regressions — add and test; IN-02; IN-03; CN-01; AC-02; AC-03; AC-06.

## Tasks (5/7 resolved)

### S1 — Shared summary and full-log behavior

> Layered shared gate-run mechanism across usecase, infrastructure, cli_driver, cli, and cli_composition, plus Makefile.toml and bin/sotp integration and focused regressions — add and test; GO-01; GO-02; IN-02; IN-03; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.

- [x] **T001**: Layered shared gate-run mechanism across usecase, infrastructure, cli_driver, cli, and cli_composition, plus Makefile.toml and bin/sotp integration and focused regressions — standardize and test; GO-01; GO-02; IN-02; IN-03; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04. (`415476acda9074085f5283a265e6b724a05c485d`)

### S2 — Leaf test and obligation tasks

> Makefile.toml and bin/sotp test-execution and obligation-evaluation tasks plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.

- [x] **T002**: Makefile.toml and bin/sotp test-execution and obligation-evaluation tasks plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04. (`be8b26e64ad6efd70f4b6f4cb974cb946001d217`)

### S3 — Aggregate pre-commit gates

> Makefile.toml and bin/sotp pre-commit aggregate gates plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.

- [x] **T003**: Makefile.toml and bin/sotp pre-commit aggregate gates plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04. (`16a3bcd17a8a95146184f179e357a59ebba8a248`)

### S4 — Reservation and preparation targets

> libs/usecase/src/gate_output.rs (GateLogPersistencePort::reserve, GateLogReservation, GateLogReservationError including CreateFile, GateRunCommand, GateRunCommandError, GateRunError::Prepare, and the GateRunInteractor pre-launch path), libs/infrastructure/src/gate_output.rs (FsGateLogPersistence::reserve), apps/cli-driver/src/gate_output.rs (GateOutputDriver preparation-failure rendering), and focused regressions in those modules — add and test; IN-04; CN-04; CN-05; AC-05.

- [x] **T004**: libs/usecase/src/gate_output.rs (GateLogPersistencePort::reserve, GateLogReservation, GateLogReservationError including CreateFile, GateRunCommand, GateRunCommandError, GateRunError::Prepare, and the GateRunInteractor pre-launch path), libs/infrastructure/src/gate_output.rs (FsGateLogPersistence::reserve), apps/cli-driver/src/gate_output.rs (GateOutputDriver preparation-failure rendering), and focused regressions in those modules — add and test; IN-04; CN-04; CN-05; AC-05. (`888eb4aee35f01653d76f1d8f66f82b66c49d894`)

### S5 — Reservation consumption and no-reclaim targets

> libs/usecase/src/gate_output.rs (GateLogPersistencePort::persist consuming GateLogReservation and the GateRunInteractor single-live-reservation flow) and libs/infrastructure/src/gate_output.rs (FsGateLogPersistence reservation-consumption/no-reclaim behavior), plus focused regressions in those modules — add and test; IN-04; CN-05; CN-07; AC-08.

- [x] **T005**: libs/usecase/src/gate_output.rs (GateLogPersistencePort::persist consuming GateLogReservation and the GateRunInteractor single-live-reservation flow) and libs/infrastructure/src/gate_output.rs (FsGateLogPersistence reservation-consumption/no-reclaim behavior), plus focused regressions in those modules — add and test; IN-04; CN-05; CN-07; AC-08. (`d4f877064d0f392a799e1c9766ea710660f3e508`)

### S6 — Contained final-publication targets

> libs/usecase/src/gate_output.rs (GateLogPath, GateLogWriteError, GateLogWriteOutcome, and GateLogPersistencePort::persist result contract) and libs/infrastructure/src/gate_output.rs (FsGateLogPersistence final-publish containment), plus focused regressions in those modules — add and test; IN-05; CN-06; AC-07.

- [ ] **T006**: libs/usecase/src/gate_output.rs (GateLogPath, GateLogWriteError, GateLogWriteOutcome, and GateLogPersistencePort::persist result contract) and libs/infrastructure/src/gate_output.rs (FsGateLogPersistence final-publish containment), plus focused regressions in those modules — add and test; IN-05; CN-06; AC-07.

### S7 — Post-execution outcome and rendering targets

> libs/usecase/src/gate_output.rs (GateLogWriteOutcome, GateRunService::execute, GateRunResult, and GateRunInteractor post-execution write outcome), apps/cli-driver/src/gate_output.rs (render_summary and GateOutputDriver result rendering), apps/cli/src/commands/gate_output.rs (execute if the result shape changes), apps/cli-composition/src/gate_output.rs (GateOutputComposition if wiring changes), and focused regressions in those modules — add and test; IN-02; IN-03; CN-01; AC-02; AC-03; AC-06.

- [ ] **T007**: libs/usecase/src/gate_output.rs (GateLogWriteOutcome, GateRunService::execute, GateRunResult, and GateRunInteractor post-execution write outcome), apps/cli-driver/src/gate_output.rs (render_summary and GateOutputDriver result rendering), apps/cli/src/commands/gate_output.rs (execute if the result shape changes), apps/cli-composition/src/gate_output.rs (GateOutputComposition if wiring changes), and focused regressions in those modules — add and test; IN-02; IN-03; CN-01; AC-02; AC-03; AC-06.
