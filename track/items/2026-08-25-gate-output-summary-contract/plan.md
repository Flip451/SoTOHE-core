<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# ゲートの標準出力をサマリ契約にする

## Summary

T001-T003: apply gate-output changes across shared, leaf, and aggregate paths (GO-01).
T001-T003: update gate-output handling in place (GO-02).

## Tasks (2/3 resolved)

### S1 — Shared summary and full-log behavior

> Layered shared gate-run mechanism across usecase, infrastructure, cli_driver, cli, and cli_composition, plus Makefile.toml and bin/sotp integration and focused regressions — add and test; GO-01; GO-02; IN-02; IN-03; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.

- [x] **T001**: Layered shared gate-run mechanism across usecase, infrastructure, cli_driver, cli, and cli_composition, plus Makefile.toml and bin/sotp integration and focused regressions — standardize and test; GO-01; GO-02; IN-02; IN-03; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04. (`415476acda9074085f5283a265e6b724a05c485d`)

### S2 — Leaf test and obligation tasks

> Makefile.toml and bin/sotp test-execution and obligation-evaluation tasks plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.

- [x] **T002**: Makefile.toml and bin/sotp test-execution and obligation-evaluation tasks plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.

### S3 — Aggregate pre-commit gates

> Makefile.toml and bin/sotp pre-commit aggregate gates plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.

- [ ] **T003**: Makefile.toml and bin/sotp pre-commit aggregate gates plus output-dependent tests — migrate and revise; GO-01; GO-02; IN-01; IN-02; IN-03; OUT-01; OUT-02; CN-01; CN-02; CN-03; AC-01; AC-02; AC-03; AC-04.
