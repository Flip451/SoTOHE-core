<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 29, yellow: 0, red: 0 }
---

# 欠ける typed-pipeline 専用経路に grok を割り当て可能にする

## Goal

- [GO-01] 現在 fail-closed している typed-pipeline 専用経路 4 つについて、provider と fast_provider の両方で grok を割り当て可能にする。起動は既存の grok 写像を共有 runner の grok arm 1 本で行い、専用経路を capability exec に合流させず、対象 4 経路と sample の committed 値は grok に書き換えず、shipped default の書き換えは committed の reviewer provider のみとする。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1, knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D2, knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D3, knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D4, knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5]

## Scope

### In Scope
- [IN-01] agent-profiles.json の provider が grok のとき、次の 4 つの typed-pipeline 専用経路が grok を起動できる: ref-verifier-chain1、ref-verifier-chain2、obligation-fulfillment-verifier、waiver-verifier。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T001]
- [IN-02] 同じ 4 capability について、fast_provider に grok を書いたときも、未対応 provider として fail-closed しない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D2] [tasks: T001]
- [IN-03] これら 4 経路の起動契約は既存の grok 写像（model / effort / resume / sandbox）を使う。経路ごとの起動契約や例外表は新設しない。返却は envelope の構造化出力フィールドから取り、テキスト欄は使わない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D3] [tasks: T001]
- [IN-04] 対象 4 capability は、既存の共有 provider-dispatching process runner に足す grok arm 1 本を使う。capability ごとの独立した grok 起動経路は持たない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D4] [tasks: T001]
- [IN-05] committed の reviewer は provider を grok、model を grok-4.6 とする。fast_provider と fast_model は現行のまま（codex / gpt-5.6-luna）とする。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]

### Out of Scope
- [OS-01] 対象 4 つの専用経路を capability exec に合流させることは対象外である。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T001]
- [OS-02] pr-reviewer の hosted 経路を grok 割り当て可能にすることは対象外である。hosted 経路は CLI 起動契約の外に残す。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T002]
- [OS-03] profile 上の全 capability、または grok arm が無い runner すべてへ今回の対象を広げることは対象外である。開集合は上記 4 名前の列挙で閉じる。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T001, T002]
- [OS-04] これら 4 経路向けの grok 起動契約や例外表を新設することは対象外である。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D3] [tasks: T001]
- [OS-05] capability ごとに独立した grok runner を持つことは対象外である。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D4] [tasks: T001]
- [OS-06] 対象 4 capability について、committed の agent-profiles.json と sample profile の provider / fast_provider 値を grok に書き換えることは対象外である。その採否は設定者の編集に残す。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]
- [OS-07] reviewer 以外の capability の shipped default を grok に指すことは対象外である。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]

## Constraints
- [CN-01] 対象 4 つの専用経路は専用経路のまま残し、capability exec に合流させない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T001]
- [CN-02] これら 4 経路の model / effort / resume / sandbox 写像は既存の grok 写像を使い、経路ごとの例外表は持たない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D3] [tasks: T001]
- [CN-03] これら 4 経路の grok 実行の返却抽出は envelope の構造化出力フィールドに限り、テキスト欄は返却チャネルにしない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D3] [tasks: T001]
- [CN-04] 対象 4 capability は共有 process runner の grok arm 1 本を共有する。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D4] [tasks: T001]
- [CN-05] 対象 4 経路と sample については割り当て可能性だけを提供し、それらの shipped な provider 値は grok に変えない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]
- [CN-06] shipped default を grok に書き換えるのは committed の reviewer に限る。他 capability の shipped default は変えない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] 対象 4 capability のそれぞれについて、provider が grok の profile は、未対応の grok provider として fail-closed せず grok を起動する。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T001]
- [ ] [AC-02] 対象 4 capability のそれぞれについて、fast_provider が grok の profile は、未対応の grok provider として fail-closed せず fast 側の grok を起動する。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D2] [tasks: T001]
- [ ] [AC-03] それら grok 起動は既存の grok 写像（model / effort / resume / sandbox）を適用する。これら 4 経路専用の起動契約表や例外表は存在しない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D3] [tasks: T001]
- [ ] [AC-04] それら経路での grok 実行が成功したとき、返却は envelope の構造化出力フィールドの値だけである。その値が無い場合は fail-closed し、envelope の失敗理由を出し、テキスト欄を結果として扱わない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D3] [tasks: T001]
- [ ] [AC-05] 対象 4 capability は、既存の共有 provider-dispatching process runner 上の grok arm 1 本を共有し、それぞれ独立した grok runner を持たない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D4] [tasks: T001]
- [ ] [AC-06] 変更後も、対象 4 capability の committed agent-profiles.json と sample profile の provider / fast_provider 値は grok を選ばない。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]
- [ ] [AC-07] committed の reviewer は provider が grok、model が grok-4.6 であり、fast_provider は codex、fast_model は gpt-5.6-luna のままである。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]
- [ ] [AC-08] pr-reviewer の hosted 経路は今回の CLI 起動契約の外に残り、grok を割り当て不能のままである。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T002]
- [ ] [AC-09] 対象 4 capability は capability exec の起動対象に追加されず、それぞれの typed-pipeline 専用経路から grok を起動する。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D1] [tasks: T001]
- [ ] [AC-10] committed の agent-profiles.json を設定済み base と比較したとき、reviewer の provider と model 以外の capability shipped default（provider / model / fast_provider / fast_model）に差分が無い。 [adr: knowledge/adr/2026-08-18-1534-grok-configurable-for-all-capabilities.md#D5] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 29  🟡 0  🔴 0

