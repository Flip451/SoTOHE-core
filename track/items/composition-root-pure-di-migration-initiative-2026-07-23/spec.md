<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 15, yellow: 0, red: 0 }
---

# Composition root pure DI migration initiative — Leaf commands slice

## Goal

- [GO-01] Leaf commands（ADR baseline、Catalog、Template、TaskContract）を担当するこの slice を、独立してレビュー可能で CI-green のまま統合できる delivery unit として完了する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3]
- [GO-02] Leaf commands の composition-root 実行経路を純 DI 境界へ収束させ、CLI 引数、stdout、stderr、exit code、および永続化結果の外部契約を維持する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5]

## Scope

### In Scope
- [IN-01] Leaf commands の ADR baseline、Catalog、Template、TaskContract の各 command 文脈を、この track が所有する純 DI 化の対象とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3] [tasks: T001, T002, T003, T004]
- [IN-02] 各所有 command 文脈の execution path から、composition root にある command 実行ロジック、CommandOutcome の生成、filesystem・process・network・terminal への直接 I/O、usecase から composition root への逆委譲、互換 shim と中間 *ServiceImpl への runtime 参照・呼出しを除去し、driver → usecase interactor → port の一方向・単一路へ収束させる。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T002, T003, T004]

### Out of Scope
- [OUT-01] Support workflows、Collaboration workflows、Core lifecycle の command 文脈の純 DI 化は、この Leaf commands track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3] [tasks: T001, T002, T003, T004]
- [OUT-02] リポジトリ全体の CompositionRootPureDi 適合、全互換 shim と逆委譲仲介 *ServiceImpl の物理削除、関連 export・lint・文書の最終同期、およびイニシアチブ全体の完了宣言は Final convergence track の対象とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D6] [tasks: T001, T002, T003, T004]
- [OUT-03] Interactor と Application Service を別 crate に分離する変更、または crate topology の変更は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T001, T002, T003, T004]

## Constraints
- [CN-01] Leaf commands の構造変更前後で、CLI 引数、stdout、stderr、exit code、および永続化結果を変更しない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T001, T002, T003, T004]
- [CN-02] この Leaf commands slice は単独でレビュー可能かつ CI-green のまま統合可能であり、未統合の他 track の変更を暗黙に前提にしない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T001, T002, T003, T004]
- [CN-03] crate topology を維持し、Interactor と Application Service は libs/usecase 内の適切な command 文脈 module に置いて同一 application boundary 内で役割を分離する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T001, T002, T003, T004]

## Acceptance Criteria
- [ ] [AC-01] ADR baseline、Catalog、Template、TaskContract の各所有 execution path は、composition root に command 実行ロジック、CommandOutcome の生成、直接 I/O、または usecase から composition root への逆委譲を残さない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T002, T003, T004]
- [ ] [AC-02] 各所有 execution path は、互換 shim または中間 *ServiceImpl への runtime 参照・呼出し・逆委譲を残さず、driver → usecase interactor → port の一方向・単一路に従う。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T002, T003, T004]
- [ ] [AC-03] 統合テストは、各所有 Leaf command 文脈について同じ入力が構造変更前と同じ CLI 引数処理、stdout、stderr、exit code、および永続化結果を返すことを確認する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T001, T002, T003, T004]
- [ ] [AC-04] この slice は単独で CI-green を維持して統合でき、他の未統合 migration track の実装を必要としない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T001, T002, T003, T004]
- [ ] [AC-05] Leaf commands の純 DI 化は crate topology を変更せず、Interactor と Application Service の role separation を libs/usecase 内に維持する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T001, T002, T003, T004]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Rules
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/type-designer-kind-selection.md#R1. Layer-Kind Compatibility (層 × kind 互換マトリクス)

## Signal Summary

### Stage 1: Spec Signals
🔵 15  🟡 0  🔴 0

