<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.1"
signals: { blue: 38, yellow: 0, red: 0 }
---

# DRY ゲートを利用者設定で切り替え可能にし、既定を無効（opt-in）とする

## Goal

- [GO-01] DRY ゲート（`sotp dry check-approved` による commit ブロックと DFP 修正ループ）を無条件必須から、`.harness/config/dry-check.json` の `enabled` boolean キーで有効/無効を切り替えられる設定に変更する。既定は `enabled: false`（opt-in 運用）とし、ゲートを使いたい利用者が明示的に有効化する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D1]
- [GO-02] `enabled: false` のとき DRY ゲートの 2 つの評価点（commit ゲートの `sotp dry check-approved` と DFP 起動判定の `fixpoint_resolve`）が「通過 / DFP 不要」を返すことで、DRY 検出・DFP 修正ループ・commit ブロックがいずれも実行されない状態を実現する。上位の Makefile 配線（`track-commit-message` 等）は変更しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2]
- [GO-03] `enabled: true` のとき DRY ゲートは従来どおり blocking gate として機能し、genuine な DRY 違反への人間による場当たり的な許容の抜け道を設けない。`2026-06-02-0716-dry-checker.md` D7 の「全 above-threshold ペアの verdict が確定するまで進めない blocking gate」「genuine な違反への人間による許容の抜け道は無い」という性質を有効時に維持する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3]
- [GO-04] dry-checker の reasoning effort 設定（`fast_reasoning_effort` / `final_reasoning_effort`）を `.harness/config/dry-check.json` から `.harness/config/agent-profiles.json` の `capabilities.dry-checker` に移す。`dry-check.json` の schema_version は v4 のまま据え置き、D2 の `enabled` 追加と D4 の reasoning_effort 削除を同一スキーマ（v4）内に統合する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4]

## Scope

### In Scope
- [IN-01] `.harness/config/dry-check.json` のスキーマに boolean キー `enabled` を追加する。`schema_version` は 3 から 4 に上げる（D2 決定）。schema_version 4 のファイルで `enabled` キーが省略された場合は `false`（gate off）として扱う（opt-in の既定 OFF を schema レベルで表現する）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T002]
- [IN-02] commit ゲートの `sotp dry check-approved`（`DryCheckApprovalService::check_approved`）が `dry-check.json` の `enabled` を読み、`false` のとき `DryCheckApprovalVerdict::Approved` を即座に返す（staleness チェック・all-resolved チェックを実行しない）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T005, T006]
- [IN-03] DFP 起動判定の `fixpoint_resolve`（`FixpointResolveInteractor`）が `dry-check.json` の `enabled` を読み、`false` のとき dry gate 評価を「通過（RunDfp を返さない）」として扱う。`enabled: false` の状態での fixpoint 解決は dry gate を常時 Approved と見なして review gate・ref-verify gate の評価に進む。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T007, T008]
- [IN-04] 設定の適用範囲はリポジトリ全体の単一設定（グローバル）とする。トラック単位・違反単位の上書きは設けない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T002]
- [IN-05] インフラ層の `dry-check.json` 読み込み DTO（`DryCheckConfigDto`）に `enabled: bool` フィールドを追加し、schema_version 4 の codec として実装する。usecase 層の `DryCheckConfig` に `enabled: bool` フィールドを追加し、composition root が DTO から `DryCheckConfig` を生成する際に `enabled` を伝播する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T003, T004, T006, T008]
- [IN-06] 実際の `.harness/config/dry-check.json` を schema_version 4 の最終形に更新し、`enabled: false` を含む。`fast_reasoning_effort` と `final_reasoning_effort` の 2 フィールドは D4 により削除する。D2 の `enabled` 追加と D4 の reasoning_effort 削除は 1 回の schema 更新として v4 内に統合する（中間 schema を作らない）。残存フィールドは `enabled`・`threshold`・`max_parallelism`・`known_bad_injection_rate_percent`・`known_bad_detection_threshold_percent` となる。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T002, T014]
- [IN-07] `2026-06-02-0716-dry-checker.md` の Follow-up セクションに、本 ADR が D7 の「無条件必須」側面を部分 supersede したことの相互参照を追記する（ADR 間整合の追従）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T012]
- [IN-08] `.harness/config/agent-profiles.json` の `capabilities.dry-checker` オブジェクトに `fast_reasoning_effort` と `final_reasoning_effort` の 2 フィールドを追加する。キー名は `dry-check.json` での旧名称をそのまま踏襲する。これら 2 フィールドは dry-checker capability が使う推論強度の設定であり、`model` / `fast_model` と同じカテゴリに属する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T013]
- [IN-09] インフラ層の `DryCheckConfigDto` から `fast_reasoning_effort` / `final_reasoning_effort` フィールドを削除し、schema_version 4 の codec を再実装する。`DryCheckConfig`（インフラ側）の `fast_reasoning_effort()` / `final_reasoning_effort()` accessors を削除する。composition root（`dry.rs`）の reasoning effort 取得先を `DryCheckConfig` から `AgentProfiles` に切り替える。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T013, T014]
- [IN-10] `DryCheckConfig::fingerprint` の canonical encoding から `fast_reasoning_effort` / `final_reasoning_effort` を除外する。これら 2 フィールドは dry-check.json のスキーマを離れるため、coverage manifest の staleness 判定に含めるべきでない。fingerprint が変わるため、既存の coverage manifest は stale 扱いとなり `dry write` の再実行が必要になる。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T014]

