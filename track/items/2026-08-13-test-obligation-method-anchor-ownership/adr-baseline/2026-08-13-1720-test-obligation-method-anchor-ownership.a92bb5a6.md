---
adr_id: "2026-08-13-1720-test-obligation-method-anchor-ownership"
decisions:
  - id: D1
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14; deadlock-workaround-adjudication:2026-08-03; phase0-approval:claude-session-011cG8wY5jmTVx5S6K8nNzYC:2026-08-14"
    review_finding_ref: "local-review:adr:fast:2026-08-13T17:55:41Z — D1/D2 entry-level anchor coverage gap; local-review:adr:fast:2026-08-13T18:04:56Z — D1 single-method ownership contradiction; local-review:adr:fast:2026-08-13T18:11:27Z — D1 single-method partial-declaration coverage gap; adr-diagnoser:phase0:d075eafd — preserve undeclared-method no-anchor rule"
    candidate_selection: "from:[A,B] chose:A"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14; phase0-approval:claude-session-011cG8wY5jmTVx5S6K8nNzYC:2026-08-14"
    review_finding_ref: "local-review:adr:fast:2026-08-13T17:55:41Z — D1/D2 entry-level anchor coverage gap"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:claude-session-01498BG434ep3fe1BuyqfDtc:2026-08-14; phase0-approval:claude-session-011cG8wY5jmTVx5S6K8nNzYC:2026-08-14"
    candidate_selection: "from:[C,D] chose:C"
    status: proposed
---
# trait_method 義務の anchor 所有権を method 単位にする（テスト義務ゲート ADR の refinement）

## Context

3 method（validate / explain / enter）を持つ ApplicationService trait で、テスト義務を正しく結び付けてもゲートを通過できない問題が見つかった。

1. 全 method のテストを各義務に付けると evaluate は通るが、無関係な method のテストまで含むためレビューで棄却される。
2. 各義務に自 method のテストだけを付けると、他 method の約束まで検証するよう求められ、evaluate で棄却される。
3. 型と anchor の edge に自発的な結び付けを足すと、派生義務が所有する edge には置けないため構造検証で棄却される。
4. edge を免除しようとしても、本質的に検証できない理由がないため免除検証で棄却される。

原因は義務と anchor の粒度が違うことにある。義務は method ごとだが、anchor は entry-level spec_refs にしか書けず、全 method の義務へ一律にコピーされる。そのため各義務が trait 全体の anchor を背負い、自 method だけでは履行できない。MethodDeclaration には method ごとの spec_refs を書く欄もない。

一方、コピーを止めるだけでは、どの method にも割り当てられない anchor が検証から漏れる。そこで D1 は、entry の全 anchor を method に漏れなく割り当てる。

現在は単一 method の trait にも entry-level spec_refs をコピーする。D1 では未宣言の method は anchor を持たないため、entry に anchor がある単一 method trait は、その全てを method に明記しなければならない。自動コピーはしない。問題が見つかった作業では暫定的に構成 1 を使っており、D3 でこの逸脱を取り除く。

## Decision

### D1: MethodDeclaration に method-level spec_refs を導入し、trait_method 義務の anchor 所有権を method 単位にする

- MethodDeclaration に、仕様要素への構造化参照の配列 `spec_refs` を追加する。schema 上は任意だが、以下の検証規則によって必須になる場合がある。entry-level spec_refs は全 anchor の総目録、各 method の spec_refs はその中の担当分とする。
- trait_method 義務は、その method の spec_refs だけを所有する。entry-level spec_refs は trait 全体と仕様を結ぶために使い、義務へはコピーしない。
- method-level spec_refs は entry-level spec_refs の一部でなければならず、全 method の分を合わせると entry-level spec_refs の全てを覆わなければならない。同じ anchor を複数 method が担当してもよい。この条件を満たさない catalogue は構造検証で棄却する。単一 method で entry に anchor がある場合は、その全てを method に明記する。
- spec_refs を書かなかった method は anchor を持たない。複数 method では他 method が全 anchor を担当していれば省略でき、単一 method では entry-level spec_refs が空の場合だけ省略できる。anchor を持たない義務には既存の自発的な結び付け・免除の規則を使う。
- schema 追加は後方互換とし、既存 catalogue には遡って適用しない。評価対象のアクティブな catalogue だけが上記の検証を受ける。

### D2: fulfillment の検証範囲を「義務が所有する anchor」に限定する

