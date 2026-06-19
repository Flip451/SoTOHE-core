<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 25, yellow: 0, red: 0 }
---

# 既存 DRY 違反の一掃 — 横断・既存重複を正典へ集約する

## Goal

- [GO-01] DRY 違反 census（2026-06-19）が確認した 4 クラスタの既存重複を、ゲートに委ねることなく意図的な remediation 作業として一掃する。各クラスタの正典を確定し、コピーを正典への委譲に置き換えることで、変更増幅リスクと層をまたぐ乖離バグリスクを解消する。いずれの変更も挙動保存（behavior-preserving）であり、公開 API の振る舞いは変更しない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5]
- [GO-02] DRY ゲートの構造的死角（既存重複・cross-layer knowledge-dup・data-dup）を人手で補完し、コードベース全体の DRY 衛生を底上げする。remediation 完了後の DRY 違反 census で密度が低下していることを確認する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5]

## Scope

### In Scope
- [IN-01] 【D1 クラスタ】track-ID / slug 検証ロジックの一本化。`validate_track_id` / slug 検証ロジックを domain の正典（`libs/domain/src/ids.rs` の `is_valid_track_id` / `TrackId::try_new`）に集約し、usecase 3 モジュール（`catalogue_impl_signals` / `type_signals` / `baseline_capture`）と CLI 2 箇所（`apps/cli-composition/src/verify.rs` の `validate_track_id_str` / `apps/cli/src/commands/track/validate.rs`）の独立実装を削除して `TrackId::try_new` への委譲に置き換える [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D1] [tasks: T001, T002]
- [IN-02] 【D2 クラスタ】空/空白禁止不変条件の `NonEmptyString` への集約。domain の 8 箇所以上（`ids.rs` / `plan.rs` / `spec.rs` / `impl_plan.rs` / `review_v2/types.rs` 等）でインライン再実装されている「フィールドが空・空白のみであってはならない」不変条件を削除し、各箇所を `NonEmptyString::try_new` への委譲に置き換える [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D2] [tasks: T003, T004, T005]
- [IN-03] 【D3 クラスタ】`review_v2` と `dry_check` の間で重複する (1) Codex subprocess 管理（`spawn_codex` / `drain_pipe` / `tee_stderr_to_file` / ランタイムパスビルダ）と (4) SHA-256→lowercase-hex エンコードを共通モジュールへ抽出する。抽出先は `infrastructure` クレート内の `pub(crate)` 共通モジュールとし、hexagonal の層配置（domain / usecase / infrastructure）を侵さない（CN-02）。(2) 排他ロック取得パターン（`FsDryCheckStore::acquire_write_lock` / `FsReviewStore` のロック）と (3) 4-source git-diff union（`GitDiffGetter` / `GitDryCheckDiffGetter`）は、異なる domain port を異なるエラー型・出力型で実装するポート固有の並行構造であり、共通抽出の対象外として現状維持とする [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D3] [tasks: T006, T007]
- [IN-04] 【D4 クラスタ】test ヘルパ・定数のテスト・コンパイル境界ごとの単一定義への集約。`CwdGuard` / `init_git_repo`（6 箇所）/ stub bindings（usecase test 3 モジュール）などの test-only ヘルパを、テスト・コンパイル境界（src ユニットテスト向けの `#[cfg(test)]` モジュールと integration-test crate 向けの `tests/common` モジュール）ごとにそれぞれ一つの共通 test-support モジュールへ集約し、各境界内での冗長コピーを削除する。Rust のテスト・コンパイルモデル上、`src` 内の `#[cfg(test)] pub(crate)` 定義は integration-test crate からは不可視であり、`tests/common` は src ユニットテストからは不可視であるため、境界をまたいだ文字どおりの単一定義は dev-visible API を追加せずには不可能である。stub bindings（usecase-only）はすでに単一境界内にのみ存在するため、その境界内での単一定義に集約する。定数は `POLL_INTERVAL`（5 箇所）と `"tmp/reviewer-runtime"`（4 つの const 定義 + inline literal）を対象にし、それぞれ単一の `const` 定義に統合する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D4] [tasks: T008, T009, T010, T011, T012]
- [IN-05] 各クラスタについて「正典を決める → コピーを正典へ委譲 → `cargo make ci` で挙動不変を確認 → 小さく分割してコミット」の手順を踏む [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]