### Out of Scope
- [OS-01] advisory（報告のみ・非ブロック）モードなどの 3 値設定（blocking / advisory / disabled）は設けない。有効/無効の 2 状態 boolean のみとする。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D1]
- [OS-02] トラック単位・違反単位の粒度での有効/無効設定は設けない。グローバルな単一設定のみ。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2]
- [OS-03] `sotp dry check-approved` / `sotp dry write` への CLI bypass フラグ（`--skip` / `--force` 等 per-invocation の抜け道）は設けない。恒久的なプロジェクト方針は設定ファイルで表現する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D1]
- [OS-04] `enabled: true` のときの DRY ゲートの内部挙動変更（blocking 性・DFP ループの構造・fixpoint の評価順序・verdict 記録方式等）は対象外。有効時の挙動は `2026-06-02-0716-dry-checker.md` の決定内容から変わらない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3]
- [OS-05] Makefile 配線（`track-commit-message` 等の上位 task）の変更は行わない。`dry check-approved` / `fixpoint_resolve` 自体が `enabled` を判断するため、上位の呼び出し側は変更不要。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2]
- [OS-06] `agent-profiles.json` 側での `fast_reasoning_effort` / `final_reasoning_effort` の値バリデーション（allowed values チェック）は対象外。`agent-profiles.json` の loader は文字列フィールドを untyped のまま保持し、値の検証は composition root（caller 側）が行う。allowed values の強制は以前と同様に composition root が担う。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T013, T014]

## Constraints
- [CN-01] schema_version 4 の `dry-check.json` で `enabled` キーが省略された場合は `false`（gate off）として扱う。opt-in 運用の既定 OFF を schema の既定値として表現するためであり、`enabled: true` への暗黙フォールバックは設けない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T002]
- [CN-02] `enabled: false` のとき `DryCheckApprovalService::check_approved` は coverage manifest の読み込み・staleness チェック・all-resolved チェックを実行せず、即座に `Approved` を返す。embedding・類似検索・DFP 修正ループは一切実行されない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T005, T006]
- [CN-03] `enabled: false` のとき `FixpointResolveInteractor` は dry gate の評価を「常時 Approved」として扱い、`FixpointStep::RunDfp` を返さない。review gate・ref-verify gate の評価は通常どおり行う。dry gate の評価スキップは fixpoint_resolve の中で行い、上位の Makefile 配線は変更しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T007, T008]
- [CN-04] `enabled: true` のとき、`DryCheckApprovalService::check_approved` と `FixpointResolveInteractor` は従来と同一の挙動を維持する。genuine な `violation` への人間による場当たり的な許容の抜け道は設けず、`2026-06-02-0716-dry-checker.md` D7 の blocking gate の性質を完全に維持する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T005, T006, T007, T008, T010, T011]
- [CN-05] `schema_version` が 4 以外の `dry-check.json` を読み込んだ場合は旧スキーマとして hard error とする。D2 と D4 を統合した schema_version 4 が唯一サポートされるバージョンであり、schema_version 4 以外のファイルは無効として明示的な migration を利用者に求める。なお reasoning_effort フィールドが残ったままの古い v4 ファイルを投入すると、loader の `deny_unknown_fields` により Parse エラーが発生する（migration 要求の手段）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T001, T009, T014, T015]
- [CN-06] `enabled` の読み取りは `DryCheckConfig`（usecase 層）を通じて行う。インフラ層の `DryCheckConfigDto` が `enabled` を保持し、composition root が DTO から `DryCheckConfig` を構築する際に伝播する。`DryCheckApprovalInteractor` および `FixpointResolveInteractor` は `enabled` を `DryCheckConfig` 経由で受け取り、infrastructure の設定ファイルを直接参照しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T003, T004, T005, T006, T007, T008]
- [CN-07] reasoning effort の設定（`fast_reasoning_effort` / `final_reasoning_effort`）は `agent-profiles.json` の `capabilities.dry-checker` から composition root が直接読み取り、`CodexDryChecker` に渡す。usecase 層・domain 層は reasoning effort を保持しない。`DryCheckConfig`（usecase 層）に reasoning effort フィールドは追加しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T013, T014]
- [CN-08] `DryCheckConfig::fingerprint` の canonical encoding から `fast_reasoning_effort` / `final_reasoning_effort` を除外する。fingerprint は dry-check.json が持つフィールドのみを対象とし、agent-profiles.json 由来の設定は含めない。reasoning effort の変更は coverage manifest の staleness 判定に影響しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T014, T015]

