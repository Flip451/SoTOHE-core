---
adr_id: "2026-07-25-0538-role-layer-matrix-enforcement"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:adr-add-role-layer-matrix-enforcement:2026-07-25:enforcement-scope"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:adr-add-role-layer-matrix-enforcement:2026-07-25:value-object-gradient"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:adr-add-role-layer-matrix-enforcement:2026-07-25:no-exemption-mechanism"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:adr-add-role-layer-matrix-enforcement:2026-07-25:shipped-config-test-scope"
    status: proposed
---
# role × 層マトリクスを機構で強制し ValueObject の層勾配を是正する

## Context

`knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md` は、意味分類を semantic review へ移す一方で、role と layer の明白な不整合は機械 lint が強制する構造的不変条件に分類した。

しかし出荷 lint config で層制約 (`KindLayerConstraint`) を持つ role は 6 つにとどまっている。
`knowledge/conventions/type-designer-kind-selection.md` の R1 マトリクスで単一層に限定されている 11 行のうち、8 行は対応する機構を持たない。
`DomainEvent` / `UseCase` / `Interactor` / `Command` / `Query` / `SecondaryAdapter` / `ApplicationService` / `UseCaseFunction` がそれにあたる。

規約本文は、`✗` または単一層限定を破る role × 層の選択を draft 段階で却下すると定めている。
これは起草者の自制に依存する規律であって、ゲートではない。
規範が機構を持たない状態は、規範と機械挙動の二重化を避けるという前掲 ADR の方針そのものと食い違う。

また `ValueObject` 行の層勾配は、`knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md` が domain 配置に domain-internal inbound 参照を必須とした際に反転した。
前掲 ADR がその必須要件を補助証拠へ降格して専用 lint も撤去した後も、記号だけが反転したまま残っている。
その結果、現行のマトリクスは application boundary 側を値オブジェクトの既定の住所として宣言し続けている。

## Decision

### D1: すべての role に許可層を宣言する

R1 マトリクスで `✓` または `△` とされた層を、role ごとに `permitted_layers` として lint config に宣言する。
`✗` とされた層はこの宣言の補集合として機械的に禁止される。

対象は単一層限定の role に限らない。
複数層に置ける role も含めて全 role に宣言を置き、層制約を持たない role をゼロにする。
これにより role × 層の逸脱は、起草者の自制ではなくゲートで止まる。

### D2: `ValueObject` の層配置はどの層でも根拠を要する判断とする

`ValueObject` の domain / usecase / infrastructure をいずれも `△` とし、既定の住所を置かない。

この role は複数層に置きうる唯一の domain 語彙 role であり、層の選択は常に意味論の判断になる。
したがって、どの層へ置く場合も配置の根拠を `docs` または track のレビュー記録に残し、reviewer がそれを照合する。

`✓` と `△` の区別は機械判定しない。
本決定は根拠の記録義務とレビューの判定基準としてのみ働く。
cli / cli_driver / cli_composition は `✗` のままとし、その禁止は D1 によって機構化される。

### D3: 適用除外の仕組みを作らない

`grandfathered` に相当するフラグも、既存宣言のための段階監査レーンも新設しない。

catalogue lint の対象はアクティブ track である。
過去 track の成果物は当時の判断の歴史的記録であって、現行規則への適合対象ではない。
それらが新しい制約に照らして適合するかどうかを問う必要がないため、適用除外の機構も不要である。

### D4: 出荷 lint config の検査はデコード可能性と両ファイルの一致に限る

出荷 lint config に対する回帰検査は、production adapter を通してデコードできることと、出荷 config と preset が構造として一致することの 2 点に限る。
config の値を Rust のリテラルへ書き写して照合する assert は置かない。

lint 方針の正は config ファイルであり、検査がその内容を再記述すると正が二つに割れる。
規則集合の変更が、方針の判断ではなくテストの追従作業として現れる状態を避ける。

撤去済みの rule 種別が再登場しないことの確認も、この範囲に含めない。
enum から除かれた種別は decode の時点で拒否されるため、型が既に構築を不可能にしている。

## Rejected Alternatives

### A. 単一層限定の 8 role だけを機構化する

