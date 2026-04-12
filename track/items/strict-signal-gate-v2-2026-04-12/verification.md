# Verification — Strict Spec Signal Gate v2

## Scope verified

- [ ] T001: `SignalBasis::Feedback` → Yellow 降格 + 関連テスト更新
- [ ] T002: `check_spec_doc_signals` domain 純粋関数 (D1–D6 tests)
- [ ] T003: `check_domain_types_signals` domain 純粋関数 (D7–D13 tests)
- [ ] T004: `validate_branch_ref` + `RefValidationError` (D14–D22 tests)
- [ ] T005: `verify_from_spec_json` リファクタ + Stage 2 NotFound skip + `reject_symlinks_below` 統合 (S1–S5 tests)
- [ ] T006: `usecase::merge_gate` port + orchestration (U1–U18 tests)
- [ ] T007: `usecase::task_completion` (K1–K7 tests)
- [ ] T008: `git_cli::show` primitives with `fetch_blob_safe` / `LANG=C` / symlink 検査
- [ ] T009: `GitShowTrackBlobReader` adapter (A1–A16 tests, symlink/submodule fixture)
- [ ] T010: `pr.rs` thin wrapper 化 (C1–C4 tests)
- [ ] T011: `source-attribution.md` Signal 列追加
- [ ] T012: `Makefile.toml` CI interim mode 組み込み
- [ ] T013: CI 統合回帰テスト I1–I11
- [ ] T014: ADR Accepted 化

## Manual verification steps

### 層境界 / ビルド検証

- [ ] `cargo make ci` が本 track ブランチで PASS する
- [ ] `cargo make deny` が通る
- [ ] `cargo make check-layers` が通る (domain/usecase/infrastructure/cli の依存ルール違反なし)
- [ ] `apps/cli/src/commands/pr.rs` から `std::process::Command::new("git")` 直呼び出しが全て消えていることを Grep で確認

### CI 統合回帰テスト (I1–I11, T013 の記録対象)

手動で以下を実行し、結果を後述の「Result / Open Issues」に記入する。

- [ ] **I1**: 本 track ブランチ (`track/strict-signal-gate-v2-2026-04-12`, domain-types.json なし) で `cargo make ci` → PASS (Stage 2 skip, warning なし)
- [ ] **I2**: ダミー track branch を作成し spec.json に yellow>0 / red=0 を仕込んで `cargo make ci` → PASS + stdout に `[warning]` 行 (interim mode で yellow 許容 + 可視化)
- [ ] **I3**: ダミー track branch で spec.json に red>0 + `cargo make ci` → BLOCKED (`[error]` 出力, exit code != 0)
- [ ] **I4**: ダミー track branch で spec.json all-Blue + domain-types.json all-Blue + `cargo make ci` → PASS (warning なし)
- [ ] **I5**: ダミー track branch で spec.json all-Blue + domain-types.json declared yellow>0 + `cargo make ci` → PASS + stdout に `[warning]` 行 + yellow 型名リスト
- [ ] **I6**: ダミー track branch で spec.json all-Blue + domain-types.json red>0 + `cargo make ci` → BLOCKED
- [ ] **I7**: `main` ブランチで `cargo make ci` → skip ログ + ci 成功
- [ ] **I8**: `plan/dummy` ブランチで `cargo make ci` → skip ログ + ci 成功
- [ ] **I9**: spec.md が存在しない track branch で `cargo make ci` → skip ログ + ci 成功
- [ ] **I10**: I2 のダミー branch で `sotp pr wait-and-merge` の merge gate 経路 (または直接 `check_strict_merge_gate` 呼び出し) → BLOCKED (strict mode で yellow を error 扱い、二層モードの差分確認)
- [ ] **I11**: I2 の stdout に期待メッセージ文字列 (例: `"yellow signal(s) detected — merge gate will block"`) が含まれていることを grep で確認

### ヘキサゴナル原則の目視確認

- [ ] `libs/domain/src/spec.rs` / `libs/domain/src/tddd/catalogue.rs` / `libs/domain/src/git_ref.rs` に pure check 関数が配置されていること
- [ ] `libs/usecase/src/merge_gate/` が `domain` と `usecase port trait` のみに依存し、`infrastructure` を一切 use していないこと
- [ ] `libs/usecase/src/task_completion.rs` が同じ `TrackBlobReader` port を共有していること
- [ ] `libs/infrastructure/src/git_cli/show.rs` の primitives が `pub(crate)` で外部非公開になっていること
- [ ] `libs/infrastructure/src/verify/merge_gate_adapter.rs` が `TrackBlobReader` port を実装し、内部で `fetch_blob_safe` を呼んでいること

### ADR 整合性

- [ ] `knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md` の全 Decision (D1–D9) が実装されている
- [ ] Fail-closed 真理値表の全入力パターン (#1–#22) に対する実装が存在する
- [ ] Test Matrix (D / U / K / A / C / I) の全ケースに対応するテストが存在する
- [ ] ADR Status が `Proposed` → `Accepted` に更新されている (T014 完了後)

## Result / Open issues

(実装完了後に記入)

## verified_at

(実装完了後に記入 — ISO 8601 timestamp)
