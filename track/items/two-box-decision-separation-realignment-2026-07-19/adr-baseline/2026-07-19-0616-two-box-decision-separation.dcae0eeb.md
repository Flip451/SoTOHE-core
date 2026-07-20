---
adr_id: 2026-07-19-0616-two-box-decision-separation
decisions:
  - id: D1
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 入力箱 (init 刻印セット) の track 中意味不変と、expected escalation in-place 編集 lane 廃止の裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 review-refinement 新設・escalation 全面引退・経過措置の裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 簡潔な決定形式への書き戻しの裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 裁定境界の承認（収束文面 D1-D6 の承認）"
    status: proposed
  - id: D2
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 は同席 in-place 編集・delta 箱は Phase 1+ のみとする境界型の裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 review-refinement 新設・escalation 全面引退・経過措置の裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 簡潔な決定形式への書き戻しの裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 裁定境界の承認（収束文面 D1-D6 の承認）"
    candidate_selection: "from:[freeze-from-init,phase0-inplace-then-freeze] chose:phase0-inplace-then-freeze; from:[no-guardian-bypass,always-consult-classification] chose:always-consult-classification"
    status: proposed
  - id: D3
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 「入力となる決定の集まり」と「pipeline で新しく現れた決定の集合」を別の箱で管理する方針の持ち込みと、pipeline 産意味変更の全 delta 箱化への同意 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 と Phase 1+ の決定分離指示 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 簡潔な決定形式への書き戻しの裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 裁定境界の承認（収束文面 D1-D6 の承認）"
    candidate_selection: "from:[inplace-3-mode-guardian,two-box-separation,track-internal-amendment-doc] chose:two-box-separation"
    status: proposed
  - id: D4
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 adr-diagnoser は Phase 1+ でも常時経由すべきとの裁定と、防御時は変更を差し戻し代案を起点へ突き返して刻印しないとの訂正、および (c) は矛盾の許容ではなく決定の修正提案であるとの訂正 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 簡潔な決定形式への書き戻しの裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 裁定境界の承認（収束文面 D1-D6 の承認）"
    candidate_selection: "from:[no-guardian-bypass,always-consult-classification] chose:always-consult-classification"
    status: proposed
  - id: D5
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 「入力となる決定の集まり」と「pipeline で新しく現れた決定の集合」を別の箱で管理する方針の持ち込みと、pipeline 産意味変更の全 delta 箱化への同意 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 adr-diagnoser は Phase 1+ でも常時経由すべきとの裁定と、防御時は変更を差し戻し代案を起点へ突き返して刻印しないとの訂正、および (c) は矛盾の許容ではなく決定の修正提案であるとの訂正 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 簡潔な決定形式への書き戻しの裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 裁定境界の承認（収束文面 D1-D6 の承認）"
    status: proposed
  - id: D6
    user_decision_ref: "chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-19 入力箱 (init 刻印セット) の track 中意味不変と、expected escalation in-place 編集 lane 廃止の裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 review-refinement 新設・escalation 全面引退・経過措置の裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 簡潔な決定形式への書き戻しの裁定 / chat_segment:session-01GzQfyJu34bkRqJ7tWxRJtC:2026-07-20 Phase 0 裁定境界の承認（収束文面 D1-D6 の承認）"
    status: proposed
---
# 入力決定と pipeline 産決定の二箱分離

## Context

track 運用では、持ち込まれた入力決定と pipeline で生じた改善を同じ ADR で扱ってきた。このため、両者の境界と意味の裁定者が履歴に埋もれていた。入力決定と pipeline 産決定を別の箱に分け、境界と承認経路を構造で明確にする。

## 用語

- **刻印 / ledger**: ADR の特定時点の全文と来歴を ADR-baseline 台帳へ記録する仕組み。
- **入力箱**: track が init 刻印した ADR の集合であり、Phase 0 裁定境界後は意味を固定する。
- **delta 箱**: Phase 1+ で生じた意味変更や新決定を保持する track-born ADR draft の集合。
- **Phase 0 裁定境界**: user が収束文面を承認し、境界刻印と ADR-baseline commit が完了した時点。

## Decision

### D1: ゲート照合

入力箱 ADR の byte 照合は、その ADR の ledger 最新記録を kind にかかわらず基準とし、ゲートで fail-closed に行う。review 入口の検査は既存決定どおり台帳健全性（init 記録の存在を含む）に限り、byte 照合を行わない。乖離の検出を単純で決定的に保つためである。

### D2: Phase 0 裁定境界

