<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 15, yellow: 0, red: 0 }
---

# /track:adr2pr の呼び出し型を引数指定から文脈自動解決に戻す

## Goal

- [GO-01] `/track:adr2pr` の起動時に feature 名と primary ADR を再入力せずに済むよう、会話文脈から両方を解決できる呼び出し契約を提供する。明示的な `<feature>` と `--primary-adr` は任意入力として維持し、指定時には文脈解決より優先する。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-1]
- [GO-02] 自動解決による track 初期化の誤選択を防ぐため、解決結果を user が確認または選択してから `/track:init` に渡す。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2, knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-3]

## Scope

### In Scope
- [IN-01] `/track:adr2pr` workflow SSoT の入力契約を、引数なしで起動可能かつ `<feature>` / `--primary-adr` を任意入力として受け付ける形に更新する。両方またはいずれかの明示入力がある場合は、その値を文脈解決より優先する。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-1] [tasks: T001, T002]
- [IN-02] 明示入力で不足している feature 名または primary ADR を会話文脈から解決し、解決された feature 名と primary ADR の組を user に 1 回確認してから `/track:init` へ渡す。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2] [tasks: T001, T002]
- [IN-03] 会話文脈からの解決候補が複数ある場合または候補がない場合に、候補 ADR と feature 名を user に提示し、選択を受けてから処理を続行する。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-3] [tasks: T001, T002]
- [IN-04] workflow SSoT と Claude / Codex provider adapter の呼び出し案内を同じ任意引数・文脈解決・確認 / 選択の契約に揃える。`/track:init` には、解決と user 確認後の feature 名および primary ADR を明示的に渡す。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-1, knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2] [tasks: T001, T002]

### Out of Scope
- [OS-01] `/track:init` 自体の必須入力および初期化手順の変更。`/track:adr2pr` は解決済みかつ user 確認済みの feature 名と primary ADR を明示的に渡す。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2] [tasks: T001, T002]
- [OS-02] 会話文脈を使わずに最新の未着手 ADR などの機械的規則だけで候補を自動選択すること、および候補が曖昧なときに選択を求めず停止すること。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2, knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-3] [tasks: T001, T002]

## Constraints
- [CN-01] 明示的な `<feature>` または `--primary-adr` が supplied の場合は、対応する値について会話文脈からの推定より常に優先する。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-1] [tasks: T001, T002]
- [CN-02] 一意に解決できた場合でも、`/track:init` の前に解決結果を user にちょうど 1 回確認する。確認なしで初期化へ進行しない。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2] [tasks: T001, T002]
- [CN-03] 候補が複数またはゼロで一意に解決できない場合は、user の選択を得るまで `/track:init` へ進めない。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-3] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] `/track:adr2pr` を引数なしで起動したとき、会話文脈から一意に feature 名と primary ADR を解決できれば、その組を user に確認してから track 初期化へ進める。既存の `<feature> --primary-adr <file>.md` 形式も有効で、明示値が解決値より優先される。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-1, knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2] [tasks: T001, T002]
- [ ] [AC-02] feature 名または primary ADR の片方だけが明示されたとき、明示値を維持したまま不足値だけを会話文脈から解決し、完成した組を user に 1 回確認してから `/track:init` へ明示的に渡す。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-1, knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2] [tasks: T001, T002]
- [ ] [AC-03] 会話文脈に複数候補または候補なしがある場合、候補 ADR と feature 名を user に提示して選択を求める。選択を得た後にのみ、選択された組で track 初期化へ進める。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-3] [tasks: T001, T002]
- [ ] [AC-04] workflow SSoT と対象の Claude / Codex adapter が、任意引数、明示値優先、会話文脈からの解決、1 回の確認、および曖昧時の候補選択を矛盾なく案内する。`/track:init` の入力は解決済みの明示値のままである。 [adr: knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-1, knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-2, knowledge/adr/2026-07-20-1508-adr2pr-argless-context-resolution.md#decision-3] [tasks: T001, T002]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/adr.md#ADR vs Convention
- knowledge/conventions/no-upstream-restatement.md#Scope
- knowledge/conventions/track-lifecycle.md#Track Lifecycle
- knowledge/conventions/branch-strategy.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 15  🟡 0  🔴 0

