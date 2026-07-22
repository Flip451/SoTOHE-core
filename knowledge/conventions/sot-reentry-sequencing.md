# SoT 再入の順次処理規律

## Purpose

SoT Chain の back-and-forth において、どの SoT へ回帰するかのルーティングは `rollback-diagnoser` の責務である (`.harness/capabilities/rollback-diagnoser.md`)。本規約はその後段 — ルーティングされた上流フェーズから下流へ降りる際の順次処理 — を規律として固定する。各下流フェーズは直上流フェーズの再収束を待ってからのみ再開し、上流編集の必要性が判明した下流作業は即時に中断して上流へ戻す。意味論検証の判定は当該上流 chain に関係する指摘に限定し、列挙不能時は列挙可能になり次第、検証する。prompt-level の規律であり、gate / CI / signal 設定の変更は伴わない。

## Scope

- 適用対象: track 内の Phase 1 (spec-design) / Phase 2 (type-design) / Phase 3 (impl-plan) / 実装フェーズの再開判断、および back-and-forth 中の orchestrator と writer capability の振る舞い。
- 適用外: 回帰先の判定そのもの (rollback-diagnoser の lane)、ADR 側の編集裁定 (`pre-track-adr-authoring.md` の二箱分離)、gate / CI の実装。

## フェーズ収束の定義

フェーズ X が**収束している**とは、次のすべてが成立していること:

1. **参照信号**: X の成果物を引用側 (下流) とする chain の参照信号 — 参照の存在を機械評価した 🔵 / 🟡 / 🔴 — が、`.harness/config/signal-gates.json` の当該 chain × gate 指定 (interim / strict) を満たす。許容水準の SSoT は同設定ファイルであり、本規約は値を再記述しない。
2. **意味論検証**: 当該上流 chain に関係する `bin/sotp ref-verify` の指摘が全て解消されている。参照先の内容と引用側の記述の意味的整合を確認する、参照信号とは独立の検証である。scope と chain の対応は ref-verify 側の定義に従い、本規約は対応表を再記述しない。`adr_user` chain (ADR の収束) には意味論検証を要求しない。**他 chain の指摘、および直後の下流 writer 再走で再生成予定の stale 下流成果物に起因する列挙失敗は、上流収束の判定に関与しない。** 既知の当該 chain 指摘は chain 限定の読み出し (例: `bin/sotp ref-verify results --chain 1`) で確認する。列挙失敗中は fresh な検証を生成できないが、それは条件の未充足ではない — 検証は列挙可能になり次第 (通常は即時、abort 時は下流再生成直後の full run で) 実行し、そこで当該 chain の指摘が出れば即時突き返し規則に従う。
3. **レビュー**: 該当 SoT スコープの review が `zero_findings` で完了している。

## 再開 Prerequisite (直上流 1 層のみ)

| 再開フェーズ (writer) | 必要な収束 |
|---|---|
| spec-design (spec-designer) | ADR の収束 (`adr_user` chain) |
| type-design (type-designer) | spec の収束 (`spec_adr` chain) |
| impl-plan (impl-planner) | カタログの収束 (`catalog_spec` chain) |
| 実装 (implementer) | カタログの収束 **かつ** impl-plan スコープ review 収束 (下記例外あり) |

各フェーズは直上流 1 層のみを検査する。上流の上流の収束は直上流の収束が推移的に保証する (SoT Chain の layer skip 禁止と同型)。

## 即時突き返し規則

