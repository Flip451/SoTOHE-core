---
adr_id: "2026-08-13-1720-test-obligation-method-anchor-ownership"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14; deadlock-workaround-adjudication:2026-08-03"
    candidate_selection: "from:[A,B] chose:A"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14"
    candidate_selection: "from:[C,D] chose:C"
    status: proposed
---
# trait_method 義務の anchor 所有権を method 単位にする（テスト義務ゲート ADR の refinement）

## Context

複数の method を持つ ApplicationService trait に対して、テスト義務ゲートを通過できる結び付け構成が存在しない事例が確認された。3 method（validate / explain / enter）を持つ trait で、次の 4 構成がすべて別々の機構で棄却された。

1. 全 method のテストの合併集合を各 method の義務に引用する構成は、evaluate を通過するが、レビューが「fulfillment が method-relevant でない」として指摘を返す。
2. 各 method の義務に自 method のテストだけを引用する構成は、evaluate の意味論検証が「この method 単独では証明不能な、他 method に属する約束の検証」まで要求して Fail する。
3. 型と引用先を結ぶ edge への自発的結び付けの追加は、派生義務が所有する edge には置けないという構造検証で棄却される。
4. 当該 edge への免除は、「この edge が本質的に検証不能である自己完結の理由」を書けないため免除検証で棄却される。

根本原因は、テスト義務ゲートの元 ADR における粒度の未決定である。元 ADR は「ApplicationService には trait_method ごとに義務を生成する」（義務の粒度 = method）と「anchor grounding は entry-level spec_refs で十分」（引用の粒度 = 型全体）を同時に定めており、義務導出は entry の全 spec_refs を各 method 義務へコピーする。その結果、各 method の義務が「trait 全体が約束する全 anchor」を検証対象として背負い、method 単位の履行が構造的に成立しない。catalogue schema の MethodDeclaration が spec_refs を持てないため、per-method の grounding を宣言する手段も存在しない。

なお単一 method の trait では義務へ anchor が付与されない経路のため、この問題は顕在化しない。発生 track では暫定措置として構成 1 を受容し、逸脱として記録している（本 ADR の D3 が撤去する）。

## Decision

### D1: MethodDeclaration に method-level spec_refs を導入し、trait_method 義務の anchor 所有権を method 単位にする

- catalogue schema の MethodDeclaration に、任意フィールドとして `spec_refs`（仕様要素への構造化参照の配列）を追加する。
- 義務導出を変更する: trait_method 義務が所有する anchor は、**当該 method の spec_refs のみ**とする。entry-level の spec_refs は trait 全体の grounding（型 → 仕様リンクの存在信号）に限定し、method 義務へコピーしない。
- method が spec_refs を宣言しない場合、その method の義務は anchor を持たない（単一 method trait の既存挙動と同型）。anchor を持たない義務の edge 解消は、既存の自発的結び付け・免除の規則に従う。
- 既存 catalogue への遡及はしない。評価はアクティブ track のみが対象であり、本フィールドは追加的（optional）である。

### D2: fulfillment の検証範囲を「義務が所有する anchor」に限定する

fulfillment の意味論検証（および指示書の生成）は、義務が所有する anchor の約束だけを検証対象とする。他 method の anchor に属する約束は、その method の義務が独立に検証する。これにより method-scoped fulfillment（各 method の義務に自 method のテストを結び付ける構成）が正規に成立し、Context の構成 1〜4 のデッドロックは構造的に消滅する。

### D3: 発生 track の再 grounding と逸脱記録の撤去までを本 track の完了条件に含める

- 発生 trait（3 method の ApplicationService）の各 method を対応する anchor へ再 grounding し、cross-populated fulfillment を method-scoped に戻す。
- 暫定受容の逸脱記録（Known Accepted Deviations）を撤去する。
- 逸脱の回収までが本 refinement の完了である（機構変更のみで終端しない）。

## Rejected Alternatives

- **B: entry 単位判定の正式化** — 現在の暫定受容形（cross-populated fulfillment）を仕様として追認する案。schema 変更が不要で最小だが、「method の義務に無関係な method のテストを引用する」形が恒久化し、fulfillment の method-relevance に対するレビュー指摘と構造的に衝突し続ける。義務の粒度（method）と検証の粒度（entry）の不一致という根本原因を温存するため却下。
- **D: 再 grounding と逸脱撤去の後続分離** — track を軽く保てるが、逸脱記録の残存期間が延び、暫定形が前例として参照されるリスクが続くため却下（D3 に統合）。
- **義務導出の廃止（trait_method 義務をやめ型単位義務に統一)** — 粒度不一致は消えるが、複数 method trait の履行検証が型単位の粗い判定に退化し、method ごとの約束の取りこぼしを検出できなくなる。ゲートの存在意義を損なうため却下。

## Consequences

- 良: 複数 method の ApplicationService でも、義務ごとに検証対象が一意に定まり、method-scoped fulfillment が evaluate とレビューの両方を通過できる。今後の全 track が同じデッドロックを踏まない。
- 良: 指示書の粒度が method 単位まで具体化可能になる（引用の粒度は指示書の粒度の上限、という既存の関係の改善側）。
- 負: catalogue schema・義務導出・fulfillment 検証・型設計者向け規約（method への spec_refs 記載の指針）の同時変更が必要で、変更面は中規模。type-designer の宣言負担が method 分だけ増える（任意フィールドのため、単純な trait では従来どおり省略可能）。
- 中立: 単一 method trait・anchor 無し義務の既存挙動は不変。既存 catalogue の再宣言は不要（追加的変更）。

## Reassess When

- method-level spec_refs の宣言が形骸化する（全 method に entry と同じ refs を機械的にコピーする運用が広がる）とき — 宣言の意味が失われるため、lint での重複検出または導出規則の再設計を検討する。
- 義務の粒度が method より細かい単位（引数条件・エラー variant 等）へ拡張されるとき。
- catalogue schema の MethodDeclaration 構造が別理由で再設計されるとき。