D1 の割り当て検証を通過した後、fulfillment と指示書は、その義務が所有する anchor だけを扱う。他の anchor は、それを担当する method 義務が検証する。全 anchor に担当義務があるため、検証漏れを防ぎながら、各 method に自 method のテストだけを結び付けられる。これで Context の 4 構成が行き止まりになる問題を解消する。

### D3: 問題が見つかった trait を再 grounding し、逸脱記録を撤去する

- 3 method の ApplicationService で、各 method を担当 anchor へ再 grounding し、cross-populated fulfillment を method-scoped に戻す。
- 暫定受容の逸脱記録（Known Accepted Deviations）を撤去する。機構変更だけでなく、ここまでを本 refinement の完了条件とする。

## Rejected Alternatives

- **B: entry 単位判定の正式化** — 現在の暫定受容形（cross-populated fulfillment）を仕様として追認する案。schema 変更が不要で最小だが、「method の義務に無関係な method のテストを引用する」形が恒久化し、fulfillment の method-relevance に対するレビュー指摘と構造的に衝突し続ける。義務の粒度（method）と検証の粒度（entry）の不一致という根本原因を温存するため却下。
- **method-level spec_refs の coverage を検証しない** — method ごとの所有権だけを導入して entry-level anchor の未割り当てを許す案。宣言を省略した anchor がどの義務の検証対象にもならず、trait-level の約束を fulfillment 検証から黙って除外できるため却下。
- **entry-level spec_refs が空の単一 method にも method-level spec_refs の明示を必須化** — 所有すべき anchor がなく、未宣言義務が anchor を持たない規則だけで状態が一意に定まる。空配列の冗長な宣言を要求する理由がないため却下。
- **単一 method の未宣言・部分宣言を entry-level spec_refs で自動補完** — coverage は満たせるが、method-level spec_refs 未宣言の義務は anchor を持たないという所有規則を置換し、部分宣言では明示した集合と実際の所有集合も乖離する。entry-level spec_refs が空でない場合は全量の明示を要求することで解決するため却下。
- **D: 再 grounding と逸脱撤去の後続分離** — track を軽く保てるが、逸脱記録の残存期間が延び、暫定形が前例として参照されるリスクが続くため却下（D3 に統合）。
- **義務導出の廃止（trait_method 義務をやめ型単位義務に統一)** — 粒度不一致は消えるが、複数 method trait の履行検証が型単位の粗い判定に退化し、method ごとの約束の取りこぼしを検出できなくなる。ゲートの存在意義を損なうため却下。

## Consequences

- 良: 複数 method の ApplicationService でも、義務ごとに検証対象が一意に定まり、method-scoped fulfillment が evaluate とレビューの両方を通過できる。今後の全 track が同じデッドロックを踏まない。
- 良: entry-level anchor は必ず一つ以上の method 義務に所有されるため、trait-level の grounding を維持したまま fulfillment 検証だけを迂回する構成を構造検証で防げる。
- 良: 指示書の粒度が method 単位まで具体化可能になる（引用の粒度は指示書の粒度の上限、という既存の関係の改善側）。
- 負: catalogue schema・義務導出・coverage 検証・fulfillment 検証・型設計者向け規約（method への spec_refs 記載の指針）の同時変更が必要で、変更面は中規模。type-designer は複数 method の entry が持つ全 anchor の所有先に加え、entry-level spec_refs が空でない単一 method trait でも同じ全 anchor を method に明示する必要がある。
- 負: entry-level spec_refs が空でない単一 method trait の未宣言・部分宣言は構造検証を通過しない。アクティブ track の catalogue は必要に応じて全量宣言へ更新する必要がある。
- 中立: entry-level spec_refs が空の単一 method trait と anchor を持たない義務の挙動は不変。既存 catalogue への遡及はしない。

## Reassess When

- method-level spec_refs の宣言が形骸化する（全 method に entry と同じ refs を機械的にコピーする運用が広がる）とき — 宣言の意味が失われるため、lint での重複検出または導出規則の再設計を検討する。
- 単一 method trait で entry-level spec_refs 全体を method-level に重複宣言する負担や不整合が継続的に発生するとき — 未宣言義務の所有規則を暗黙に変えず、schema 上の明示的な継承表現を新たな決定として検討する。
- entry-level anchor が複数 method にまたがる不可分な約束を表し、個別 method 義務への所有割り当てでは自然に検証できない事例が現れたとき — 未所有のまま迂回させず、aggregate 義務の導入を検討する。
- 義務の粒度が method より細かい単位（引数条件・エラー variant 等）へ拡張されるとき。
- catalogue schema の MethodDeclaration 構造が別理由で再設計されるとき。
