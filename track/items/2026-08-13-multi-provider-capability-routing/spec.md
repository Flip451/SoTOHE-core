<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# capability routing を Codex custom model provider へ拡張する

## Goal

- [GO-01] capability profile から Codex custom model provider を選択可能にし、capability 単位の provider routing を拡張する。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D1]
- [GO-02] 外部 provider の選択肢、設定方法、互換性の限界を consumer が data-residency 判断とともに理解して採用できるようにする。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2, knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D3]

## Scope

### In Scope
- [IN-01] capability profile schema に optional な model_provider を追加し、非空の指定を capability dispatch の Codex custom provider 選択へ渡す。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D1] [tasks: T001, T002, T003]
- [IN-02] model_provider を利用する profile でも sotp の provider 名を codex に維持し、既存の typed-pipeline provider gate を変更しない。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D1] [tasks: T001, T002, T003]
- [IN-03] DeepSeek、GLM、Qwen 向け Codex custom provider の config.toml 例と、Anthropic 互換経路の設定上の注意を consumer documentation として同梱する。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2] [tasks: T004]
- [IN-04] 外部 provider 利用時の data residency 判断を consumer の責任として文書化し、外部 provider を選ばない既定 profile を維持する。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D3] [tasks: T006]

### Out of Scope
- [OUT-01] sotp が custom provider の存在、endpoint、認証、または provider 固有の意味論を検証・解釈することは対象外とする。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D1] [tasks: T001, T002]
- [OUT-02] sotp に各 provider の API client を直接実装することは対象外とする。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2] [tasks: T003]
- [OUT-03] global な環境変数 redirect を provider routing の正規経路として採用することは対象外とする。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2] [tasks: T004]
- [OUT-04] open weights の self-host provider 経路を実装または運用することは対象外とする。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2] [tasks: T006]

## Constraints
- [CN-01] sotp は model_provider を Codex custom provider 名として解釈せず、非空性だけを検証する pass-through に限定する。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D1] [tasks: T001, T002, T003]
- [CN-02] provider 固有の互換性、既知の制約、および typed-pipeline 準拠の未検証状態は consumer documentation で区別して伝える。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2] [tasks: T004, T005]
- [CN-03] data residency の採否は consumer-owned とし、template は外部 provider を既定化せず、CI enforcement を追加しない。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D3] [tasks: T006]

## Acceptance Criteria
- [ ] [AC-01] model_provider を指定した有効な capability profile は、空でない値だけを検証したうえで、capability dispatch 時に --config model_provider="<id>" を Codex CLI へ渡す。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-02] model_provider が未指定の profile は外部 custom provider を選択せず、指定の有無にかかわらず sotp の provider 名は codex のままで、既存の typed-pipeline provider gate が維持される。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-03] consumer-facing documentation は DeepSeek、GLM、Qwen の Codex custom provider 用 config.toml 例と、Anthropic 互換経路の per-subprocess 環境変数注入、delegate-in-host 非 redirect、Qwen の system-message gap を明記する。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2] [tasks: T004]
- [ ] [AC-04] 各外部 provider の typed-pipeline 準拠は、verdict envelope と structured output を含めて検証されるまで、文書上で未検証として扱われる。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D2] [tasks: T005]
- [ ] [AC-05] 外部 provider への source code・briefing・diff 送信の可否は consumer が判断し、CI は data residency を強制せず、既定 profile は外部 provider を指さない。 [adr: knowledge/adr/2026-08-02-0151-multi-provider-capability-routing.md#D3] [tasks: T006]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