機構を持たない 8 行だけを埋める案。
`✗` 列の残りが規範のまま残り、同じ「規範はあるが機構がない」乖離が別の role で再発するため、採用しない。

### B. `ValueObject` を `✓ △ △` へ戻す

domain を既定の住所とし、他の domain 語彙 role と揃える案。
domain 配置の根拠記録義務が消えるため、前掲 ADR が semantic review へ移した意味分類の入力そのものが失われる。
reviewer briefing の判定カテゴリも記録された根拠の評価を前提にしており、組み替えが必要になるため、採用しない。

### C. `ValueObject` を現状の `△ ✓ △` のまま維持する

機構被覆だけを扱い、勾配には触れない案。
application boundary 側を値オブジェクトの既定の住所として宣言し続けることになり、domain concept を意味論で判定するという前掲 ADR の主基準と矛盾するため、採用しない。

### D. transport 型の内側への漏出をカタログ lint でも検査する

application boundary の内側への transport 固有型の漏出を、カタログ側にも検査規則として置く案。
usecase crate は domain にしか依存できないためコンパイル自体が成立せず、crate 依存グラフの検査も独立に走っている。
既存機構との重複にしかならないため、採用しない。

### E. `✓` と `△` の区別も機械判定する

層ごとの要根拠フラグまで lint で判定する案。
ユビキタス言語への所属や不変条件の所有といった意味分類の機械化にあたり、意味分類を review 側へ置いた前掲 ADR の分界に反するため、採用しない。

### F. 既存 track カタログに grandfather 機構を設ける

新しい制約から既存宣言を除外する機構を置く案。
track 成果物は歴史的記録であり適合対象ではないため、除外すべき対象がそもそも存在しない。
不要な適用除外の経路を残すことになるため、採用しない。

### G. 出荷 config の値をテストへ写経したまま維持する

規則の完全一致や role 一覧のリテラルをテストに残し、config を変更するたびにテストも直す案。
lint 方針の正が config とテストの二つに割れたままになる。
規則の追加や変更が、方針の判断ではなく追従作業として現れるため、採用しない。

### H. 規則の欠落を検出するための契約を新設する

config から特定の規則が消えたことを名指しで検出する仕組みを別に設ける案。
`knowledge/adr/2026-07-25-0045-drop-contractless-export-surface-assertions.md` が、この種の問題に対して新たな強制機構を設けないという境界を引いている。
その境界を越えるため、採用しない。

## Consequences

### Positive

- R1 マトリクスの `✗` 列が全面的に機構化され、role × 層の逸脱が起草者の自制ではなくゲートで止まる。
- `ValueObject` の配置がどの層でも根拠記録を要する対称な判断になり、reviewer briefing の既存カテゴリが無修正で機能する。
- lint 方針の変更が config の編集だけで完結し、テストの追従作業を伴わない。

### Negative

- role を追加または改名するたびに、lint config の `permitted_layers` も更新する必要が生じる。
- config から規則が欠落しても、それを名指しで検出する検査は残らない。
- domain に `ValueObject` を置くたびに、意味論の根拠を記述する負担が生じる。

### Neutral

- `✓` と `△` の区別は引き続き機械判定せず、根拠の記録と semantic review が担う。
- crate topology と外部観測可能な CLI 契約は変更しない。

## Reassess When

- role の追加や層構成の変更により、`permitted_layers` の維持が負担になったとき。
- `△ △ △` が、教科書どおりの domain 配置に対してまで定型文の根拠記録を強いていると判明したとき。
- 機構化した `✗` が、正当な設計を反復して誤検出するとき。
- 出荷 config からの規則の欠落が人手のレビューを通過し、実害を生んだとき。

## Related

- `knowledge/adr/2026-07-24-1001-architecture-pattern-placement-guard-realignment.md`
- `knowledge/adr/2026-07-23-0113-type-contract-pipeline-consistency.md`
- `knowledge/adr/2026-07-25-0045-drop-contractless-export-surface-assertions.md`
- `knowledge/adr/2026-06-21-1420-cli-layers-tddd-and-role-placement-lint.md`
- `knowledge/conventions/type-designer-kind-selection.md`
- `knowledge/conventions/enforce-by-mechanism.md`
- `knowledge/conventions/no-upstream-restatement.md`
- `.harness/catalogue-lint/config.json`
