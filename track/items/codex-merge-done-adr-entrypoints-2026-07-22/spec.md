<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 10, yellow: 0, red: 0 }
---

# Codex 正規入口として track-merge / track-done / adr-add skill を追加する

## Goal

- [GO-01] Codex ホストが SoTOHE ワークフローを、ADR の起草入口から PR merge 後の完了処理まで、正規の $ コマンドだけで実行できるようにする。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1]

## Scope

### In Scope
- [IN-01] `.agents/skills/track-merge/` に `track-merge` skill を追加し、Codex から merge の正規入口を呼び出せるようにする。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1] [tasks: T001]
- [IN-02] `.agents/skills/track-done/` に `track-done` skill を追加し、Codex から merge 後に configured base branch へ戻る完了処理の正規入口を呼び出せるようにする。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1] [tasks: T002]
- [IN-03] `.harness/workflows/` に `adr-add` の provider 非依存 workflow SSoT を整備し、`.agents/skills/adr-add/` にその SSoT を参照する `adr-add` skill を追加して、Codex から pre-track ADR を起草する正規入口を呼び出せるようにする。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1] [tasks: T003]

### Out of Scope
- [OUT-01] skill を追加せず、自然言語による merge、完了処理、または ADR 起草の依頼で正規入口を代替すること。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1] [tasks: T001, T002, T003]
- [OUT-02] merge または done の自動化を `track-adr2pr` skill に組み込み、merge を明示的な user operation ではないものにすること。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1] [tasks: T001, T002]

## Constraints
- [CN-01] 追加する 3 skill は Codex host adapter に限定し、workflow logic を `.harness` 配下の source-of-truth surface から複製または分岐させてはならない。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1] [tasks: T001, T002, T003]
- [CN-02] 各 skill は対応する canonical command の invocation form、tool constraints、reporting boundary だけを host adapter として定め、provider 非依存の workflow logic は `.harness/workflows/` 配下の source-of-truth に置く。 [adr: knowledge/adr/2026-06-30-0425-harness-workflow-ssot-adapters.md#D2, knowledge/adr/2026-06-30-0425-harness-workflow-ssot-adapters.md#D3, knowledge/adr/2026-06-30-0425-harness-workflow-ssot-adapters.md#D8] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] Codex で `$track-merge`、`$track-done`、`$adr-add` の 3 つが正規 skill としてそれぞれ利用可能であり、対応する workflow の入口として機能する。 [adr: knowledge/adr/2026-07-22-1149-codex-merge-done-adr-entrypoints.md#D1] [tasks: T001, T002, T003]
- [ ] [AC-02] 3 skill の各定義は薄い adapter に留まり、対応する provider 非依存 workflow SSoT が `.harness/workflows/` に存在して workflow behavior の権威を保持し、skill 本文には同じ workflow logic を重複して持たない。 [adr: knowledge/adr/2026-06-30-0425-harness-workflow-ssot-adapters.md#D2, knowledge/adr/2026-06-30-0425-harness-workflow-ssot-adapters.md#D3] [tasks: T001, T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/adr.md#Format
- knowledge/conventions/branch-strategy.md#Rules
- knowledge/conventions/track-lifecycle.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 10  🟡 0  🔴 0

