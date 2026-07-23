---
adr_id: "2026-07-23-0113-type-contract-pipeline-consistency"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-22"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session_67282137-412e-4cc0-b687-505dd036e653:2026-07-23"
    status: proposed
---
# 型契約パイプラインの規範と機構を実挙動に整合させる

## Context

テンプレート利用プロジェクト（mini-repomix）の実走で、型契約パイプラインに 3 つの乖離が観測された。

1. **R1 の配置勾配が境界ラッパーを domain へ誤誘導する。**
`type-designer-kind-selection.md` R1 の ValueObject 行は domain = ✓（デフォルト）/ usecase = △（要根拠）で、domain 配置だけが挙証責任を負わない。
実走では `InputDirectory` / `OutputDestination`（不変条件なし・domain 内の型/関数から一切参照されないアプリケーション境界の値ラッパー）が R1 適合のまま domain に置かれた。
対照的に `SplitLimit` は domain 内で 3 箇所から参照されており、正当な domain VO である。
規則に適合したまま配置が歪む勾配が、規約の側にある。

2. **type-designer 仕様と生成器の実挙動が空層で乖離する。**
capability 仕様 12a（全層 d2 必須）に対し、生成器は公開アイテム 0 の層で d2 を出力しない。
mini-repomix の Phase 2 はこの乖離でブロックし、orchestrator 裁定（空層は d1 存在 + exit 0 で 12a 充足）で resume した（2026-07-22）。
裁定は妥当だが人間レーン依存のままで、次の空層プロジェクトで再発する。

3. **contract-map の style 設定に role の欠落があり、silent に無スタイル描画される。**
`primary_adapter` / `composition_root` の classDef が定義されておらず、該当 role のノード（PackDriver / MiniRepomixComposition 相当）だけが既定スタイルで描画された。
警告は出ない。

## Decision

### D1: R1 ValueObject 行に domain 配置の使用要件を追加する

ValueObject の domain 配置には「当該 track catalogue 内で、domain 層の他エントリ（型・trait・関数）のシグネチャから参照されること」を要件として追記する。
この要件を満たさない境界値ラッパーは usecase 配置（Dto、または Command の構成要素）を既定とする。
勾配を対称化し、domain 配置にも根拠（domain 内消費）を要求する。

### D2: D1 を catalogue lint として機構化する

catalogue lint に「domain 配置の ValueObject で domain 層内の inbound 参照がゼロ」の検出を追加する（cross-entry 検査の前例は `FieldElementUniqueAcrossEntries` にある）。
判定は contract-map renderer が既に構築している参照グラフと同じ情報源から行える。

違反は `action` の種別（add / reference / modify）を問わずエラーとし、grandfather の警告レーンは設けない。
baseline に眠る誤配置は、どの track も宣言しない限り誰も妨げない。
いずれかの track が catalogue で接触した時点で返済義務が発生し、返済の出口は 2 つ——当該型を usecase へ移設するか、正当な domain 消費者を同 track で宣言して D1 の要件を満たすか——である。
散文根拠による免除レーンは置かない（機構化の趣旨が崩れるため）。
宣言を避けることによる迂回は R7 の declare 義務と網羅性ゲートの管轄であり、本ルールでは扱わない。
baseline 全体を一斉修正する big-bang を避け、接触を契機に配置を単調に清算していく方針である。

### D3: 空層の扱いを仕様側に明文化し、裁定を機構化する

公開アイテム 0 の層は「d1 存在 + exit 0 で 12a 充足、d2 は生成されない」を正とする。
type-designer capability 仕様と生成器のドキュメント・実装の記述を一致させ、空層での人間裁定レーンを廃する。
（2026-07-22 の orchestrator 裁定の登記と恒久化。）

### D4: contract-map style に全 role の classDef を必須化し、未定義 role を警告にする

style 設定（`contract-map-style.toml`）は DataRole / ContractRole / FunctionRole の全値に対応する classDef を持つことを必須とする。
renderer は classDef 未定義の role を検出した場合、silent に既定描画へフォールバックせず警告を出す。

## Rejected Alternatives

### A. R1 は現行のまま運用・レビューでカバーする

実走でまさに破れた。
確率的なレビューより決定的な検査が上位（enforce-by-mechanism）。

### B. 空層でも d2 を常に生成する側に倒す

消費者のいない成果物の生成を義務化することになり、「整合的な不在」を認める既存の設計判断と矛盾する。

### C. renderer 側に未知 role の hardcode fallback を足す

欠落が恒久的に見えなくなる。
だんまり通過の温存であり、警告が正しい。

### D. baseline 由来の reference 宣言は警告に留める（恒久 grandfather）

期限のない警告は儀式化してノイズになり、負債の返済が永遠にスケジュールされない。
「🟡 は track 終端までに解消する」という既存の信号運用の設計思想とも整合しない。
接触時の強制返済（D2）なら、眠っている負債は誰も妨げず、接触のたびに配置が単調に清算されていく。

## Consequences

### Positive

- 境界ラッパーの domain 混入が宣言時点で構造的に止まる。
- 空層プロジェクトで Phase 2 が人間裁定なしに通る。
- role 追加時に style 欠落が即座に発覚する。

### Negative

- 誤配置型に接触した track は、本来の作業に加えて移設（または domain 消費者の宣言）を負う。膨張は接触時の 1 回限りで、以後の track は清算済みの配置を受け取る。
- 検査・仕様改訂・style 補完の実装コスト。

## Reassess When

- usecase 境界型（Command / Report）が domain 型を公開面に出す問題を扱うとき（本 ADR のスコープ外。別 ADR で公開面規律として検討）
- ValueObject 以外の role でも同種の配置誤誘導が観測されたとき
- 接触時の強制返済が track のスコープを不釣り合いに膨らませる事例が続いたとき（免除レーンの導入を再検討）
- role 集合が拡張されたとき（D4 の必須検査が自然に検出する）

## Related

- `knowledge/conventions/type-designer-kind-selection.md` — R1 / R3 / R6
- `.harness/capabilities/type-designer.md` — 12a
- `.harness/config/contract-map-style.toml`
- `knowledge/conventions/enforce-by-mechanism.md`
