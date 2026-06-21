<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# DRY ゲートを利用者設定で切り替え可能にし、既定を無効（opt-in）とする

## Goal

- [GO-01] DRY ゲート（`sotp dry check-approved` による commit ブロックと DFP 修正ループ）を無条件必須から、`.harness/config/dry-check.json` の `enabled` boolean キーで有効/無効を切り替えられる設定に変更する。既定は `enabled: false`（opt-in 運用）とし、ゲートを使いたい利用者が明示的に有効化する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D1]
- [GO-02] `enabled: false` のとき DRY ゲートの 2 つの評価点（commit ゲートの `sotp dry check-approved` と DFP 起動判定の `fixpoint_resolve`）が「通過 / DFP 不要」を返すことで、DRY 検出・DFP 修正ループ・commit ブロックがいずれも実行されない状態を実現する。上位の Makefile 配線（`track-commit-message` 等）は変更しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2]
- [GO-03] `enabled: true` のとき DRY ゲートは従来どおり blocking gate として機能し、genuine な DRY 違反への人間による場当たり的な許容の抜け道を設けない。`2026-06-02-0716-dry-checker.md` D7 の「全 above-threshold ペアの verdict が確定するまで進めない blocking gate」「genuine な違反への人間による許容の抜け道は無い」という性質を有効時に維持する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3]

## Scope

### In Scope
- [IN-01] `.harness/config/dry-check.json` のスキーマに boolean キー `enabled` を追加する。`schema_version` を 3 から 4 に上げる。schema_version 4 のファイルで `enabled` キーが省略された場合は `false`（gate off）として扱う（opt-in の既定 OFF を schema レベルで表現する）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T002]
- [IN-02] commit ゲートの `sotp dry check-approved`（`DryCheckApprovalService::check_approved`）が `dry-check.json` の `enabled` を読み、`false` のとき `DryCheckApprovalVerdict::Approved` を即座に返す（staleness チェック・all-resolved チェックを実行しない）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T005, T006]
- [IN-03] DFP 起動判定の `fixpoint_resolve`（`FixpointResolveInteractor`）が `dry-check.json` の `enabled` を読み、`false` のとき dry gate 評価を「通過（RunDfp を返さない）」として扱う。`enabled: false` の状態での fixpoint 解決は dry gate を常時 Approved と見なして review gate・ref-verify gate の評価に進む。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T007, T008]
- [IN-04] 設定の適用範囲はリポジトリ全体の単一設定（グローバル）とする。トラック単位・違反単位の上書きは設けない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T002]
- [IN-05] インフラ層の `dry-check.json` 読み込み DTO（`DryCheckConfigDto`）に `enabled: bool` フィールドを追加し、schema_version 4 の codec として実装する。usecase 層の `DryCheckConfig` に `enabled: bool` フィールドを追加し、composition root が DTO から `DryCheckConfig` を生成する際に `enabled` を伝播する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T003, T004, T006, T008]
- [IN-06] 実際の `.harness/config/dry-check.json` を schema_version 4 に更新し、`enabled: false` を既定値として追記する。既存フィールド（`threshold`、`max_parallelism`、`fast_reasoning_effort`、`final_reasoning_effort`、`known_bad_injection_rate_percent`、`known_bad_detection_threshold_percent`）はそのまま維持する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T002]
- [IN-07] `2026-06-02-0716-dry-checker.md` の Follow-up セクションに、本 ADR が D7 の「無条件必須」側面を部分 supersede したことの相互参照を追記する（ADR 間整合の追従）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T012]

### Out of Scope
- [OS-01] advisory（報告のみ・非ブロック）モードなどの 3 値設定（blocking / advisory / disabled）は設けない。有効/無効の 2 状態 boolean のみとする。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D1]
- [OS-02] トラック単位・違反単位の粒度での有効/無効設定は設けない。グローバルな単一設定のみ。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2]
- [OS-03] `sotp dry check-approved` / `sotp dry write` への CLI bypass フラグ（`--skip` / `--force` 等 per-invocation の抜け道）は設けない。恒久的なプロジェクト方針は設定ファイルで表現する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D1]
- [OS-04] `enabled: true` のときの DRY ゲートの内部挙動変更（blocking 性・DFP ループの構造・fixpoint の評価順序・verdict 記録方式等）は対象外。有効時の挙動は `2026-06-02-0716-dry-checker.md` の決定内容から変わらない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3]
- [OS-05] Makefile 配線（`track-commit-message` 等の上位 task）の変更は行わない。`dry check-approved` / `fixpoint_resolve` 自体が `enabled` を判断するため、上位の呼び出し側は変更不要。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2]

