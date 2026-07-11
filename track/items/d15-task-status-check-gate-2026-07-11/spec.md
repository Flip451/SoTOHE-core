<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 24, yellow: 0, red: 0 }
---

# D15 実装 — test-obligation check の task-status 連動判定（todo 帰属 🟡 許容）

## Goal

- [GO-01] `bin/sotp test-obligation check` が、義務・edge の task 帰属を既存の track artifact から決定論的に解決し、未着手 task に帰属する未解消を許容することで、Phase 2 完了直後の early-derive と実装 batch ごとの増分 fulfillment を可能にする。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15]
- [GO-02] task-status lane による最終 gate 解釈を導入しても、obligation-fulfillment の fail-closed 保証、merge 時の収束、および `results` と `check` の責務分離を維持する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D14]

## Scope

### In Scope
- [IN-01] 義務または edge の task 帰属を、`obligations.json` の target entry から `task-contract.json` の entry 帰属、さらに `impl-plan.json` の task status（todo / in_progress / done）へ至る既存 artifact の合成で解決する。同一 entry が複数 task に帰属するときは、done または in_progress があれば todo より strict lane を適用する（strictest-wins）。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T003, T006]
- [IN-02] `check` の最終 gate 判定を task-status lane で解釈する。done・in_progress 帰属の missing・stale・verdict 欠如は従来どおり fail-closed で block し、todo 帰属の同じ未解消だけを 🟡 warning として報告して gate を通す。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T004]
- [IN-03] task status を、drift と verdict 不在の検出後に行う最終 gate 解釈だけへ適用する。missing / orphaned と鮮度失効の分類、edge 局所の fulfillment 判定、rules の load-time totality、および malformed artifact の検出は status 非依存のまま維持する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9] [tasks: T002]
- [IN-04] unattributable entry、rules の load-time totality 違反、artifact malformed を構造軸として扱い、todo 帰属を含む全 status で fail-closed に block する。entry 帰属の完全性は既存の `task-contract coverage` gate を前提にし、その前提が破れた場合も status による許容を適用しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D10, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11] [tasks: T001, T002, T004]
- [IN-05] `sotp test-obligation results` に、todo / in_progress / done lane ごとの unresolved 件数と、各 lane の missing / stale / verdict 欠如の内訳を表示する informational 集計を追加する。gate の pass/fail は引き続き `check` だけが担い、`results` は read error を含め常に exit 0 とする。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D14] [tasks: T003, T005, T006]
- [IN-06] verifier-prompt fingerprint が不一致または不在の verdict を存在しない verdict として扱う既存の freshness 規律を、D15 の task-status lane による最終解釈へそのまま接続する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T002, T003]
- [IN-07] strictest-wins の task 帰属解決で `skipped` となる義務は、`todo` より厳しい独立 lane に置き、status 順序 `done > in_progress > skipped > todo` に従う。binding 欠如、verdict 欠如、または鮮度失効の未解消は `check` が fail-closed に block する。`results` は `skipped` を独立した informational lane として集計表示し、exit は常に 0、pass/fail の判定は `check` だけが担う。 [adr: knowledge/adr/2026-07-11-0802-test-obligation-skipped-status-lane.md#D1] [tasks: T002, T003, T004, T005]

### Out of Scope
- [OUT-01] task-status lane のための新しい attribution artifact、status field、または coverage mechanism を追加しない。帰属は既存 artifact の決定論的合成と既存 `task-contract coverage` gate を利用する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T003, T006]
- [OUT-02] status を input とする drift 算出、verdict の意味論評価、または bindings totality の再設計は行わない。todo の猶予は未解消の表示と最終 gate verdict に限る。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6] [tasks: T002, T003]
- [OUT-03] `.harness/config/signal-gates.json` の chain × gate 宣言、既存の merge task-completion 前提、または `sotp pr wait-and-merge` の待機・merge 手順を変更しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T006]
- [OUT-04] `derive`、`evaluate`、および `bindings-skeleton` の artifact 作成・意味論評価・authoring helper としての責務を変更しない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D14] [tasks: T002, T003]

