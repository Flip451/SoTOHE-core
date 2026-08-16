<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# Track lifecycle command composition roots pure-DI migration

## Goal

- [GO-01] Track context 全体を、外部観測可能な CLI 契約を保ったまま、composition root が配線だけを担う純 DI の実行経路へ収束させて完了する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1]

## Scope

### In Scope
- [IN-01] Track context の composition root に残る 34 個の public execution methods、3 個の解決 helper、およびそれらが依存する usecase input boundary、primary adapter、CLI command wiring を純 DI 化対象とする。対象には mod.rs、ops.rs、branch_strategy.rs、set_commit_hash.rs、tddd.rs、tddd_catalogue_lint.rs、resolution.rs の command contexts を含める。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5, knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md#D3] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T027, T032, T033, T028]
- [IN-02] 対象 execution path から composition root 上の command execution logic、CommandOutcome generation、直接 I/O、および usecase から composition root への逆委譲を除去し、driver → usecase interactor → port の一方向・単一路へ収束させる。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T027, T032, T033, T028]
- [IN-03] 対象 CLI subcommand ごとの execution boundary を分離し、primary adapter が command selection と presentation を担い、usecase boundary に未検証 primitive の位置引数または整形済み stdout / stderr 出力を露出させない。 [adr: knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md#D1, knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md#D2] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026]
- [IN-04] ADR baseline、review_v2、task_contract の各 context から旧 TrackCompositionRoot resolution helper を利用する call site を更新し、同等の解決機能を維持する。この更新は Track context の公開面を純 DI に収束させるための互換適応であり、これら対象外 context の composition root または usecase port を純 DI 化する作業を含まない。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4, knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md#D3] [tasks: T027, T030, T031]

### Out of Scope
- [OUT-02] semantic_dup command context と、Leaf commands、Support workflows、Collaboration workflows の composition root および usecase port の純 DI 化は本 track の対象外とする。ただし ADR baseline、review_v2、task_contract の旧 resolution helper 利用 call site の互換適応は IN-04 の範囲で行う。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D3] [tasks: T027]
- [OUT-03] 参照されなくなった compatibility definitions / exports の物理削除、全 composition root の最終適合、関連 lint・文書・export の最終同期、および initiative 全体の完了判定は Final convergence track の対象とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D6] [tasks: T028]
- [OUT-04] Interactor と Application Service の別 crate への分離、または crate topology の変更は本 track の対象外とする。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T028]

## Constraints
- [CN-01] 構造変更前後で、対象 Track command contexts と旧 resolution helper を利用する対象外 context の call site における CLI 引数、stdout、stderr、exit code、および永続化結果を変更しない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T027, T030, T031, T032, T028]
- [CN-02] この track は単独でレビュー可能かつ CI-green のまま統合可能であり、未統合の migration track の変更を暗黙に前提にしない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T027, T030, T031, T032, T033, T028, T029]
- [CN-03] crate topology を維持し、各対象 CLI subcommand の Command・入力ポート・Interactor・Application Service・結果型・エラー型を、1 subcommand : 1 usecase に対応する libs/usecase 内の同一 command-context module に配置して役割を分離し、Interactor と Application Service を別 crate に分離しない。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D9] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T028]

## Acceptance Criteria
- [ ] [AC-01] Track context 全体の composition root の公開面は fully wired driver を引き渡す配線 API だけとなり、34 個の public execution method と 3 個の解決 helper を残さない。 [adr: knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D1] [tasks: T001, T027, T028]
- [ ] [AC-02] 対象 CLI subcommand ごとに一つの独立した usecase execution boundary があり、複数 execution method を束ねる Service-style input boundary を新設せず、command selection は primary adapter が担う。 [adr: knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md#D1] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026]
- [ ] [AC-03] 対象 usecase boundary は検証済み入力を受けて presentation-free result または error を返し、stdout / stderr の整形と exit-code mapping は primary adapter が担う。 [adr: knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md#D2] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T032, T033]
- [ ] [AC-04] 各対象 execution path は compatibility shim または中間 *ServiceImpl への runtime reference、call、reverse delegation を残さず、driver → usecase interactor → port の一方向・単一路に従う。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T027, T028]
- [ ] [AC-05] integration tests は TDDD command contexts を含む各対象 Track lifecycle command context、および旧 resolution helper を利用する対象外 context の call site について、同じ入力が構造変更前と同じ CLI 引数処理、stdout、stderr、exit code、および永続化結果を返すことを確認する。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D4] [tasks: T002, T003, T004, T005, T006, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017, T018, T019, T020, T021, T022, T023, T024, T025, T026, T027, T030, T031, T033]
- [ ] [AC-06] 対象 context の composition root public surface と usecase boundary の規律は既存 catalogue lint で検出可能であり、lint が検出しない逸脱は reviewer 判断に委ねる。 [adr: knowledge/adr/2026-08-15-1302-composition-root-pure-di-port-granularity.md#D3, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D3, knowledge/adr/2026-07-23-0111-composition-root-pure-di-realignment.md#D5] [tasks: T029]
- [ ] [AC-07] この slice は、canonical review results の findings block が `findings: zero_findings` を示し、`cargo make ci-track` が成功し、他の未統合 migration track の成果物・変更・実行を前提にせず統合できる。 [adr: knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D2, knowledge/adr/2026-07-23-1318-composition-root-pure-di-migration-initiative.md#D7] [tasks: T029]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