- 下流作業中に、収束済み上流 SoT への編集の必要性が発見された時点で、下流作業を中断して上流へ戻る。回帰先が自明でなければ diagnose ルート (`/track:diagnose`) を経由する。
- 「後でまとめて直す」「変更の有無を判定して続行する」は禁止する — 収束の有効性判定という裁量を挟まない。
- 上流 SoT への編集は適用された時点で当該フェーズの収束を即座に失効させる。再収束 (上記 3 要素) まで、その下流のフェーズは再開禁止。意味論検証要素の判定は item 2 のとおり当該 chain の指摘に限る — 列挙 abort 中に fresh 検証を生成できないことは再開を妨げず、列挙可能になり次第の検証で当該 chain の指摘が出れば即時に上流へ戻る。
- **artifact 編集に対する唯一の例外**: `impl-plan.json` は review 収束後も `bin/sotp track transition` による task ステータス遷移のみ許容される。この例外は本規律 (順次処理) 上のものに限る — 遷移は上流 rollback も下流停止も要求しない。ただし hash ベースの commit gate が要求する impl-plan final `zero_findings` review refresh (`.harness/workflows/track/full-cycle.md` の lifecycle tail) は引き続き必須であり、本例外はそれを免除しない。それ以外の impl-plan 変更は通常どおり失効・再収束を要する。この例外を他の SoT へ一般化しない。

## 役割分担

- **回帰先の判定**: `rollback-diagnoser`。出力は勧告であり、orchestrator が `reason` を不十分と判断すれば override し得る (既存どおり)。
- **Prerequisite の充足確認と降下順序の遵守**: dispatch する orchestrator。
- **各 writer capability**: 自分の再開 Prerequisite が briefing 上満たされていない場合、作業せず orchestrator へ差し戻す。

## Examples

- Good: impl フェーズの review finding が spec の欠陥に由来 → `/track:diagnose` が `spec` を勧告 → spec-designer 再入の前に orchestrator が ADR の収束を確認 → spec 再収束 (信号 + 当該 chain の指摘解消 + spec review) → その後にのみ type-design 以降を再開。
- Good: spec の修復で stale な catalogue が残り `ref-verify run` が列挙 abort する → signal と spec review を再収束し、`ref-verify results --chain 1` で既知の Chain ① 指摘ゼロを確認 → type-design で catalogue を再生成 → 直後の full `ref-verify run` で全 chain を検証してから次フェーズへ降りる。
- Good: type-design 作業中に spec 側の曖昧さを発見 → type-design を中断し、catalogue を書き進めずに orchestrator へ返す → spec 再収束後に type-design を再開。
- Bad: spec を編集したまま、既存の type catalogue 作業を並走で続ける (収束失効中の下流継続)。
- Bad: 上流編集の必要性を発見したが「後でまとめて直す」と記録だけ残して下流を続行する。
- Bad: 「spec は編集されたが該当 entry に影響しない」と orchestrator が自己判定して type-design を再開する (有効性判定の裁量を挟む行為)。

## Exceptions

- `impl-plan` の task ステータス遷移 (上記「即時突き返し規則」の artifact 編集に対する明示例外) のみ。意味論検証の chain 限定 (item 2) は例外ではなく判定基準そのものである。追加の例外は ADR の Reassess When に従い、実証を伴う別 ADR で検討する。

## Review Checklist

- [ ] 上流 SoT の編集後、その下流フェーズを再開する前に、当該 chain に適用される再収束要素を確認したか
- [ ] 再開 Prerequisite の検査を直上流 1 層に限定しているか (上位層の再検査を重複させない)
- [ ] 上流編集の必要性の発見時に下流作業を即時中断したか
- [ ] 収束失効時の再開例外を impl-plan の task ステータス遷移以外に拡張していないか。意味論検証の判定を当該 chain の指摘に限定し、他 chain の指摘・列挙失敗を混入させていないか
- [ ] 信号の許容値や ref-verify の対応表を本規約や下流文書に複製していないか

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本規約の原典 ADR はこの索引の「トラック・ワークフロー」節から辿る
- [knowledge/conventions/pre-track-adr-authoring.md](./pre-track-adr-authoring.md) — ADR 側の編集裁定権 (二箱分離)
- [knowledge/conventions/enforce-by-mechanism.md](./enforce-by-mechanism.md) — 将来の mechanism 昇格の判断基準
- [knowledge/conventions/workflow-ceremony-minimization.md](./workflow-ceremony-minimization.md) — 人工的状態フィールドを作らない原則