## Acceptance Criteria
- [ ] [AC-01] `.harness/config/dry-check.json` が `"schema_version": 4` と `"enabled": false` を含む形で更新されており、`fast_reasoning_effort` / `final_reasoning_effort` フィールドが削除されている。残存フィールドは `enabled`・`threshold`・`max_parallelism`・`known_bad_injection_rate_percent`・`known_bad_detection_threshold_percent` のみ。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T002, T014]
- [ ] [AC-02] `enabled: false` の設定で `sotp track fixpoint-resolve` を呼び出したとき、dry gate が `Approved` として扱われ `RunDfp` が返らない（review gate・ref-verify gate の状態に従って `RunRfp` / `RunRefVerify` / `Commit` のいずれかが返る）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T007, T008, T011]
- [ ] [AC-03] `enabled: false` の設定で `sotp dry check-approved` を呼び出したとき、coverage manifest の読み込み・staleness チェック・all-resolved チェックを実行せずに `Approved` を返す（DFP ループは起動されない）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T005, T006, T010]
- [ ] [AC-04] `enabled: true` の設定で `sotp dry check-approved` を呼び出したとき、従来と同一の挙動（coverage manifest 読み込み・staleness チェック・all-resolved チェック）が維持される。genuine な violation が存在する場合は `Blocked` を返してコミットをブロックする。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T005, T006, T010]
- [ ] [AC-05] schema_version 4 の `dry-check.json` で `enabled` キーが省略されているとき、`false`（gate off）として扱われ、hard error にならない。reasoning_effort フィールドが v4 ファイルに残っている場合は loader の `deny_unknown_fields` により Parse エラーが発生し、利用者に migration を促す。schema_version が 4 以外のファイルを読み込んだときは `UnsupportedSchemaVersion` エラーが発生する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T001, T009, T015]
- [ ] [AC-06] usecase 層の `DryCheckConfig` に `enabled: bool` フィールドが追加されており、`DryCheckApprovalInteractor` が `DryCheckConfig` の `enabled` を参照して早期 return するパスが実装されている。`FixpointResolveInteractor` も同様に `DryCheckConfig` の `enabled` を参照して dry gate スキップを実装している。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T003, T005, T007]
- [ ] [AC-07] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する。`enabled: false` / `enabled: true` の各ケースおよびスキーマ不正ケースを網羅するユニットテストが追加されており、既存テストへのリグレッションが存在しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T009, T010, T011, T012]
- [ ] [AC-08] `2026-06-02-0716-dry-checker.md` の Follow-up セクションに、本 ADR（`2026-06-19-2335-dry-gate-configurable-default-off.md`）が D7 の「無条件必須」側面を部分 supersede したことを示す相互参照が追記されている。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T012]
- [ ] [AC-09] `.harness/config/agent-profiles.json` の `capabilities.dry-checker` に `fast_reasoning_effort` と `final_reasoning_effort` が追加されており、`dry.rs` の composition root がこれらを `AgentProfiles` 経由で読み取って `CodexDryChecker` に渡している。`DryCheckConfig`（インフラ層）の `fast_reasoning_effort()` / `final_reasoning_effort()` accessors が削除されている。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T013, T014]
- [ ] [AC-10] `DryCheckConfig::fingerprint` の canonical encoding が `fast_reasoning_effort` / `final_reasoning_effort` を含まない形に更新されており、reasoning effort を変更しても既存の coverage manifest が stale 扱いにならない（fingerprint が変わらない）ことをテストで確認できる。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D4] [tasks: T014, T015]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 38  🟡 0  🔴 0

