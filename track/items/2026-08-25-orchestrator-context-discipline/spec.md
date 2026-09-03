<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 23, yellow: 0, red: 0 }
---

# オーケストレーターの文脈摂取を規律化する

## Goal

- [GL-01] オーケストレーション手順が CLI 要約を一次情報とし、親コンテキストへの不要な全文摂取を避ける。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D1]
- [GL-02] PR 指摘の修正を含む実装判断で委譲を正規経路とし、親の直接実装を委譲失敗時の回復に限定する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D2]
- [GL-03] 長時間ゲートの結果待ちでポーリングを指示しない。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D3]

## Scope

### In Scope
- [IN-01] Workflow SSoT、thin adapter、policy/capability 文書、および always-applied rules surface のオーケストレーター向け手順を改訂する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D1, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D2, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D3, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T019, T020, T021, T022, T023, T024, T007, T008, T009, T010, T011, T012, T013, T014, T015]
- [IN-02] オーケストレーターが進行・レビュー要否・義務状態・カタログ照会で CLI 要約を一次情報とし、成果物本文は差分またはブロッカーの調査時にのみ開く手順を定める。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D1] [tasks: T001, T005, T006, T007, T011, T015]
- [IN-03] PR 指摘の修正を briefing、implementer または review-fix-lead への委譲、ローカル収束、commit workflow の順で扱い、親の直接編集は委譲失敗時の回復に限定する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D2] [tasks: T002, T019, T022, T008, T012]
- [IN-04] 長時間ゲートの待機、evaluate の実行、計画成果物コミット後・最初の実装バッチ後・PR レーン開始時の親セッション更新点を手順化する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D3, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T003, T004, T020, T021, T023, T024, T009, T010, T013, T014]
- [IN-05] PR reviewer 向け文書を orchestrator の always-applied 面から分離し、短い orchestrator rules、ルートのポインタ化、consumer が所有する provider 互換設定の文書化を行う。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5] [tasks: T015, T016]
- [IN-06] .harness/config/agent-profiles.json の orchestrator profile の既定 reasoning effort を中位にする。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D6] [tasks: T017]

### Out of Scope
- [OS-01] Rust コード、CLI または Makefile の stdout-format、ならびに gate stdout summary contract の変更。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T019, T020, T021, T022, T023, T024, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016, T017]
- [OS-02] Skills catalogue を単一の入口文書へ畳み込むこと。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5] [tasks: T011, T012, T013, T014]
- [OS-03] ホスト固有の backgrounding threshold、通知形式、または compaction timing への適応を設計または実装すること。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D3, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T003, T004, T020, T021, T023, T024, T009, T010, T013, T014]

## Constraints
- [CN-01] 実行時に agent または運用者へ動作を指示する文書は自己完結させ、特定 ADR の日付 ID または path を本文から参照しない。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5] [tasks: T001, T002, T003, T004, T005, T006, T019, T020, T021, T022, T023, T024, T007, T008, T009, T010, T011, T012, T013, T014, T015, T016]
- [CN-02] provider 側の互換 rules 設定は consumer 所有として提供と文書化に留め、設定値、ファイル存在、または散文を CI の hard-fail 条件にしない。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5] [tasks: T015, T016]
- [CN-03] 摂取規律は prompt-level の手順として扱い、機械強制や実行時 token metric を完了条件に追加しない。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D1] [tasks: T001, T002, T003, T004, T005, T006, T019, T020, T021, T022, T023, T024, T007, T008, T009, T010, T011, T012, T013, T014]

## Acceptance Criteria
- [ ] [AC-01] 改訂済みオーケストレーション手順は、CLI 要約を一次情報とし、workflow 冒頭の成果物本文一括読み込みを要求せず、本文閲覧を差分またはブロッカー調査に限定している。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D1] [tasks: T001, T005, T006, T007, T011, T015]
- [ ] [AC-02] PR review 手順は、指摘修正を briefing から委譲、ローカル収束、commit workflow へ進め、親の直接編集を委譲失敗時の回復としてのみ記述している。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D2] [tasks: T002, T019, T022, T008, T012]
- [ ] [AC-03] 長時間ゲートを扱う手順は、一回のブロッキング待機、または host が background 化した場合の一回の完了通知後の結果読取りを記述し、ポーリングを指示しない。evaluate は修復作業時の同期実行としてのみ記述している。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D3] [tasks: T003, T020, T023, T009, T013]
- [ ] [AC-04] adr2pr 手順は、計画成果物コミット後、最初の実装バッチ後、PR レーン開始時を親セッション更新点として明記し、更新できない host では user への更新要求を許容している。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T004, T021, T024, T010, T014]
- [ ] [AC-05] always-applied 面は PR reviewer 向け文書を読み込まず、orchestrator 向け rules 文書に委譲、CLI 一次情報、git 直叩き禁止の三つを明記し、ルート文書はその rules へのポインタになっている。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5] [tasks: T015]
- [ ] [AC-06] consumer-facing 文書は provider 互換 rules 設定を consumer 所有として説明する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5] [tasks: T016]
- [ ] [AC-07] .harness/config/agent-profiles.json の orchestrator default reasoning effort は中位値である。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D6] [tasks: T017]
- [ ] [AC-08] 改訂後の各対象面を、次の固定された対応で文書内容から個別に確認し、全ての確認を満たす。workflow SSoT は CLI 要約を一次情報とし、PR 指摘修正を委譲経路とし、長時間ゲートを一回の待機（ポーリングなし、evaluate は修復時の同期実行のみ）として扱い、計画成果物コミット後・最初の実装バッチ後・PR レーン開始時のセッション更新点を示す。thin adapter は workflow SSoT を指し、これらと異なる手順を持たない。policy/capability 文書は workflow SSoT と同じ委譲・CLI 一次情報・待機・セッション更新の境界を記述する。always-applied rules は orchestrator 向けに委譲・CLI 一次情報・git 直叩き禁止を指示する。consumer-facing 文書は provider 互換 rules 設定を consumer 所有として扱う。agent profile は orchestrator の既定 reasoning effort を中位にする。各対象面で対応する確認項目の欠落、旧指示の残存、または対象面間の明示された境界との矛盾が一つでもあれば不合格とする。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D1, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D2, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D3, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D5, knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D6] [tasks: T018]

## Signal Summary

### Stage 1: Spec Signals
🔵 23  🟡 0  🔴 0

