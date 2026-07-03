---
adr_id: 2026-07-02-1600-review-load-batch-findings-no-restatement
decisions:
  - id: D1
    user_decision_ref: "chat:session-e823e003:2026-07-03 裁定「プロンプトの修正だけで十分な価値を発揮できそう。4b(findings 全件報告)と再記述禁止の convention + reviewer policy 更新を、レビュー負荷軽減を目的とした一つの ADR にまとめる」; chat:session-6459b365:2026-07-04 裁定「D1 配置を framework-owned (review workflow briefing 規則) へ変更」"
    status: proposed
  - id: D2
    user_decision_ref: "chat:session-e823e003:2026-07-03 同裁定（再記述禁止 convention の新設）; chat:session-6459b365:2026-07-04 裁定「数値状態の相対参照 bullet を決定から削除」"
    status: proposed
  - id: D3
    user_decision_ref: "chat:session-e823e003:2026-07-03 同裁定（reviewer severity policy の更新）+ 機械 lint は「False Positive の扱いが難しいのですぐには採用しない」"
    status: proposed
---

# レビュー負荷軽減 — findings 全件報告と下流 artifact の再記述禁止

## Context

レビュー負荷の実測が対策の対象を特定している。review.json が存在する 77 track の総レビューラウンドは 9,611（平均 124.8 / track、中央値 75）。直近の track（branch-strategy-config-driven-2026-06-30）は 205 ラウンド・105 findings で、findings が出たラウンドあたりの findings 数は平均約 1.5 — reviewer は 1〜2 件ずつ報告し、fix → 再レビューの往復で直列に収束している。review prompt（`.harness/custom/review-prompts/*.md`）には報告カテゴリの定義はあるが、「該当する findings を全件列挙する」という指示はどこにもない。

findings の内容面では、最大クラスは artifact 間矛盾であり、その多くは下流 artifact が上流（ADR / spec）の挙動や設計理由を散文で言い換えた箇所に宿る。実例: impl-plan の task text が ADR の挙動を誤って言い直す、型カタログの散文が spec の CN-03 を参照すべき箇所で ADR の D5 を参照する。矛盾は言い換えの数だけ生まれる。

言い換えの一部は review policy 自身が誘発している。impl-plan の severity policy の「task description non-executable」カテゴリは「ADR / spec を読み直さずに実行できる記述（expected behaviour を書け）」を要求しており、planner はこれに応えるために spec の挙動を task text へ複写する。複写の数だけ矛盾面が増える構造である。

一方、ラウンド構造そのものの改変は不要であることも実測が示した。clean な fast ラウンドと同一 hash に対する final ラウンドは 72 件中 26 件（36%）で実 findings を捕捉しており（final の省略・確率化は品質を直撃する）、scope hash はスコープの対象 file 集合に限定した manifest で計算済みで隔離されており、同一 hash の重複再実行は 205 ラウンド中 5 回に過ぎない。したがって対策は、ラウンド構造ではなく report の密度（全件報告）と矛盾の発生源（再記述）に集中させる。

## Decision

### D1: findings の全件報告 — 1 ラウンドで該当 findings を全件列挙する

全件報告の規律は framework-owned surface に配置する: `.harness/workflows/track/review.md` の Step 3（briefing 構成規則）で、全 scope の briefing に「severity policy に該当する findings は、その round で発見した全件を列挙して報告する」旨の 1 文を含めることを義務化する。`.harness/custom/review-prompts/*.md`（利用者所有の severity policy）には挿入しない — 報告密度の規律は framework の review-process 挙動であり、利用者が独立に書き換えられる surface には置かない。severity 制約（事実誤り・矛盾・実行不能・broken reference のみを報告する既存の基準）は変更しない — 報告の基準を緩めるのではなく、基準に該当する findings の報告漏れ（最初の 1 件で打ち切る挙動）を禁止する。fix 側は報告された全件を 1 バッチで修正し、次の round で一括検証する。

狙いは 1 finding ずつの直列往復の削減である。全件化が低確度 findings の水増しに転じないよう、severity 制約の維持を同じ文で明示する。

### D2: 下流 artifact の再記述禁止 — convention の新設

impl-plan の task text / plan sections と型カタログの docs / intent は「変更対象（file / symbol）+ 操作 + spec anchor の cite」で記述し、上流（ADR / spec）の設計理由・挙動契約の再説明を書かない。この規範を `knowledge/conventions/` に convention として新設する。

- 挙動は `AC-NN` / `IN-NN` / `CN-NN` の cite で参照し、内容を言い直さない
- spec.json は対象外とする — ADR を細粒度化するのが spec の仕事であり、再記述はその本務である
- workflow ドキュメントは既存の adapter-SSoT 規則（provider 非依存 logic の重複禁止）が同族としてカバー済みであり、対象に含めない
- 既存 track artifact への遡及適用はしない（完了 track の artifact は歴史的記録）

### D3: reviewer severity policy の更新 — 実行可能性の再定義と再記述の finding 化

`.harness/custom/review-prompts/` の severity policy を 2 点更新する。

