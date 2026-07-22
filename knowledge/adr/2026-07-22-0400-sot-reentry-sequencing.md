---
adr_id: "2026-07-22-0400-sot-reentry-sequencing"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session_01ESUACDZiuzbJG2RrG83Foa:2026-07-22"
    status: proposed
---
# SoT 再入の順次処理規律 — ルーティング後のフェーズ収束 Prerequisite

## Context

SoT Chain（ADR → spec.json → 型カタログ → impl-plan → 実装）の back-and-forth は、現状すべて reactive に構成されている: 信号 🔴 / review finding / PreReviewGate Blocked を検知してから rollback-diagnoser がルーティングし、上流フェーズへ部分再入する。一方で、編集の前提条件が明文化されているのは ADR のみ（二箱分離 model と adr-editor の rollback-safety 前提）であり、spec / 型カタログ / impl-plan / 実装には「直上流がどの状態なら編集を開始してよいか」の規定がなかった。その結果、上流を編集したまま下流作業を並走させる・上流の未収束を抱えたまま下流へ降りる、といった運用が構造的に禁止されていなかった。

どの SoT へ回帰するかのルーティングは rollback-diagnoser の既存責務である。本 ADR はその後段 — ルーティングされた上流から下流へ降りる際の順次処理 — を規律として固定する。

## Decision

### D1: ルーティングと順次処理強制の責務分離

回帰先の判定は rollback-diagnoser（勧告。orchestrator の override 余地も既存どおり）が担い、本規律はルーティング後の降下順序の強制を担う。Prerequisite の充足確認と降下順序の遵守は dispatch する orchestrator の責務とし、各 writer capability は自分の Prerequisite が満たされていない briefing を受けた場合、作業せず orchestrator へ差し戻す。

### D2: フェーズ収束の 3 要素定義

フェーズ X が「収束している」とは、次のすべてが成立していること:

1. **参照信号**: X の成果物を引用側（下流）とする chain の参照信号 — 参照の存在を機械評価した 🔵 / 🟡 / 🔴 — が、`.harness/config/signal-gates.json` の当該 chain × gate 指定（interim / strict）を満たす。許容水準の SSoT は同設定ファイルであり、本規律は値を再記述しない。
2. **意味論検証**: `bin/sotp ref-verify` の該当 scope が通過している（参照先の内容と引用側の記述の意味的整合。参照信号とは独立の検証で、scope と chain の対応は ref-verify 側の定義に従う）。
3. **レビュー**: 該当 SoT スコープの review が `zero_findings` で完了している。

### D3: 再開 Prerequisite は直上流 1 層のみ検査

| 再開フェーズ（writer） | 必要な収束 |
|---|---|
| spec-design（spec-designer） | ADR の収束（`adr_user` chain） |
| type-design（type-designer） | spec の収束（`spec_adr` chain） |
| impl-plan（impl-planner） | カタログの収束（`catalog_spec` chain） |
| 実装（implementer） | カタログの収束 **かつ** impl-plan スコープ review 収束（D5 例外あり） |

各フェーズは直上流 1 層のみを検査する。上流の上流の収束は直上流の収束が推移的に保証する（SoT Chain の layer skip 禁止と同型）。

### D4: 即時突き返し規則

下流作業中に収束済み上流 SoT への編集の必要性が発見された時点で、下流作業を中断して上流へ戻る（ルーティングが自明でなければ diagnose 経由）。「後でまとめて直す」「変更の有無を判定して続行する」は禁止する — 収束の有効性判定という裁量を挟まない。上流 SoT への編集は適用された時点で当該フェーズの収束を即座に失効させ、再収束（D2 の 3 要素）まで下流フェーズの再開を禁止する。

### D5: impl-plan の task ステータス遷移例外

`impl-plan.json` のみ、review 収束後も `bin/sotp track transition` による task ステータス遷移を許容し、これは収束を失効させない。それ以外の impl-plan 変更は D4 に従い失効・再収束を要する。この例外を他の SoT へ一般化しない。

### D6: プロンプトレベル規律として導入

本規律は convention 文書と各 writer capability 文書への追記で運用する。gate / CI / `.harness/config/signal-gates.json` / `adr_user` 評価は変更しない。機械 enforcement への昇格（enforce-by-mechanism）は、判定材料がすべて永続化された機械可読の証跡（signal JSON / ref-verify 結果 / review verdict）であるため将来可能だが、本 ADR の範囲外とする。

## Rejected Alternatives

### A. 収束の失効を「その後変更されていない限り有効」とする猶予規則

却下理由: 有効性判定の主体が曖昧になり、orchestrator の自己判断が gate の代わりを務めてしまう。のちのち収束させるための編集がどのみち発生するのだから、発見時点で即座に突き返す方が判定不要で強い。impl-plan のステータス遷移だけを明示例外とする（D4 / D5）。

### B. 全 chain で 🟡 を一律許容する再入 Prerequisite

却下理由: 許容水準を本規律に再記述すると設定変更で stale になる。interim / strict の指定は `.harness/config/signal-gates.json` を SSoT とし、本規律は参照のみ行う（D2）。

### C. 最初から gate / CI として機械 enforcement する

却下理由: まずプロンプトレベルの規律として運用し、実効性を確認してから昇格を検討する。判定式が binary であるため昇格の道は閉じていない（D6）。

## Consequences

### Positive

- 「spec を直したまま types を触り続ける」類の並走が規律上禁止され、上流編集後の下流の不整合持ち越しが構造的に減る
- Prerequisite の判定材料がすべて機械可読の証跡であり、将来の gate 化の道が開いている
- 直上流 1 層のみの検査で規律が SoT Chain の一方向性と同型になり、理解コストが低い

### Negative

- 上流の再収束（信号 + ref-verify + review）が済むまで下流が完全に止まるため、back-and-forth の 1 往復あたりのレイテンシは増える
- プロンプトレベルの規律であるため、orchestrator が遵守しない場合の機械的な検出はない

## Reassess When

- プロンプトレベル運用で違反が観測され、機械 enforcement（gate 化）が必要になったとき
- `signal-gates.json` の chain / gate 構成が変わり、D2 の参照構造が実態と合わなくなったとき
- impl-plan 以外の SoT に「収束を失効させない変更種別」が実証されたとき（D5 の一般化検討）
- ref-verify の scope 解決方式が変わり、D2-2 の対応関係の記述が実態と乖離したとき

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR 側の編集裁定権（二箱分離）
- `knowledge/conventions/workflow-ceremony-minimization.md` — 人工的状態フィールドを作らない原則
- `.harness/capabilities/rollback-diagnoser.md` — ルーティング責務の SSoT