## Constraints
- [CN-01] `check` は pure-read の fail-closed gate であり続ける。status lane の解釈は artifact、binding、verdict cache、または obligations を書き換えて unresolved を解消する経路を作らない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D11, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T004]
- [CN-02] todo lane は、未着手 task に帰属する missing・stale・verdict 欠如だけを一時的に 🟡 として扱う。構造破綻、done / in_progress 帰属の未解消、または attribution を解決できない状態を許容してはならない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T001, T002, T003, T004, T005]
- [CN-03] fingerprint 不一致または legacy fingerprint 不在は error ではなく verdict 欠如として扱い、手動 purge や手編集を要求せず、status lane の規則で最終 gate を解釈する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T003]
- [CN-04] merge task-completion 前提で todo task が残らないため、todo lane の 🟡 許容は merge 時点で必ず消尽する。本番運用へ 🟡 未解消が漏出する経路を作らない。これは runtime の例外経路や追加の merge gate を必要としない。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T003, T004, T005]

## Acceptance Criteria
- [ ] [AC-01] `obligations.json` の target entry が `task-contract.json` の entry 帰属と `impl-plan.json` の `tasks[]` status を経由して決定論的に lane へ解決されることを検証する。同一の missing、stale、または verdict 欠如が、done / in_progress 帰属では非零 exit で block し、todo 帰属だけでは 🟡 warning を表示して exit 0 となることを検証する。複数 task 帰属では done または in_progress が todo より優先する strictest-wins により strict lane の結果になることも検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T004]
- [ ] [AC-02] todo task にだけ未解消が残る `check` は pass し、warning 出力で todo lane の未解消を識別できる。一方、義務が導出されない fulfillment binding（`orphaned`）、unattributable entry、rules totality 違反、または malformed artifact がある場合は todo status であっても非零 exit で block する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D10] [tasks: T001, T002, T004]
- [ ] [AC-03] missing / orphaned、spec_changed / decl_changed / test_changed / reason_changed、および verdict 欠如の検出結果が task status によって変化せず、status が変えるのは検出済み未解消に対する最終 gate verdict だけであることを検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D6, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D9] [tasks: T002]
- [ ] [AC-04] fingerprint 不一致または fingerprint 不在の cache entry は current verdict として採用されず、verdict 欠如として扱われる。その結果は done / in_progress lane では block、todo lane では warning と exit 0 になることを検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D16] [tasks: T002, T004]
- [ ] [AC-05] `sotp test-obligation results` が todo / in_progress / done ごとに unresolved 件数を表示し、各 lane で missing / stale / verdict 欠如を区別して集計する。unresolved の有無や read error の有無にかかわらず exit は常に 0 であり、gate の合否を表さないことを検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15, knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D14] [tasks: T003, T005, T006]
- [ ] [AC-06] task-status lane の導入後も、`signal-gates.json` に変更がなく、既存の merge task-completion 前提により todo task が残らない時点では todo lane の 🟡 未解消が残らないことを検証する。 [adr: knowledge/adr/2026-07-02-0359-test-obligation-and-fulfillment-gate.md#D15] [tasks: T002, T004]
- [ ] [AC-07] strictest-wins の task 帰属解決が `skipped` となる missing、stale、または verdict 欠如は、todo 猶予として warning にせず、`check` が非零 exit で fail-closed に block することを検証する。`results` が skipped の未解消を独立 lane として表示し、unresolved や read error の有無にかかわらず exit 0 を維持することも検証する。 [adr: knowledge/adr/2026-07-11-0802-test-obligation-skipped-status-lane.md#D1] [tasks: T002, T003, T004, T005]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule
- knowledge/conventions/testing.md#Rules
- knowledge/conventions/tddd-product-correctness.md#判断基準
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 24  🟡 0  🔴 0

