<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# Support workflow composition roots pure-DI migration

## Goal

- [GO-01] RefVerify、SemanticDup、Verify の Support workflows を、この track 単独でレビュー可能かつ CI-green のまま統合できる純 DI migration delivery unit として完了する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7]
- [GO-02] 各 Support workflow の composition-root execution path を、外部観測可能な CLI 契約を保った純 DI 境界へ収束させる。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1]

## Scope

### In Scope
- [IN-01] RefVerify、SemanticDup、Verify の三つの Support workflow command 文脈を、この track の純 DI 化対象とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3] [tasks: T001, T002, T003, T004]
- [IN-02] 各所有 execution path から、composition root 上の command 実行ロジック、CommandOutcome の生成、filesystem・process・network・terminal への直接 I/O、usecase から composition root への逆委譲、互換 shim と中間 *ServiceImpl への runtime 参照・呼出しを除去し、driver → usecase interactor → port の一方向・単一路へ収束させる。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1] [tasks: T001, T002, T003, T004]
- [IN-03] Support workflow の composition root 公開面を、fully wired driver を引き渡す純 DI boundary として維持し、実行メソッドまたは禁止された公開型露出を既存 catalogue lint により検出可能な状態にする。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D3, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D5] [tasks: T001, T002, T004]

### Out of Scope
- [OUT-01] Leaf commands、Collaboration workflows、Core lifecycle の command 文脈、および PR / Signal / Review command contexts の純 DI 化は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3]
- [OUT-02] now-unreferenced となる互換定義・export の物理削除、リポジトリ全体の CompositionRootPureDi 適合、関連 lint・文書・export の最終同期、およびイニシアチブ全体の完了判定は Final convergence track の対象とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5]
- [OUT-03] Interactor と Application Service の別 crate への分離、または crate topology の変更は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9]

## Constraints
- [CN-01] 構造変更前後で、CLI 引数、stdout、stderr、exit code、および永続化結果を変更しない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T001, T002, T003, T004]
- [CN-02] この Support workflows slice は単独でレビュー可能かつ CI-green のまま develop へ統合可能であり、未統合の他 migration track の実装を暗黙に前提にしない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T001, T002, T003, T004]
- [CN-03] crate topology を維持し、Interactor と Application Service は libs/usecase 内の適切な command-context module に置いて、同一 application boundary 内で役割を分離する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T002, T003]
- [CN-04] SemanticDup の外部契約維持は semantic-dup feature を有効にした統合テストで実証する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T003, T004]

## Acceptance Criteria
- [ ] [AC-01] RefVerify の live CLI path は fully wired driver を composition root から取得して実行し、legacy convenience execution methods を経由せず、composition root に command execution、CommandOutcome 生成、直接 I/O、または逆委譲を残さない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001]
- [ ] [AC-02] Verify catalogue-spec-reference execution path は、track-id validation、infrastructure 呼出し、CommandOutcome assembly を composition root で実行せず、単一の driver → usecase interactor → port route に従う。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T002]
- [ ] [AC-03] SemanticDup は semantic-dup feature 有効時に、composition 内の SemanticDupDriverPort implementation または composition execution methods を経由せず、single one-way execution path で index build、check、find similar、index measure quality を実行する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T003, T004]
- [ ] [AC-05] 三つの Support workflow の各 execution path は、互換 shim または中間 *ServiceImpl への runtime 参照・呼出し・逆委譲を残さず、driver → usecase interactor → port の一方向・単一路に従う。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T002, T003, T004]
- [ ] [AC-06] 統合テストは、RefVerify、SemanticDup、Verify の各所有 command 文脈について、同じ入力が構造変更前と同じ CLI 引数処理、stdout、stderr、exit code、および永続化結果を返すことを確認する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T001, T002, T004]
- [ ] [AC-07] この slice は他の未統合 migration track を必要とせず、CI-green を維持して統合できる。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T001, T002, T003, T004]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/testing.md#Rules
- knowledge/conventions/security.md#Sensitive Directories
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

