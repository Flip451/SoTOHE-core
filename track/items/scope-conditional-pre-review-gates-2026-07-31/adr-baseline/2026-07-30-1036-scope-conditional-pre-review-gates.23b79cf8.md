---
adr_id: "2026-07-30-1036-scope-conditional-pre-review-gates"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-30"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:sotohe-issues-discussion:2026-07-30"
    status: proposed
---
# pre-review gate を scope 条件付きにし、下流整合の要求を排除する

## Context

review 入口の wrapper（`cargo make track-local-review`）は、scope 引数に関係なく無条件で pre-review liveness gate（`task-contract-check`: chain ③ 帰属信号の NonBlueSignal 検査）を dependency として前置している。このため Phase 2 成果物である types scope のレビューが、下流 Phase 3（impl-plan / task-contract）の帰属状態の整合を先に要求される。

これは依存の逆転である。sot-reentry-sequencing は「types 収束 → impl-plan 再入場」の順を命じるが、この gate 配線は「impl-planner が帰属を動かすまで types review を走らせない」を強制する。Phase 2 の正常な中間状態（宣言先行の 🟡）が gate に阻まれ、実運用では impl-planner を types review より先に走らせる順序破りと、「二段階帰属プロトコル」（🟡 中は todo 専属 → 🔵 後に共有復元）という workaround が発生し、impl-plan reviewer の中間状態誤検出という副産物も生んだ。

根本原則は再入 prerequisite の機械強制（別 ADR）と同一である: **検査対象は当該 scope の直上流 1 層のみ** — 上流収束は検査し（同 ADR）、下流整合は検査しない（本 ADR）。

構造的な原因は、gate 発火が Makefile の dependency 連鎖で行われ、scope を知り得ない位置にあること。Makefile に条件分岐を持たせない方針のため、scope 条件化は CLI 側に置く必要がある。

## Decision

### D1: pre-review gate の適用有無を scope × gate の config マトリクスで宣言する

signal-gates.json（chain × gate の strictness 宣言）と同型に、review scope × pre-review gate の適用有無を config で宣言する。既定は「gate は当該 scope の上流 chain に由来する検査のみ適用」: types / spec など計画系 scope には chain ③ 由来の liveness 検査を適用せず、実装系 scope には現行どおり適用する。types だけの ad-hoc 例外にはしない（spec scope も Phase 1 で同じ逆転を潜在的に持つ）。

### D2: gate 発火の所有権を Makefile dependency から CLI へ移す

`sotp review local` が scope を解決した上で D1 のマトリクスに従い必要な gate を内部発火する。Makefile wrapper は薄い委譲のみとし、scope-blind な dependency 前置を廃する（Makefile にロジックを書かない既定方針に従う）。

## Rejected Alternatives

### A: 現状維持（全 scope 共通の無条件 gate 前置）

SoT chain の順序と逆転した依存を強制し、順序破り・workaround・誤検出を再生産するため却下。

### B: diff で宣言が変わった entry の 🟡 を declaration-ahead として許容（単独案）

判定が diff 依存になり gate の決定性が下がること、task-contract.json が存在しない Phase 2 初回には判定の土台がなく結局 skip 相当が必要になることから、単独修正としては不採用。ただし 🟡 = declaration-ahead という意味論の観察自体は正しく、D1 採用後の task-contract gate 側 todo-lane 意味論の補強として後続の第二段で扱う（本 ADR の scope 外）。

## Consequences

- 良: Phase 2 のレビューが Phase 3 成果物に阻まれる逆転が消え、sot-reentry-sequencing の順序と gate 配線が一致する。二段階帰属プロトコル workaround が不要になる。
- 良: scope × gate マトリクスにより、将来の gate 追加時も「どの scope に効かせるか」が宣言で決まり、scope-blind な前置の再発を防ぐ。
- 負: gate 発火が CLI 内部に移ることで、wrapper 単体では gate の有無が見えなくなる（マトリクス config が可視性の代替）。
- 中立: 実装系 scope の gate 意味論は不変。本 ADR は適用範囲の宣言化のみで、NonBlueSignal 検査自体の判定規則は変えない。

## Reassess When

- 第二段（todo-lane / declaration-ahead 意味論の補強）を検討するとき。
- scope × gate マトリクスの宣言が増え、signal-gates.json との統合が妥当になったとき。
