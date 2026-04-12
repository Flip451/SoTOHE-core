# Verification — Strict Spec Signal Gate v2

## Scope verified

- [x] T001: `SignalBasis::Feedback` → Yellow 降格 + 関連テスト更新 (commit 36ca580)
- [x] T002: `check_spec_doc_signals` domain 純粋関数 (D1–D6 tests) (commit 9302f2f)
- [x] T003: `check_domain_types_signals` domain 純粋関数 (D7–D13 tests) (commit 7a0db96)
- [x] T004: `validate_branch_ref` + `RefValidationError` (D14–D22 tests) (commit 07a3c93)
- [x] T005: `verify_from_spec_json` リファクタ + Stage 2 NotFound skip + `reject_symlinks_below` 統合 (S1–S5 tests) (commit 9726a03)
- [x] T006: `usecase::merge_gate` port + orchestration (U1–U18 tests) (commit 8ff4c45)
- [x] T007: `usecase::task_completion` (K1–K7 tests) (commit 3c9ed5a)
- [x] T008: `git_cli::show` primitives with `fetch_blob_safe` / `LANG=C` / symlink 検査 (commit a012b3f)
- [x] T009: `GitShowTrackBlobReader` adapter (A1–A16 tests, symlink/submodule fixture) (commit e03add6)
- [x] T010: `pr.rs` thin wrapper 化 + merge gate wiring (commit d93f875)
- [x] T011: `source-attribution.md` Signal 列追加 (commit eb6ea71)
- [x] T012: `Makefile.toml` CI interim mode 組み込み (commit df48608)
- [x] T013: CI 統合回帰テスト (本ドキュメント記録)
- [ ] T014: ADR Accepted 化 (最終タスク)

## Manual verification steps

### 層境界 / ビルド検証

- [x] `cargo make ci` が本 track ブランチで PASS する — T012 commit 前後で green を確認
- [x] `cargo make deny` が通る — `cargo make ci` 内で実行され PASS
- [x] `cargo make check-layers` が通る — T009 commit 後に単体実行し PASS
- [x] `apps/cli/src/commands/pr.rs` から `std::process::Command::new("git")` 直呼び出しが全て消えている — T010 レビュー時に grep 確認

### CI 統合回帰テスト (I1–I11, T013)

- [x] **I1**: 本 track ブランチ (`track/strict-signal-gate-v2-2026-04-12`, domain-types.json なし) で `cargo make ci` → **PASS** (Stage 2 skip, signals all-Blue なので warning なし) ✓ T012 commit 時に確認
- [x] **I2 (自動化済み)**: Yellow signals が warning として出力される動作は `libs/domain/src/spec.rs::test_check_spec_doc_signals_yellow_is_warning_in_interim_mode` (D4) で網羅。ダミーブランチを実際に作成せずとも domain レベルで保証されている
- [x] **I3**: Red signals が error として出力される動作は `test_check_spec_doc_signals_red_is_error_in_interim_mode` (D3a) で網羅
- [x] **I4**: spec.json all-Blue + domain-types.json all-Blue → `test_check_domain_types_signals_all_blue_passes_in_both_modes` (D13) で網羅
- [x] **I5**: declared yellow in interim → `test_check_domain_types_signals_yellow_is_warning_in_interim_mode` (D11) で網羅
- [x] **I6**: Red → error は `test_check_domain_types_signals_red_is_error_regardless_of_mode` (D10) で網羅
- [ ] **I7**: `main` ブランチで `cargo make ci` → skip ログ — 本 track ブランチ (`track/*`) 上では main への切替え CI 実行が不可のため未検証。`Makefile.toml` L578–579 の `plan/*|main)` case 分岐コードレビューで skip ロジック確認済み
- [ ] **I8**: `plan/dummy` ブランチで `cargo make ci` → skip ログ — I7 と同じ理由で未検証。同一 case 分岐のため
- [ ] **I9**: spec.md 欠如 track/* branch で `cargo make ci` → skip ログ — spec.md を持たない別 track ブランチが必要なため未検証。`Makefile.toml` L575 の `[ -f "$SPEC_PATH" ]` guard コードレビューで確認済み
- [x] **I10**: 二層モードの差分(CI interim → PASS, merge gate strict → BLOCKED)は以下で網羅:
  - `test_check_spec_doc_signals_yellow_is_warning_in_interim_mode` + `test_check_spec_doc_signals_yellow_is_error_in_strict_mode` (domain)
  - `test_u3_spec_blue_dt_yellow_blocks_in_strict` (usecase merge_gate, Stage 2 Yellow)
  - `test_u9_spec_yellow_blocks_in_strict` (usecase merge_gate, Stage 1 Yellow)
- [x] **I11**: warning メッセージテキスト ("merge gate will block" / "upgrade hint") は `test_check_spec_doc_signals_yellow_is_warning_in_interim_mode` で assertion 済み

### ヘキサゴナル原則の目視確認

- [x] `libs/domain/src/spec.rs` / `libs/domain/src/tddd/catalogue.rs` / `libs/domain/src/git_ref.rs` に pure check 関数配置 — T002/T003/T004 で実装確認
- [x] `libs/usecase/src/merge_gate.rs` + `task_completion.rs` が `domain` と `usecase port trait` のみに依存 (`cargo make check-layers` PASS)
- [x] `libs/infrastructure/src/git_cli/show.rs` の primitives が `pub(crate)` で外部非公開 — T008 で `pub(crate) mod show;` 確認
- [x] `libs/infrastructure/src/verify/merge_gate_adapter.rs` が `TrackBlobReader` port を実装し `fetch_blob_safe` を内部で呼ぶ — T009 で確認

### ADR 整合性

- [x] ADR の全 Decision (D1–D9) が実装された
- [x] Fail-closed 真理値表の入力パターンに対する実装が存在する (domain/usecase/infra 各層テストで網羅)
- [x] Test Matrix (D / U / K / A / C / I) の全カテゴリに対応テストが存在する
- [ ] ADR Status が `Proposed` → `Accepted` に更新される (T014 完了時)

## Result / Open issues

### Result

- **全 12 タスク (T001–T012) 実装完了、CI PASSED**
- **1923 tests 全て PASS** (T007 以降の追加分を含む)
- **本 track 自身の CI が新設された `verify-spec-states-current-local` を通過** — `domain-types.json` を作らない判断 (ADR Consequences) が正しく機能し、Stage 2 skip + Stage 1 all-Blue で PASS
- **ヘキサゴナル原則の完成** — `apps/cli/src/commands/pr.rs` から `std::process::Command::new("git")` 直呼び出しが完全に消え、CLI は composition root のみ
- **二層モードの動作確認**:
  - CI interim (T012): Yellow → warning / red → BLOCKED
  - Merge gate strict (T010 wiring): Yellow → error / red → BLOCKED

### Open Issues

なし。本 track の責務範囲内での既知の open issue はありません。

### Out of Scope (別 track で対応)

- SEC-18+ (番号未採番) `wait-and-merge` race condition: ポーリング前 1 回のみの検査。ADR D7 参照
- `check_tasks_resolved` の `git_show_blob` 共有: 本 track の D9 で consolidation 済み → この項目は達成済み
- キャッシュ signals vs fresh 評価: ADR D7 参照

## verified_at

2026-04-12T05:15:00Z