Phase 0 の baseline review では入力箱 ADR を in-place で収束させ、各編集を adr-diagnoser が監査し、意味を orchestrator に自己裁定させない。収束 diff は user が裁定し、承認文面を review-refinement として刻印してから commit で境界を閉じる（新 kind の実装までは、既存 escalation kind と reason 冒頭の明記を唯一の経過例外とする）。user 同席区間では同期承認が最も安価で確実なためである。

### D3: Phase 1+ の freeze と delta 箱

Phase 0 裁定境界後は入力箱 ADR への意味的な in-place 編集を禁止し、精緻化を含む意味変更と新決定をすべて track-born draft として delta 箱に起草する。draft は非 user 系根拠で 🟡 として strict merge gate で user 裁定まで merge を止め、旧 expected-escalation lane を廃止して D2 の経過例外を除く escalation kind を新規用途から引退させる。入力と改善の境界を構造で保証するためである。

### D4: delta 入庫判定

delta draft は、入庫の前に必ず adr-diagnoser が判定する。判定は三択である。記録済み決定を変えないなら、そのまま入庫する。決定を変えずに済む別の解決があるなら、その解決を添えて差し戻す。決定の修正が避けられない場合だけ、決定修正の提案として入庫し、user の裁定を待つ。判定に迷う場合は差し戻す。入庫後に draft を書き換えた場合は、判定からやり直す。守護者が防御し、本当に必要な修正提案だけを user に届けるためである。

### D5: delta draft の採用と棄却

入庫済みの draft は、user の裁定を待つ間も下流成果物が cite して作業を進められる。ただし draft が正式な決定になるのは user が明示的に採用したときだけであり、採用まで merge は通らない。supersede / refine の関係は採用された delta 側だけに記録し、入力箱 ADR は書き換えない。このため、決定の現在の内容は、元の決定に採用済みの修正を採用順に重ねて読むことで得る。棄却された draft は撤回し、そこから導出された下流成果物を merge 前に再作業する。凍結を保ったまま、pipeline を止めずに決定の進化を表現するためである。

### D6: terminal 監査

merge 前の terminal 監査では、全 protected source の track 中の変更来歴を user に提示する。記録の意味裁定は user に属し、誤分類と裁定された記録は復元で回復する。残余リスクを隠蔽不能にするためである。

## Rejected Alternatives

### A. 現行維持（expected escalation の in-place 直接刻印）

入力と改善の境界が履歴に埋もれ、精緻化か新決定かを orchestrator が自己裁定する。「配達人は意味を裁定しない」という原則に反するため却下する。

### B. in-place 編集を維持し 3 モード守護者判定で防御

「意味変更なし」の誤判定だけが fail-open となり、user 承認済み文面が黙って変質しうる。判定依存の in-place lane を境界後も主経路にする案は却下する。

### C. delta 箱を track/items/ 配下の amendment 文書とする

ADR は track 横断資産である。track 内成果物を ADR 根拠にすると SoT Chain の層構造が崩れるため、delta は `knowledge/adr/` 配下の正規 ADR draft とする。

## Consequences

### Positive

- Phase 0 は user 同席の in-place 収束を維持し、不要な delta 儀式を避けられる。
- Phase 1+ は入力と pipeline 産改善をファイル単位で分離できる。
- user の明示的採用なしに意味変更を恒久化できず、残余リスクを terminal 監査で扱える。

### Negative

- refinement が別ファイルに積み重なり、現在有効な決定の把握に chain の読解が要る。
- Phase 1+ の軽微な意味修正にも draft と merge 裁定のコストが生じる。

### Neutral

- historical escalation は有効な来歴として残るが、新しい escalation は経過例外を除いて生じない。

## Reassess When

- refine 鎖の読解コストが実測で問題化したとき（derived effective-view / consolidation ADR の導入を検討）
- Phase 1+ の delta 箱の儀式コスト（軽微修正の draft 化）が実測で問題化したとき
- diagnoser の入庫分類の誤りが実測で問題化したとき（抜き取り監査等を検討）

## Related

- `knowledge/adr/README.md` — ADR 索引
- `2026-07-16-2001-adr-decision-freeze.md` — D4/D5/D6 の refinement 対象
- `2026-07-18-0340-adr2pr-baseline-diff-comment.md` — D1 の primary 単独 terminal comment を D6 で全 protected source へ refinement、D3 の verdict 来歴要件との整合
- `knowledge/conventions/pre-track-adr-authoring.md` — 裁定権節の再整合対象
- `knowledge/conventions/adr.md` — front-matter / lifecycle 規約
- `knowledge/conventions/enforce-by-mechanism.md` — 判定より構造の原則
