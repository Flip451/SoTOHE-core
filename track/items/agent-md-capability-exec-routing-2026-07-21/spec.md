<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 11, yellow: 0, red: 0 }
---

# .claude/agents の capability 定義を capability exec 経由へ誘導する

## Goal

- [GO-01] capability に対応する `.claude/agents/*.md` の description が、orchestrator を provider / model 解決を内包する `bin/sotp capability exec` の正規経路へ誘導し、Agent tool による直接呼び出しを行わせないようにする。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1]

## Scope

### In Scope
- [IN-01] `adr-editor`、`dry-fix-lead`、`impl-planner`、`implementer`、`researcher`、`review-fix-lead`、`rollback-diagnoser`、`spec-designer`、`type-designer` に対応する `.claude/agents/*.md` の各 description を、`bin/sotp capability exec` 経由の呼び出しと Agent tool の直接呼び出し禁止を明示する内容へ更新する。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]
- [IN-02] 各 description が agent 選択時に、capability の provider / model routing SSoT である `.harness/config/agent-profiles.json` を経由する正規 dispatch 経路を理解できる誘導を提供する。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]

### Out of Scope
- [OUT-01] `.claude/agents/README.md` のような index を capability agent definition として扱ったり、その description 更新対象に含めたりしない。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]
- [OUT-02] capability 相当 agent の Agent tool 直接呼び出しを hook 等で機械的に block する enforcement は導入しない。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]
- [OUT-03] `.harness/config/agent-profiles.json` の capability → provider / model routing、または `bin/sotp capability exec` の dispatch の仕組み自体は変更しない。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]

## Constraints
- [CN-01] description による案内は、provider / model 解決の唯一の権威を `.harness/config/agent-profiles.json` とし、その解決を内包する `bin/sotp capability exec` を正規経路として一貫して示さなければならない。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]
- [CN-02] この track の対策は description による誘導に留め、agent 判定の複雑さや誤検知を伴う新しい機械的 block を追加しない。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] 対象となる 9 個の capability-corresponding `.claude/agents/*.md` 定義のそれぞれについて、description が `bin/sotp capability exec` を経由して呼び出すことと、Agent tool で直接呼び出さないことの両方を明記している。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]
- [ ] [AC-02] 各対象 description は、direct Agent-tool invocation が provider / model 解決を bypass すること、及び `bin/sotp capability exec` が `.harness/config/agent-profiles.json` による profile 解決を内包する正規経路であることを明記している。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]
- [ ] [AC-03] `.claude/agents/README.md` は index として扱われ、agent definition の更新対象に含まれない。hook などを用いる direct Agent-tool invocation の新規機械的 block も存在しない。 [adr: knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1, knowledge/adr/2026-07-21-1522-agent-md-capability-exec-routing.md#D1] [tasks: T001, T002]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/adr.md#Format
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 11  🟡 0  🔴 0

