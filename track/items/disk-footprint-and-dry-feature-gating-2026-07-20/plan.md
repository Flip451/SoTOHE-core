<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# ビルド成果物によるディスク圧迫の解消と dry gate 重量依存の feature flag 化

## Summary

GO-01 → T001, T002
GO-02 → T002, T006
GO-03 → T003, T005
GO-04 → T004

## Tasks (6/6 resolved)

### S1 — Default-off semantic-dup feature boundary

> `libs/infrastructure/{Cargo.toml,src/{lib.rs,semantic_dup/**}}` の cfg gate 化を T001 で実施し、その後 `apps/cli-composition/` と `apps/cli/` の feature 伝播を T002 で実施する。IN-01, IN-02, CN-01, CN-02, AC-01, AC-02。

- [x] **T001**: `libs/infrastructure/Cargo.toml` の `semantic-dup` feature と重量依存の宣言、`libs/infrastructure/src/{lib.rs,semantic_dup/**}` の公開 API、ならびに `dry_check/{dry_write_driver.rs,corpus_meta.rs,dry_write_driver/{fragments.rs,manifest.rs,persistent_index.rs}}` と `track/fixpoint_resolve_driver.rs` の consumer を cfg gate または feature-off 実装へ切り替え、feature-on/off build coverage を追加する。IN-01, CN-01, AC-01。 (`b5d9cf49a22a8f175b626c73105260bcc66a0255`)
- [x] **T002**: `apps/cli-composition/Cargo.toml`、`apps/cli-composition/src/{lib.rs,semantic_dup/**}`、`apps/cli/{Cargo.toml,src/{main.rs,commands/{dry.rs,semantic_dup.rs}}}` に `semantic-dup` feature 伝播と、gate 評価点以外の feature-off dry command dispatch を実装し、両 build 構成の command coverage を追加する。check-approved gate 評価点の enabled 優先分岐は T006 が担う。IN-01, IN-02, CN-01, CN-02, AC-01, AC-02。 (`b5d9cf49a22a8f175b626c73105260bcc66a0255`)

### S2 — Configurable disk maintenance

> 新規 `.harness/config/disk-maintenance.toml` と `Makefile.toml` の設定読取・maintenance task を T003 で追加済みとする。IN-03, CN-03, AC-03, AC-04。T005 で `apps/cli-driver/src/maintenance.rs` を command/query driver・input family へ移行し、`apps/cli/src/commands/maintenance.rs`・`apps/cli/src/main.rs` の CLI dispatch と `apps/cli-composition/src/maintenance.rs` の両 driver wiring を追随させて旧 driver/input を除去する。IN-05, AC-06。

- [x] **T003**: 新規 `.harness/config/disk-maintenance.toml` と `Makefile.toml`、domain/usecase の disk-maintenance、`FsDiskMaintenanceAdapter` を実装し、設定読取・検証、sccache 上限設定、target/.cache cleanup の dry-run/apply 実行を追加する。IN-03, CN-03, AC-03, AC-04。 (`b5d9cf49a22a8f175b626c73105260bcc66a0255`)
- [x] **T005**: maintenance の分割 migration として、`apps/cli-driver/src/maintenance.rs` の旧 `MaintenanceDriver`/`MaintenanceInput` を `MaintenanceCommandDriver`/`MaintenanceCommandInput` と `MaintenanceQueryDriver`/`MaintenanceQueryInput` に置換する。`apps/cli/src/commands/maintenance.rs` と `apps/cli/src/main.rs` の CLI command 定義・dispatch を各 driver/input family へ追随させ、`apps/cli-composition/src/maintenance.rs` を両 driver を返す wire-only 構成へ移行する。旧2型を除去し、command/query の各実行経路を検証する。IN-05, AC-06。 (`b5d9cf49a22a8f175b626c73105260bcc66a0255`)

### S3 — Feature-aware continuous verification

> T001/T002 後に `.github/workflows/ci.yml` と `Makefile.toml` の CI entry points および `build-sotp` を T004 で更新する。IN-04, CN-04, AC-05。

- [x] **T004**: `.github/workflows/ci.yml` と `Makefile.toml` の clippy/test CI entry points、および `build-sotp` を更新し、feature-on CI と default feature-off binary build の checks を追加する。IN-04, CN-04, AC-05。 (`b5d9cf49a22a8f175b626c73105260bcc66a0255`)

### S4 — Enabled-first DRY gate evaluation

> T006 で `libs/usecase/src/dry_check_approved_driver.rs`、`apps/cli-driver/src/dry.rs`、`apps/cli-composition/src/dry_gate.rs`、`apps/cli/src/commands/dry.rs` の check-approved gate evaluation path と AC-07 の feature-off 両分岐 command coverage を追加・接続する。IN-07, AC-07。

- [x] **T006**: `libs/usecase/src/dry_check_approved_driver.rs` の gate interactor、`apps/cli-driver/src/dry.rs` の専用 CLI driver、`apps/cli-composition/src/dry_gate.rs` の composition root、`apps/cli/src/commands/dry.rs` の check-approved dispatch を実装し、旧 `execute_dry_check_approved` を除去する。AC-07 の feature-off 両分岐に対する command coverage を追加する。IN-07, AC-07。 (`0d61b54ff04901ef6776749776c4ded3ec57f09a`)
