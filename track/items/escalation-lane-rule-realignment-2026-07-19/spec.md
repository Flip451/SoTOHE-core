<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 14, yellow: 0, red: 0 }
---

# escalation lane の規約・guardian 挙動を decision-freeze ADR D6/D7 に整合させる

## Goal

- [GO-01] in-track ADR 編集と track-born ADR の裁定を扱う運用文書を、予期された編集、予期しない baseline 不一致、および未承認の新規 decision を区別する D5–D7 の経路へ整合させる。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7]

## Scope

### In Scope
- [IN-01] pre-track ADR authoring convention の Phase 1 以降の自律区間を、grounding escalation で orchestrator が adr-editor に指示した編集は予期された編集として扱い、編集直後に reason 必須の escalation snapshot を実行する規範へ更新する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6] [tasks: T001]
- [IN-02] guardian capability 契約と escalation lane の workflow 文書を、adr-diagnoser が予期しない baseline byte mismatch の D5 トリアージだけを担い、予期された escalation 編集の前提判定を要求しない記述へ整合させる。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6] [tasks: T002, T003]
- [IN-03] guardian-facing operational docs を、非 user 系根拠だけの track-born ADR は chain ⓪ の 🟡 評価と strict merge gate により非同期の user 裁定を待てること、user_decision_ref 昇格後に reason 必須の new-adr snapshot を実行することへ整合させる。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T002, T003]

### Out of Scope
- [OUT-01] primary ADR 本文またはその D5–D7 の決定を編集、再解釈、または supersede すること。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6]
- [OUT-02] adr_user signal evaluator、signal-gates の strictness、または user_decision_ref の chain ⓪ 評価機構を変更すること。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7]
- [OUT-03] Phase 0 の user 承認エスカレーション、または pr-review の Accepted Deviations で既に要求される同期 pause を変更すること。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7]

## Constraints
- [CN-01] 運用文書は escalation / new-adr snapshot の reason 必須、orchestrator 起動の原子的な機械書込、判定 capability の read-only 性を矛盾なく記述する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6] [tasks: T001, T002, T003]
- [CN-02] D5 の diagnoser triage は経路不明の baseline 乖離に限定し、非意味的 mismatch の再刻印、意味的 mismatch の復元、判断不能時の fail-closed を保持する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5] [tasks: T002, T003]
- [CN-03] 新規 decision の user 裁定を同期 pause として一般化せず、非 user 根拠の間は 🟡 と strict merge gate を用い、裁定後にだけ user_decision_ref 昇格と new-adr snapshot を行う。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T002, T003]

## Acceptance Criteria
- [ ] [AC-01] Phase 1–3 grounding escalation の adr-editor 編集後、operational docs は adr-diagnoser verdict を待たず escalation kind と自己完結の reason による snapshot へ進む経路を記述する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6] [tasks: T001, T003]
- [ ] [AC-02] baseline mismatch が予期しない場合に限り、operational docs は adr-diagnoser の read-only verdict を起点として D5 の non-semantic restamp、deviation restore、または fail-closed routing を選ぶ。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5] [tasks: T002, T003]
- [ ] [AC-03] review finding 等の非 user 根拠しか持たない新規 ADR について、guardian verdict と workflow が同期 user adjudication を要求せず、chain ⓪ 🟡 と strict merge gate が merge 前の裁定を強制する経路を記述する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T002, T003]
- [ ] [AC-04] user 裁定後の track-born ADR は、user_decision_ref を昇格してから working tree を new-adr kind と reason 必須で snapshot し、以後の通常の凍結対象となる経路を記述する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/no-upstream-restatement.md#Scope
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 14  🟡 0  🔴 0

