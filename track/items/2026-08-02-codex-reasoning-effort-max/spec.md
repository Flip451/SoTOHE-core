<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 20, yellow: 0, red: 0 }
---

# reasoning effort に max 段を追加し、限定レーンを Luna Max へ移行する

## Goal

- [GO-01] provider 非依存の reasoning effort 語彙として max を利用可能にし、プロバイダーごとの対応可否を fail-closed で検証する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D1]
- [GO-02] 実装・修正レーンだけを Luna Max で限定運用し、品質重視レーンは Terra の既定構成に維持したうえで、最初の運用観測を記録する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2]

## Scope

### In Scope
- [IN-01] reasoning effort の選択肢に max を追加し、max を指定した実行が provider に max として発行されるようにする。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D1] [tasks: T001]
- [IN-02] capability 実行、agent profile 検証、review 系実行経路は max を含む reasoning effort を一貫して扱い、プロバイダーが受理しない effort 指定を拒否する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D1] [tasks: T001, T005, T006]
- [IN-03] 既定 capability profile の implementer、review-fix-lead、dry-fix-lead を gpt-5.6-luna と max の組み合わせへ変更する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T002]
- [IN-04] spec-designer、impl-planner、researcher、dry-checker の final、obligation-fulfillment-verifier、waiver-verifier は Terra の既定構成を維持する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T002]
- [IN-05] Luna Max の割り当てが不完全な出力、timeout、または gate failure になった場合、同じ割り当てを従来の Terra 構成で 1 回再実行し、Terra で成功した事例を model regression 候補として記録する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T003]
- [IN-06] この限定運用を Luna Max の最初の観測として、取得可能な品質、credits、所要時間、再試行回数を track 完了時に記録する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T004]

### Out of Scope
- [OUT-01] Ultra reasoning effort の採用、provider への発行、または既定 profile への設定は対象外とする。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D1] [tasks: T001]
- [OUT-02] 設計、計画、調査、または最終 verdict レーンを Luna Max へ一括移行することは対象外とする。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T002]
- [OUT-03] Luna Max の失敗時に runtime が自動的に Terra へフォールバックする仕組みは対象外とする。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T003]
- [OUT-04] Luna Max の過去 track 実績を作成すること、または採用前提として過去 track を再実行して比較することは対象外とする。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T004]

## Constraints
- [CN-01] provider × effort の妥当性は provider 側の宣言に基づき fail-closed で検証し、max の enum 追加は特定 provider に依存してはならない。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D1] [tasks: T001]
- [CN-02] Luna Max の適用範囲は implementer、review-fix-lead、dry-fix-lead に限定し、列挙された品質重視レーンの Terra 構成を変更してはならない。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T002]
- [CN-03] 失敗後の Terra 再実行は runtime の自動フォールバックではなく、同一割り当てに対する 1 回の明示的な再実行として扱う。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T003]

## Acceptance Criteria
- [ ] [AC-01] max を指定した有効な provider 構成は検証を通過して provider に max として渡され、当該 provider が受理しない effort 構成は UnsupportedEffort として拒否される。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D1] [tasks: T001]
- [ ] [AC-02] (1) capability 実行では、max を設定した対応 provider の割り当てが provider 呼出しに max を含めて開始される。(2) agent profile 検証では、max を設定した profile は provider が max を受理する場合に受理され、受理しない場合は provider 呼出し前に拒否される。(3) review 系実行経路では、max を設定した review 割り当てが provider 呼出しに max を含めて開始され、通常の完了時には当該 review の verdict が記録される。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D1] [tasks: T001, T005, T006]
- [ ] [AC-03] 既定 profile では implementer、review-fix-lead、dry-fix-lead だけが gpt-5.6-luna + max となり、spec-designer、impl-planner、researcher、dry-checker final、obligation-fulfillment-verifier、waiver-verifier は Terra のままである。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T002]
- [ ] [AC-04] Luna Max の各割り当てについて、(1) 設定された制限時間に達する前に終了し、割り当てに定義された完了結果または verdict が記録されない場合を不完全出力、(2) 制限時間までに当該完了結果または verdict が記録されない場合を timeout、(3) 適用される gate が失敗結果を記録した場合を gate failure と判定する。いずれかの判定が記録された割り当ては、同一の入力・完了条件・適用 gate を従来の Terra 構成でちょうど 1 回再実行し、Terra で成功した場合は model regression 候補として記録する。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T003]
- [ ] [AC-05] track 完了時に、この最初の限定運用で実行した各割り当てについて、割り当て識別子とレーン、使用した provider と effort、品質（定義された完了条件の充足・非充足、適用 gate ごとの結果、および不完全出力・timeout・gate failure の該当有無）、provider が報告した credits（報告されない場合は取得不能である旨）、開始から完了結果または verdict の記録までの所要時間、Terra 再実行を含む実行回数、Terra 再実行の結果と model regression 候補の該当有無を記録する。比較可能な過去の Luna Max 実績は存在しないため、過去 track との A/B 比較を前提にしない。 [adr: knowledge/adr/2026-08-02-0151-codex-reasoning-effort-max.md#D2] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 0  🔴 0

