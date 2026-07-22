<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# テスト義務ゲートへの登録を機構化し、成果物不在による空振り合格を廃する

## Goal

- [GO-01] 型カタログを持つ track が test-obligation gate へ必ず明示的に登録され、義務がゼロである結果と enrollment artifact 自体の不在を区別できるようにする。これにより、成果物不在による `check` の空振り合格を除去し、Phase 2 完了時点から task-status 連動の incremental fulfillment lane を有効にする。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D1, knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15]

## Scope

### In Scope
- [IN-01] type-design workflow の終端で `bin/sotp test-obligation derive` を必須実行し、`obligations.json` と空 records の `test-bindings.json` を実体化する。導出結果がゼロ件でも両 artifact を明示的に残し、catalogue / spec を再入した場合は再導出する。両 artifact は計画 artifacts と同じ commit 単位で扱う。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D1, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002]
- [IN-02] 一つ以上の `<layer>-types.json` が存在する track で `obligations.json` または `test-bindings.json` が不在なら、`bin/sotp test-obligation check` を fail-closed に失敗させる。catalogue を持たない Phase 0〜1 または文書のみの track は、整合的な artifact 不在として従来どおり zero pairs で通す。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6] [tasks: T001]
- [IN-03] implement workflow Step 4 を、既に enrollment 済みの義務に対する binding の増分著作と `evaluate` ループに限定する。`when applicable`、`materialize したら`、`once obligation artifacts exist` といった enrollment を任意化または自己参照させる条件を除去し、full-cycle と obligation-fulfillment の前提文書も同じ規則に同期する。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D3, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T003]

### Out of Scope
- [OUT-01] Rust テストコード本文、binding 内容、または fulfillment verdict の自動生成は対象にしない。enrollment 後の binding 著作と意味論評価は既存の implement / obligation-fulfillment lanes が担う。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D3, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D1] [tasks: T003]
- [OUT-02] 既存の task-status lane、strictest-wins、todo の warning 許容、または merge 時の収束条件を変更しない。本 track は、これらの規則が導出済み義務へ確実に適用される前提を機構化する。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-11-0802-test-obligation-skipped-status-lane.md#D1] [tasks: T001, T003]
- [OUT-03] catalogue を持たない track に obligation artifacts を強制せず、catalogue 存在以外の Phase 2 完了判定も導入しない。必要になった場合は ADR の再評価条件として別途扱う。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2] [tasks: T001]

## Constraints
- [CN-01] `check` は pure-read の fail-closed gate であり続ける。commit gate または `check` の実行中に derive、binding 作成、artifact 修復、または暗黙の再導出を行ってはならない。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2] [tasks: T001]
- [CN-02] enrollment の有無は artifact の存在と catalogue の存在で機械的に判定し、orchestrator の任意判断、軟条件語彙、または task status の有無に依存させない。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2, knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D3] [tasks: T001, T003]
- [CN-03] obligation artifacts の不在、導出結果ゼロ、未解消の binding / verdict を別々に観測可能な状態として維持する。導出結果ゼロを artifact 不在と同一視してはならない。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D1] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] type-design workflow を完了した track では、`derive` の結果件数にかかわらず `obligations.json` と空 records を含む `test-bindings.json` が作成されることを検証する。これらの artifact が plan artifacts と同じ commit 単位に含まれ、catalogue または spec への再入後に再導出されることも検証する。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D1] [tasks: T002]
- [ ] [AC-02] 一つ以上の catalogue がある track で `obligations.json` と `test-bindings.json` のいずれかが不在のとき、`bin/sotp test-obligation check` が非零 exit で fail-closed に失敗することを検証する。両 artifact が存在し導出結果がゼロ件のときは、artifact 不在として扱われないことも検証する。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D1, knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6] [tasks: T001]
- [ ] [AC-03] catalogue を持たない Phase 0〜1 または文書のみの track では、obligation artifacts の両方が不在でも `check` が整合的な zero-pair scope として通ることを検証する。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D2] [tasks: T001]
- [ ] [AC-04] implement workflow は enrollment の有無を条件にせず、既存 artifacts に対する binding の増分著作と `evaluate` ループだけを Step 4 の完了条件として示すことを検証する。full-cycle と obligation-fulfillment の文書も同じ前提で同期され、旧来の任意化・自己参照表現を残さないことを検証する。 [adr: knowledge/adr/2026-07-23-0240-test-obligation-enrollment-mechanization.md#D3] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/track-lifecycle.md#Rules
- knowledge/conventions/task-completion-flow.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0

