<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 19, yellow: 0, red: 0 }
---

# PR / Signal Composition Root Pure DI Migration

## Goal

- [GO-01] PR と Signal の command 文脈を、composition root が配線済み driver を引き渡すだけの純 DI 実行経路へ移行する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5]
- [GO-02] PR / Signal migration slice を、未統合の migration track に依存せず CI green のまま統合可能にする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7]

## Scope

### In Scope
- [IN-01] PR command 文脈の composition root、CLI entrypoint、driver、usecase、および必要な port 配線を、composition root に実行責務を残さない一方向・単一路へ移行する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T003, T005, T007]
- [IN-02] Signal command 文脈の composition root、CLI entrypoint、driver、usecase、および必要な port 配線を、composition root に実行責務を残さない一方向・単一路へ移行する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T002, T004, T006, T007]

### Out of Scope
- [OUT-01] Review を含む PR / Signal 以外の command 文脈の純 DI 化は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3] [tasks: T005, T006]
- [OUT-02] 参照されなくなった compatibility shim、中間 ServiceImpl、export の物理削除と、リポジトリ全体の最終収束判定は Final convergence track の対象とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T006, T007]
- [OUT-03] Interactor と Application Service を別 crate に分離する変更、その他の crate topology 変更は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T001, T002]
- [OUT-04] 他 track が所有する command 文脈、共通 policy surface、または共有変更を暗黙に前提とする実装は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T007]

## Constraints
- [CN-01] PR と Signal の構造変更前後で、CLI 引数、stdout、stderr、exit code、および永続化結果を変更しない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T005, T006, T007]
- [CN-02] 各統合単位は CI green を保ち、未統合の migration track の実装を暗黙に前提としない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [CN-03] crate topology を維持し、Interactor と Application Service は libs/usecase 内の同一 application boundary で役割を分離する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T001, T002]
- [CN-04] PR と Signal の担当 execution path は driver から usecase interactor、port へ向かう一方向・単一路とし、新しい compatibility facade を完了状態として残さない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007]

## Acceptance Criteria
- [ ] [AC-01] PR の composition root は command 実行ロジック、CommandOutcome の生成、filesystem・process・network・terminal への直接 I/O、または usecase から composition への逆委譲を残さない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T005, T007]
- [ ] [AC-02] Signal の composition root は command 実行ロジック、CommandOutcome の生成、filesystem・process・network・terminal への直接 I/O、または usecase から composition への逆委譲を残さない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T006, T007]
- [ ] [AC-03] PR driver は closure による command 実行注入を typed driven port に置き換え、driver → usecase interactor → port の一方向・単一路で各 PR command を実行する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T003, T005]
- [ ] [AC-04] PR と Signal の担当 execution path は SignalServiceImpl および signal shim への runtime 参照・呼出し・逆委譲を残さない。物理削除は要求しない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T006, T007]
- [ ] [AC-05] 統合テストは、PR と Signal の各 command 文脈について、同じ入力が構造変更前と同じ CLI 引数処理、stdout、stderr、exit code、および永続化結果を返すことを確認する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T007]
- [ ] [AC-06] この PR / Signal slice は単独で CI green を維持して統合でき、他の未統合 migration track の実装を必要としない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T001, T002, T003, T004, T005, T006, T007]
- [ ] [AC-07] Signal driver は各 Signal command を driver → usecase interactor → port の一方向・単一路で実行し、composition root または互換 facade を経由して実行しない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T006, T007]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/testing.md#Rules
- knowledge/conventions/security.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/type-designer-kind-selection.md#R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)

## Signal Summary

### Stage 1: Spec Signals
🔵 19  🟡 0  🔴 0