1. **実行可能性の再定義**（impl-plan）: 「task description non-executable」の判定基準を「変更対象 file / symbol + 操作 + anchor cite が揃っていれば実行可能」に書き換える。挙動の再説明を実行可能性の要件から外し、要求しない。
2. **再記述の finding 化**（impl-plan / types）: 「上流（ADR / spec）の挙動・設計理由を散文で再説明している」こと自体を finding クラスとして追加する。reviewer の仕事は散文同士の意味調停（どちらが正しいかの調査）から、再記述の存在検出 + citation の妥当性確認へ寄る。

この enforcement は毎ラウンドの review gate で効くため、書き手（planner / type-designer / orchestrator）の遵守意思に依存しない。convention 文書（D2）単独では形骸化することが全コード調査（gate 強制なしの文書ルール = 25 違反、強制あり = 0 違反）で実測されており、D2 と D3 は不可分のセットである。

## Rejected Alternatives

### A. 散文中の anchor 参照の機械 lint

impl-plan / カタログの散文に現れる anchor token（`AC-NN` / `CN-NN` / `D<n>` 等）の実在検査とレイヤー適合検査（カタログ散文の ADR 直接参照を fail にする等）を sotp の lint として実装する案。決定論的で、実例（CN-03 と書くべき箇所の D5 参照）を機械捕捉できるが、引用・例示・打ち消し文脈（「D5 ではなく CN-03 が正しい」のような記述）での false positive の扱いが難しい。今回は採用しない（Reassess When 参照）。

### B. clean fast 後の final ラウンドの省略・確率化

同一 hash に対して fast が zero findings なら final を省略（または確率的 probe に置換）する案。実測で final は clean fast と同一 hash の 36%（26/72）で実 findings を捕捉しており、省略は品質を直撃する。却下。

### C. per-scope hash 隔離の強化と同一 hash 再実行の verdict cache

スコープ外編集による hash 陳腐化の連鎖を対策する案。hash は既にスコープの対象 file 集合限定の sorted manifest で計算されており隔離済み、同一 hash の重複再実行も 205 ラウンド中 5 回（2.4%）で、単独の対策としては見合わない。却下（将来、責務中立な semantic-verdict core を実装する際に verdict cache を「ついで」で載せることは妨げない）。

### D. convention 文書のみで運用する（reviewer policy を更新しない）

D2 の規範を文書として置くだけの案。gate 強制のない文書ルールは形骸化することが実測されているため、review gate に載る D3 とのセット採用とする。却下。

## Consequences

### Positive

- findings の直列往復が減る（現行実測: findings ラウンドあたり平均約 1.5 件。全件報告で複数 findings が 1 往復に束なる）
- 矛盾 findings の発生源（言い換え散文）が縮み、artifact 間矛盾クラスの発生自体が減る
- reviewer の 1 finding あたりの調査コストが下がる（意味調停 → 再記述の存在検出）
- 実装は workflow / prompt / convention の文書変更のみで sotp のコード変更ゼロ。即日適用でき、効果がなければ即時に戻せる

### Negative

- task text の情報量が減り、implementer は anchor 先（spec.json）の参照を強制される（implementer が spec を読むのは前提であり許容）
- 全件報告により 1 ラウンドの fix バッチが大きくなる（往復回数と引き換え）
- 再記述か操作記述かの境界判定は reviewer の意味論判断に残る（機械検査は採用していないため、scope 間の判定ぶれはありうる）

### Neutral

- fast → final の 2 ラウンド構造・per-scope hash 機構は変更しない
- 既存 track artifact への遡及適用はしない

## Reassess When

- 全件報告の導入後も findings/ラウンドが約 1.5 から改善しない場合 — 直列発見（fix が次の欠陥を露出させる）が支配的で、バッチ化の効果が薄いと判断し、別の対策を検討する
- 再記述 finding の判定が scope 間でぶれ、false positive / false negative が目立つ場合 — 却下案 A（散文 anchor lint）を、打ち消し文脈等の false positive 設計込みで再検討する
- telemetry のレビュー時間比率（直近 8 track 実測で壁時計時間の 13.5〜38.6%）が導入後も下がらない場合
- impl-plan の task text を構造化フィールド（対象 / 操作 / 参照）に分解する schema 変更を検討する場合 — D2 はその前段にあたる

## Related

- `.harness/workflows/track/review.md` — D1 の変更対象（Step 3 の briefing 構成規則）
- `.harness/custom/review-prompts/` — D3 の変更対象 policy 群
- `knowledge/conventions/type-designer-kind-selection.md` — `.harness/custom/review-prompts/*` を framework methodology の enforcement source にしないという既存の境界言明（D1 の配置判断と同じ基準）
- `knowledge/conventions/enforce-by-mechanism.md` — 文書ルール単独の形骸化と gate 強制の原則（D2 + D3 セット採用の根拠）
- `knowledge/conventions/workflow-ceremony-minimization.md` — エラーを実質的に防がない ceremony の廃止原則（B / C の却下と同じ判断基準）
- `tmp/adr/2026-07-02-1345-catalogue-generation-annotation.md`（未昇格 draft。`tmp/` は `.gitignore` 対象のため本リポジトリの版管理外） — 型カタログの生成 + 注釈化 draft（カタログ散文を intent 一行に絞る方向で D2 と整合）
