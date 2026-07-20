---
adr_id: 2026-07-19-0616-two-box-decision-separation
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 入力箱 (init 刻印セット) の track 中意味不変と、expected escalation in-place 編集 lane 廃止の裁定"
    candidate_selection: "from:[current-d6-inplace-lane,input-box-immutable] chose:input-box-immutable"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 「入力となる決定の集まり」と「pipeline で新しく現れた決定の集合」を別の箱で管理する方針の持ち込みと、pipeline 産意味変更の全 delta 箱化への同意"
    candidate_selection: "from:[inplace-3-mode-guardian,two-box-separation,track-internal-amendment-doc] chose:two-box-separation"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 adr-diagnoser は Phase 1+ でも常時経由すべきとの裁定と、防御時は変更を差し戻し代案を起点へ突き返して刻印しないとの訂正、および (c) は矛盾の許容ではなく決定の修正提案であるとの訂正"
    candidate_selection: "from:[no-guardian-bypass,always-consult-classification] chose:always-consult-classification"
    status: proposed
---
# 入力決定と pipeline 産決定の二箱分離

## Context

track 運用における ADR の意味変更 lane は、入力（user が承認して track に持ち込んだ決定）とその改善（Phase 1+ pipeline で必要になった精緻化・新決定）を同一文書への in-place 編集で扱ってきた。decision-freeze ADR（`2026-07-16-2001-adr-decision-freeze.md`）D6 は expected escalation 編集を diagnoser 非経由の直接刻印とし、入力と改善の境界は ledger の diff（init 刻印 vs escalation 刻印）と front-matter refs でしか再構成できなかった。

この境界の曖昧さが、守護者判定の要否・同期/非同期裁定の別・verdict 来歴の記録条件（`2026-07-18-0340-adr2pr-baseline-diff-comment.md` D3 との衝突、PR #203 round 1 finding）といった一連の規約矛盾の根本原因であった。相談を経て、入力決定の集合と pipeline 産決定の集合を構造的に別の箱で管理する方針を裁定する。

## Decision

### D1: 入力箱は track 中意味不変

track が init 刻印した ADR 集合（入力箱）に対し、pipeline は track 中いかなる意味変更も行わない。init との byte 乖離は常に不意の変更として不一致トリアージ（fail-closed）へ送る。誤字・参照 path 等の非意味的修正 lane（kind: non-semantic-fix）のみ存続する。decision-freeze D6 の expected escalation in-place 編集 lane（diagnoser 非経由の直接 escalation 刻印）は廃止する。

### D2: pipeline 産の意味変更・新決定は全て delta 箱へ

Phase 1+ で必要になった意味変更（既存決定の精緻化を含む）と新決定は、対象 ADR の pre-merge / post-merge を問わず、すべて track-born draft ADR（`knowledge/adr/` 配下）として起草する（delta 箱）。draft は非 user 系根拠（`review_finding_ref` 等）で chain ⓪ 🟡 と評価され、strict merge gate が merge 前の user 裁定を機械的に強制する。裁定後は根拠を `user_decision_ref` へ昇格し kind: new-adr で刻印する（既存の track-born draft lane の一般化）。下流 spec の 🔴 は delta draft を cite して解消する。

### D3: adr-diagnoser は delta 入庫時に常時経由

delta draft の起草・改稿ごとに adr-diagnoser を read-only で常時経由し、入力箱の決定との関係を判定させる:

- (a) 矛盾なし（精緻化・独立な新決定）→ 入庫（🟡 のまま）
- (b) 入力決定と矛盾 + 保全代案あり → draft を差し戻し、代案を意味変更の起点へ突き返す（入庫しない、刻印しない）
- (c) 入力決定の修正（supersede / refinement）が必要と判定 → 修正対象の決定を明示的に指す draft として入庫（🟡）し、user の非同期裁定に委ねる。採用時は対象決定が superseded / refined となるため、矛盾が併存する状態は生じない — これは決定の修正提案であり、矛盾の許容ではない。差し戻し後に起点が必要性を維持して再提出した場合もこの判定で入庫しうる

判定要旨は draft 昇格時の new-adr 刻印 reason に記録する（verdict は常に存在する）。守護者は防御と分類まで、lane 選択は起点側、意味の最終裁定は user が担う。

## Rejected Alternatives

### A. 現行維持（expected escalation の in-place 直接刻印）

入力と改善の境界が ledger の diff 考古学でしか再構成できず、「精緻化か新決定か」の分岐を orchestrator が自己裁定することになる（配達人は意味を裁定しない、との緊張）。verdict 来歴の記録条件を巡る規約矛盾はこの構造の症状だった。却下。

### B. in-place 編集を維持し 3 モード守護者判定で防御

「意味変更なし」の誤判定だけが fail-open になり、user 承認済み文面が 🔵 のまま黙って変質しうる。安全性が LLM 判定に依存する。二箱分離は同じ目的を構造で達成し、判定の役割を品質（防御・分類）に限定できる。却下。

### C. delta 箱を track/items/ 配下の amendment 文書とする

ADR は track 横断資産であり、spec が track 内成果物を ADR 根拠として cite すると SoT chain の層構造が壊れる。delta も `knowledge/adr/` 配下の正規 ADR draft として置く。却下。

## Consequences

### Positive

- 入力と改善の境界がファイル単位の構造になり、merge 監査は「delta 箱を読む」だけになる
- byte 照合が track 全区間の不変条件になり、不意の変更検出が単純かつ強くなる
- 守護者の誤判定が user 承認文面を変質させる経路が構造的に消える
- 既存機構（init 刻印・track-born draft・chain ⓪ 🟡・strict merge gate・昇格刻印・不一致トリアージ）をほぼそのまま流用できる
- primary ADR の terminal diff は常に空（または非意味的修正のみ）が正常となり、来歴監査が単純化する

### Negative

- 決定の断片化: 精緻化が refine 鎖として別ファイルに積もり、現在有効な決定の把握に鎖の読解が要る
- 軽微な意味修正にも draft 1 ファイル + merge 裁定 1 件の儀式が付く

### Neutral

- escalation 刻印 kind は使われなくなる（enum 整理は後続 track の機構判断に委ねる）

## Reassess When

- refine 鎖の読解コストが実測で問題化したとき（derived effective-view / consolidation ADR の導入を検討）
- delta 箱の儀式コスト（軽微修正の draft 化）が実測で問題化したとき
- diagnoser の入庫分類の誤りが実測で問題化したとき（抜き取り監査等を検討）

## Related

- `knowledge/adr/README.md` — ADR 索引
- `2026-07-16-2001-adr-decision-freeze.md` — D5/D6/D7 の refinement 対象
- `2026-07-18-0340-adr2pr-baseline-diff-comment.md` — verdict 来歴要件との整合
- `knowledge/conventions/pre-track-adr-authoring.md` — 裁定権節の再整合対象
- `knowledge/conventions/adr.md` — front-matter / lifecycle 規約
- `knowledge/conventions/enforce-by-mechanism.md` — 判定より構造の原則
