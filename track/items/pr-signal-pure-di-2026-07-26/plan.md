<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# PR / Signal composition root を純 DI 化し、closure 注入と逆委譲 ServiceImpl を除去する

## Summary

GO-01 → T001, T002, T003, T004, T005, T006, T008. GO-02 → T001, T002, T003, T004, T005, T006, T007, T008.

## Tasks (7/8 resolved)

### pr-typed-boundary — PR typed boundary and secondary adapter

> Reuse PrCommandOutput, modify the existing PrCommandInteractor, and add SystemPrCommandAdapter. IN-01/CN-02/CN-03/CN-04/AC-03/AC-06. T001, T003.

- [x] **T001**: Add PrCommand and validated PR value objects at the driver-facing service port; modify the existing PrCommandInteractor and reuse the existing PrCommandOutput; add validation and delegation tests. IN-01/CN-02/CN-03/CN-04/AC-03/AC-06. (`6557ef6020cd950109f4e5ecf114022c9f3ce9f2`)
- [x] **T003**: Add SystemPrCommandAdapter for the existing PR filesystem, Git, GitHub, terminal, and polling components; add representative success and error tests. IN-01/CN-02/CN-04/AC-03/AC-06. (`6557ef6020cd950109f4e5ecf114022c9f3ce9f2`)

### signal-typed-boundary — Signal typed boundary and secondary adapter

> Add SignalCommand, SignalCommandPort, SignalCommandPortError, SignalFailureReason, SignalGateConfigError, SignalGateConfigPort, SignalRootSelection, SignalStrictOverride, ResolvedSignalChainCommand, SignalChainExecutionReport, SignalCommandInteractor, and SystemSignalCommandAdapter. IN-02/CN-02/CN-03/CN-04/AC-06. T002, T004.

- [x] **T002**: Add SignalCommand, SignalCommandPort, SignalCommandPortError, SignalFailureReason, SignalGateConfigError, SignalGateConfigPort, SignalRootSelection, SignalStrictOverride, ResolvedSignalChainCommand, SignalChainExecutionReport, and SignalCommandInteractor at the SignalService port; reuse SignalCommandOutput and SignalGateName; add command-translation and port-integration tests. T008 owns the later error-contract amendments. IN-02/CN-02/CN-03/CN-04/AC-06/AC-08/AC-10.
- [x] **T004**: Deliver SystemSignalCommandAdapter and SystemSignalGateConfigAdapter, including their signal execution and gate-settings loading behavior, with focused tests. IN-02/CN-02/CN-04/AC-06/AC-10.

### signal-error-parity — Signal error-contract amendments

> Implement the workspace-aware active-track preflight path across the command port, interactor, adapter, mocks, and focused tests. T008.

- [x] **T008**: Amend SignalCommandPort, SignalCommandInteractor, SignalCommandPortError, SignalFailureReason, SignalGateConfigError, SignalGateConfigPort, SignalRootSelection, SystemSignalCommandAdapter, and SystemSignalGateConfigAdapter; split active-track and spec-path resolution into SignalActiveTrackResolverPort and SignalSpecPathResolverPort, then align the interactor, adapters, mocks, and focused parity tests. IN-02/CN-01/AC-05/AC-06/AC-08/AC-09/AC-10.

### pr-cutover — PR composition cutover

> Wire PrDriver, PrCommandInteractor, and SystemPrCommandAdapter in the PR composition root; remove closure execution. IN-01/OUT-01/CN-01/CN-02/CN-04/AC-01/AC-03/AC-06. T005.

- [x] **T005**: Wire PrDriver, PrCommandInteractor, and SystemPrCommandAdapter in the PR composition root; remove closure-based command execution. IN-01/OUT-01/CN-01/CN-02/CN-04/AC-01/AC-03/AC-06. (`6557ef6020cd950109f4e5ecf114022c9f3ce9f2`)

### signal-cutover — Signal composition-root cutover

> Wire the Signal composition root and route the Signal CLI entries through the driver. IN-02/OUT-01/CN-01/CN-02/CN-04/AC-02/AC-04/AC-06/AC-07. T006.

- [x] **T006**: Deliver SignalCompositionRoot wiring for SignalDriver, SignalCommandInteractor, SystemSignalCommandAdapter, and SystemSignalGateConfigAdapter; move execution into the adapter, remove the composition execution surface, and switch all Signal CLI entries to the driver. IN-02/OUT-01/CN-01/CN-02/CN-04/AC-02/AC-04/AC-06/AC-07.

### contract-evidence-and-surface-cleanup — Contract evidence and live-reference verification

> Add apps/cli/tests/pr_signal_pure_di_contract.rs to exercise sotp pr push, ensure-pr, review-cycle, and signal check CLI contracts, including AC-07 single-path validation; extend apps/cli/tests/operational_reference_cutover.rs::test_live_operational_reference_surfaces_retired_planner_not_present for the live-reference check. IN-01/IN-02/OUT-02/OUT-04/CN-01/CN-02/CN-04/AC-01/AC-02/AC-04/AC-05/AC-06/AC-07. T007.

- [ ] **T007**: Add apps/cli/tests/pr_signal_pure_di_contract.rs to exercise sotp pr push, ensure-pr, review-cycle, and signal check CLI contracts, including AC-07 single-path validation; extend apps/cli/tests/operational_reference_cutover.rs::test_live_operational_reference_surfaces_retired_planner_not_present for the live-reference check. IN-01/IN-02/OUT-02/OUT-04/CN-01/CN-02/CN-04/AC-01/AC-02/AC-04/AC-05/AC-06/AC-07.