### Out of Scope
- [OS-01] DRY ゲート本体の変更・拡張。本 track は remediation のみであり、ゲートの full-corpus 化や cross-layer 検出拡張は別 ADR の関心事とする [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T013]
- [OS-02] 公開 API の振る舞い変更。本 track の変更はすべて挙動保存（behavior-preserving）であり、外部から観測可能な API の動作を変えてはならない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]
- [OS-03] 全クラスタを 1 コミットで一括大規模リファクタする方法（Rejected Alternative C）。クラスタ別・小コミットに分割して進める [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]
- [OS-04] clippy 等の lint 強化だけによる対応（Rejected Alternative D）。lint は near-clone やセマンティックな cross-layer knowledge-dup を捕捉できず、「どのコピーを正典とするか」の設計判断を代替しない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]
- [OS-05] census で確認された 4 クラスタ以外の DRY 違反の修正（本 track は影響度の高い 4 クラスタを対象とする） [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T013]

## Constraints
- [CN-01] 挙動保存制約: すべての変更後に `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass すること。既存テストが pass し続けることが挙動不変の判定基準である [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]
- [CN-02] hexagonal 層境界制約（D3 共通化）: D3 の共通モジュール抽出は hexagonal の層配置（domain / usecase / infrastructure）を侵さない位置に行う。usecase 層に `std::fs` / `std::process` / `std::env` 等の I/O が混入してはならない。抽出先が既存層境界を越える場合は本 spec の open question として報告し、ADR を更新する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D3] [conv: knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules] [tasks: T006]
- [CN-03] 最小化制約: 過剰な共通化は避ける。共通化が層間・テスト間の結合をわずかに増やすことは許容するが、hexagonal 境界を実質的に侵し始める規模になってはならない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D3] [tasks: T006, T008, T009, T010]
- [CN-04] 小コミット制約: 4 クラスタは別タスクとして分割し、1 コミットあたりの diff を小さく保つ（guardrails の small-task-commit 方針に従い 500 行未満を目安とする）。レビューコストは diff サイズに対し約 O(N^2) で増大するため、同一コミットに複数クラスタを混在させてはならない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]
- [CN-05] D1 正典制約: track-ID / slug 検証の正典は domain の `TrackId::try_new`（`libs/domain/src/ids.rs` の `is_valid_track_id`）とする。usecase や CLI 層は独立した文字集合チェックを再実装してはならず、domain 経由の委譲に限定する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D1] [tasks: T001, T002]
- [CN-06] D2 正典制約: 空/空白禁止の不変条件の正典は `NonEmptyString`（domain 既存型）とする。domain の各コンストラクタは `NonEmptyString::try_new` を呼び出してバリデーションを行い、インラインの `is_empty()` / `trim().is_empty()` チェックを独立実装してはならない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D2] [conv: knowledge/conventions/prefer-type-safe-abstractions.md#Newtype パターン：プリミティブ値の制約] [tasks: T003, T004, T005]

## Acceptance Criteria
- [ ] [AC-01] 【D1 完了基準】`validate_track_id` / slug 検証のコピーが usecase（`catalogue_impl_signals` / `type_signals` / `baseline_capture`）と CLI（`apps/cli-composition/src/verify.rs` / `apps/cli/src/commands/track/validate.rs`）から削除され、それぞれ `TrackId::try_new`（domain）への委譲に置き換わっている。`libs/domain/src/ids.rs` の `is_valid_track_id` / `TrackId::try_new` が唯一の独立実装として残る。`cargo make ci` が pass する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D1] [tasks: T001, T002]
- [ ] [AC-02] 【D2 完了基準】domain の `ids.rs` / `plan.rs` / `spec.rs` / `impl_plan.rs` / `review_v2/types.rs` 等での空/空白チェックのインライン再実装（`is_empty()` / `trim().is_empty()` による独自ガード）が `NonEmptyString::try_new` への委譲に置き換わっている。8 箇所以上の変換が完了し、`NonEmptyString` が空/空白禁止の単一の実装となる。`cargo make ci` が pass する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D2] [tasks: T003, T004, T005]
- [ ] [AC-03] 【D3 完了基準】(a) Codex subprocess ヘルパ（`spawn_codex` / `drain_pipe` / `tee_stderr_to_file` / ランタイムパスビルダ）が `infrastructure` クレート内の `pub(crate)` 共通インフラモジュールへ抽出され、`codex_reviewer.rs` と `codex_dry_checker.rs` の両方がそのモジュールを参照している。(b) インラインの SHA-256→hex の `format!` 呼び出しが `infrastructure::dry_check::corpus::sha256_hex` への委譲に置き換わっている。共通モジュールの配置は hexagonal 層境界を侵していない（`cargo make check-layers` が pass する）。`cargo make ci` が pass する。【現状維持】排他ロック取得パターン（(2) `FsDryCheckStore::acquire_write_lock` / `FsReviewStore` のロック）と 4-source git-diff union（(3) `GitDiffGetter` / `GitDryCheckDiffGetter`）はポート固有の並行構造として AC-03 の対象外であり、共通化しない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D3] [tasks: T006, T007]
- [ ] [AC-04] 【D4 完了基準 — test ヘルパ】`CwdGuard`（または `CurrentDirGuard`）が、テスト・コンパイル境界ごとに単一の共通 test-support 定義に集約され（src ユニットテスト向けには `#[cfg(test)]` モジュール内に一つ、integration-test crate が使用する場合は `tests/common` モジュール内に一つ）、各境界内での冗長な独立実装が削除されている。`init_git_repo`（`init_git_repo_on_track_branch` を含む）も同様に、テスト・コンパイル境界ごとに単一定義となるよう集約され（`apps/cli` のように src ユニットテストと integration-test の両方で使用するケースでは、Rust のテスト・コンパイルモデル上 dev-visible API を追加せずに境界をまたいだ単一定義にすることは不可能なため、それぞれの境界に一つずつ配置する）、各境界内での冗長コピーが削除されている。usecase test 3 モジュールの `stub_binding` / `StubLayerBindings` が usecase 内 test-support モジュールのテスト・コンパイル境界ごとの単一定義に集約されている。`cargo make ci` が pass する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D4] [tasks: T008, T009, T010]
- [ ] [AC-05] 【D4 完了基準 — 定数】`POLL_INTERVAL`（`Duration::from_millis(50)`）が単一の `const` 定義（例: infrastructure の共通定数モジュール）に集約され、5 箇所の独立定義が削除されている。`"tmp/reviewer-runtime"` が単一の `const` 定義に集約され、4 つの const 定義 + inline literal が削除されている。`cargo make ci` が pass する [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D4] [tasks: T011, T012]
- [ ] [AC-06] 各クラスタのコミットが独立した小コミットとして分割されている（同一コミットに複数クラスタの変更が混在していない）。各コミット後に `cargo make ci` が pass することが確認されている [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012]
- [ ] [AC-07] remediation 完了後に DRY 違反 census を再実行し、4 クラスタに対応する違反群（D1: cross-layer knowledge-dup の validate_track_id 系、D2: domain の NonEmptyString 系インライン再実装、D3: subprocess helper / SHA-256 hex エンコードの重複、D4: test ヘルパ・`POLL_INTERVAL` / `"tmp/reviewer-runtime"` 定数の重複）が census から消滅していることを確認する。D3 の排他ロック取得パターンと 4-source git-diff union は port 固有の parallel structure として扱い、未解消違反には数えない [adr: knowledge/adr/2026-06-19-0924-existing-dry-violation-cleanup.md#D5] [tasks: T013]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Newtype パターン：プリミティブ値の制約
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/coding-principles.md#Module Size

## Signal Summary

### Stage 1: Spec Signals
🔵 25  🟡 0  🔴 0