## Constraints
- [CN-01] schema_version 4 の `dry-check.json` で `enabled` キーが省略された場合は `false`（gate off）として扱う。opt-in 運用の既定 OFF を schema の既定値として表現するためであり、`enabled: true` への暗黙フォールバックは設けない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T002]
- [CN-02] `enabled: false` のとき `DryCheckApprovalService::check_approved` は coverage manifest の読み込み・staleness チェック・all-resolved チェックを実行せず、即座に `Approved` を返す。embedding・類似検索・DFP 修正ループは一切実行されない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T005, T006]
- [CN-03] `enabled: false` のとき `FixpointResolveInteractor` は dry gate の評価を「常時 Approved」として扱い、`FixpointStep::RunDfp` を返さない。review gate・ref-verify gate の評価は通常どおり行う。dry gate の評価スキップは fixpoint_resolve の中で行い、上位の Makefile 配線は変更しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T007, T008]
- [CN-04] `enabled: true` のとき、`DryCheckApprovalService::check_approved` と `FixpointResolveInteractor` は従来と同一の挙動を維持する。genuine な `violation` への人間による場当たり的な許容の抜け道は設けず、`2026-06-02-0716-dry-checker.md` D7 の blocking gate の性質を完全に維持する。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T005, T006, T007, T008, T010, T011]
- [CN-05] `schema_version` が 4 以外の `dry-check.json` を読み込んだ場合は旧スキーマとして hard error とする。`enabled` フィールドを持たない旧 schema_version 3 ファイルは無効であり、明示的な schema_version 4 への更新を利用者に求める。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T009]
- [CN-06] `enabled` の読み取りは `DryCheckConfig`（usecase 層）を通じて行う。インフラ層の `DryCheckConfigDto` が `enabled` を保持し、composition root が DTO から `DryCheckConfig` を構築する際に伝播する。`DryCheckApprovalInteractor` および `FixpointResolveInteractor` は `enabled` を `DryCheckConfig` 経由で受け取り、infrastructure の設定ファイルを直接参照しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T003, T004, T005, T006, T007, T008]

## Acceptance Criteria
- [ ] [AC-01] `.harness/config/dry-check.json` が `"schema_version": 4` と `"enabled": false` を含む形で更新されており、既存の `threshold`・`max_parallelism`・reasoning effort・probe 設定フィールドがそのまま維持されている。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T002]
- [ ] [AC-02] `enabled: false` の設定で `sotp track fixpoint-resolve` を呼び出したとき、dry gate が `Approved` として扱われ `RunDfp` が返らない（review gate・ref-verify gate の状態に従って `RunRfp` / `RunRefVerify` / `Commit` のいずれかが返る）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T007, T008, T011]
- [ ] [AC-03] `enabled: false` の設定で `sotp dry check-approved` を呼び出したとき、coverage manifest の読み込み・staleness チェック・all-resolved チェックを実行せずに `Approved` を返す（DFP ループは起動されない）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T005, T006, T010]
- [ ] [AC-04] `enabled: true` の設定で `sotp dry check-approved` を呼び出したとき、従来と同一の挙動（coverage manifest 読み込み・staleness チェック・all-resolved チェック）が維持される。genuine な violation が存在する場合は `Blocked` を返してコミットをブロックする。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T005, T006, T010]
- [ ] [AC-05] schema_version 4 の `dry-check.json` で `enabled` キーが省略されているとき、`false`（gate off）として扱われ、hard error にならない。一方、`schema_version` が 4 以外（例: 旧 schema_version 3）のファイルを読み込んだときは hard error が発生する（CN-05 の制約に対応）。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T001, T009]
- [ ] [AC-06] usecase 層の `DryCheckConfig` に `enabled: bool` フィールドが追加されており、`DryCheckApprovalInteractor` が `DryCheckConfig` の `enabled` を参照して早期 return するパスが実装されている。`FixpointResolveInteractor` も同様に `DryCheckConfig` の `enabled` を参照して dry gate スキップを実装している。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T003, T005, T007]
- [ ] [AC-07] `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する。`enabled: false` / `enabled: true` の各ケースおよびスキーマ不正ケースを網羅するユニットテストが追加されており、既存テストへのリグレッションが存在しない。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D2] [tasks: T009, T010, T011, T012]
- [ ] [AC-08] `2026-06-02-0716-dry-checker.md` の Follow-up セクションに、本 ADR（`2026-06-19-2335-dry-gate-configurable-default-off.md`）が D7 の「無条件必須」側面を部分 supersede したことを示す相互参照が追記されている。 [adr: knowledge/adr/2026-06-19-2335-dry-gate-configurable-default-off.md#D3] [tasks: T012]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

