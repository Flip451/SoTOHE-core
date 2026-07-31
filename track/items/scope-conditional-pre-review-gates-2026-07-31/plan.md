<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Scope Conditional Pre Review Gates

## Summary

GO-01 → T001、T002、T003。
GO-02 → T004、T005、T006。

## Tasks (1/6 resolved)

### S1 — 適用ポリシーと設定境界

> `libs/usecase/src/pre_review_gate_dispatch.rs`、`libs/infrastructure/src/pre_review_gate_config.rs`、`.harness/config/pre-review-gates.json` を T001–T003 の順に追加する。IN-01、OS-03、CN-01、AC-01。

- [x] **T001**: `libs/usecase/src/pre_review_gate_dispatch.rs` に `PreReviewGateKind`、`PreReviewGateMatrix`、matrix / lookup / load error、`PreReviewGateConfigLoaderPort` を追加し、`libs/usecase/src/lib.rs` から公開する。matrix validation と lookup の unit tests を追加する。IN-01、CN-01、AC-01。
- [ ] **T002**: `libs/infrastructure/src/pre_review_gate_config.rs` に `FsPreReviewGateConfigLoader` を実装し、infrastructure module に登録する。loader port の conformance / decode tests を追加する。IN-01、CN-01、AC-01。
- [ ] **T003**: `.harness/config/pre-review-gates.json` に scope × gate declaration を追加し、`libs/infrastructure/src/pre_review_gate_config.rs` に declaration fixture validation を追加する。IN-01、OS-03、CN-01、AC-01。

### S2 — dispatch と composition 接続

> `libs/usecase/src/pre_review_gate_dispatch.rs` の T004 を完了してから、`apps/cli-composition/src/review_v2/mod.rs` の T005 を更新する。IN-02、IN-03、IN-04、CN-02、CN-03、AC-02、AC-03、AC-04。

- [ ] **T004**: `libs/usecase/src/pre_review_gate_dispatch.rs` に `PreReviewGateDispatchService`、`PreReviewGateDispatchCommand`、`PreReviewGateDispatchError`、`PreReviewGateDispatchInteractor`、`PreReviewGateDispatchOutcome` を実装する。scope lookup、`NotApplicable`、`TaskContractLiveness` の全 path と result / error tests を追加する。IN-02、IN-03、IN-04、OS-01、OS-02、CN-02、CN-03、AC-02、AC-03、AC-04。
- [ ] **T005**: `apps/cli-composition/src/review_v2/mod.rs` の `ReviewCompositionRoot::review_run_local` に T002 の loader と T004 の dispatcher を配線し、local-review integration tests を追加する。IN-02、IN-03、IN-04、OS-01、OS-02、CN-02、CN-03、AC-02、AC-03、AC-04。

### S3 — Makefile wrapper

> T005 完了後に `Makefile.toml` と `apps/cli/tests/consumer_scaffold_host_first.rs` を T006 で更新する。IN-05、CN-02、AC-05。

- [ ] **T006**: `Makefile.toml` の `[tasks.track-local-review]` から scope-blind pre-review dependencies を削除し、delegated `bin/sotp review local` script を維持する。`apps/cli/tests/consumer_scaffold_host_first.rs` に wrapper validation を追加する。IN-05、CN-02、AC-05。
