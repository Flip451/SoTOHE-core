<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# PR / Signal composition root を純 DI 化し、closure 注入と逆委譲 ServiceImpl を除去する

## Summary

GO-01 → T001–T006 establish the typed PR and Signal execution paths; T007 supplies their CLI contract evidence. GO-02 → T001–T007 delivers an independently CI-green migration slice, with T007 validating parity and live references.

## Tasks (0/7 resolved)

### pr-typed-boundary — PR typed boundary and secondary adapter

> Reuse PrCommandOutput, modify the existing PrCommandInteractor, and add SystemPrCommandAdapter. IN-01/CN-02/CN-03/CN-04/AC-03/AC-06. T001, T003.

- [ ] **T001**: Add PrCommand and validated PR value objects at the driver-facing service port; modify the existing PrCommandInteractor and reuse the existing PrCommandOutput; add validation and delegation tests. IN-01/CN-02/CN-03/CN-04/AC-03/AC-06.
- [ ] **T003**: Add SystemPrCommandAdapter for the existing PR filesystem, Git, GitHub, terminal, and polling components; add representative success and error tests. IN-01/CN-02/CN-04/AC-03/AC-06.

### signal-typed-boundary — Signal typed boundary and secondary adapter

> Add SignalCommandInteractor and SystemSignalCommandAdapter. IN-02/CN-02/CN-03/CN-04/AC-06. T002, T004.

- [ ] **T002**: Add SignalCommand and SignalCommandInteractor at the SignalService port; reuse the existing SignalCommandOutput; add command-translation and port-delegation tests. IN-02/CN-02/CN-03/CN-04/AC-06.
- [ ] **T004**: Add SystemSignalCommandAdapter; add representative success and error tests. IN-02/CN-02/CN-04/AC-06.

### pr-cutover — PR composition cutover

> Wire PrDriver, PrCommandInteractor, and SystemPrCommandAdapter in the PR composition root; remove closure execution. IN-01/OUT-01/CN-01/CN-02/CN-04/AC-01/AC-03/AC-06. T005.

- [ ] **T005**: Wire PrDriver, PrCommandInteractor, and SystemPrCommandAdapter in the PR composition root; remove closure-based command execution. IN-01/OUT-01/CN-01/CN-02/CN-04/AC-01/AC-03/AC-06.

### signal-cutover — Signal composition and CLI cutover

> Wire SignalDriver, SignalCommandInteractor, and SystemSignalCommandAdapter in the Signal composition root and CLI command call sites. IN-02/OUT-01/CN-01/CN-02/CN-04/AC-02/AC-04/AC-06/AC-07. T006.

- [ ] **T006**: Wire SignalDriver, SignalCommandInteractor, and SystemSignalCommandAdapter in the Signal composition root and CLI command call sites; remove runtime uses of SignalServiceImpl and the shim. IN-02/OUT-01/CN-01/CN-02/CN-04/AC-02/AC-04/AC-06/AC-07.

### contract-evidence-and-surface-cleanup — Contract evidence and live-reference verification

> Add apps/cli/tests/pr_signal_pure_di_contract.rs to exercise sotp pr push, ensure-pr, review-cycle, and signal check CLI contracts, including AC-07 single-path validation; extend apps/cli/tests/operational_reference_cutover.rs::test_live_operational_reference_surfaces_retired_planner_not_present for the live-reference check. IN-01/IN-02/OUT-02/OUT-04/CN-01/CN-02/CN-04/AC-01/AC-02/AC-04/AC-05/AC-06/AC-07. T007.

- [ ] **T007**: Add apps/cli/tests/pr_signal_pure_di_contract.rs to exercise sotp pr push, ensure-pr, review-cycle, and signal check CLI contracts, including AC-07 single-path validation; extend apps/cli/tests/operational_reference_cutover.rs::test_live_operational_reference_surfaces_retired_planner_not_present for the live-reference check. IN-01/IN-02/OUT-02/OUT-04/CN-01/CN-02/CN-04/AC-01/AC-02/AC-04/AC-05/AC-06/AC-07.
