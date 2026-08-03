---
adr_id: "2026-07-29-0358-task-dependency-declaration-and-batch-order-check"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-07-31 merge 段階の採用裁定「すべてのdelta adrの決定を承認します。」(PR #228 の delta ADR 一括採用)"
    candidate_selection: "from:[impl-plan-schema-2-dependency-declaration,batch-plan-dependency-declaration,no-dependency-declaration,mandatory-dependency-declaration] chose:impl-plan-schema-2-dependency-declaration"
    status: accepted
  - id: D2
    user_decision_ref: "chat:2026-07-31 merge 段階の採用裁定「すべてのdelta adrの決定を承認します。」(PR #228 の delta ADR 一括採用)"
    candidate_selection: "from:[declared-dependency-batch-order-check,plan-order-monotonicity,no-ordering-check] chose:declared-dependency-batch-order-check"
    status: accepted
---
# タスク間の依存関係を impl-plan で宣言し、batch 順序をその宣言に対して検査する

## Context

`batch-plan.json` の cross-file 構造検査の一項として「タスクの依存先が同一または先行の batch に
ある」ことを検査すると決めた記録があるが、その task → task の依存関係を保持する artifact は
SoT chain 上に存在しない。`impl-plan.json`（schema 1）の task entry は id / description /
status / commit_hash のみで DTO は `deny_unknown_fields`、`batch-plan.json` にも依存宣言の
スロットはない。`plan.sections[]` は依存関係ではなく task id の順序付きグループ化だが、全タスクを
一度ずつ覆う順序（plan order）であり、次に着手するタスクの解決がこれをたどる。

検査対象と定めた関係を同一決定集合内のどの artifact も供給していない。依存関係はタスク分解の
時点でタスク列と同時に確定する入力側の情報であって終端の導出物ではなく、task の SoT である
`impl-plan.json` が宣言の置き場所として自然である。

## Decision

### D1: `impl-plan.json` を schema 2 へ拡張し、タスクが依存関係を宣言できるようにする

task entry に省略可能な `depends_on`（同一ファイル内の task id の列）を追加する。意味は「その
タスクの実装は列挙されたタスクの完了を前提とする」。省略と空列は未宣言と読む。

codec が fail-closed で拒否する file-internal 不変条件（`plan.sections[]` の参照整合性検査と同型）:

- **参照の実在**: `depends_on` の各 id が同一ファイルの `tasks[]` に存在する。
- **非循環**: 宣言された依存関係が閉路を含まない（自己参照を含む）。
- **線形拡大**: plan order が宣言された依存グラフの線形拡大である。

三つ目は宣言と実行順の食い違いを構造的に排除する — plan order は次タスク解決がたどる順序で
あるため、これが依存グラフに反すれば、前提未完了のタスクを先に配る計画を表現できてしまう。

読み取りは schema 1 / 2 の双方を受理し、書き出しは 2 とする。registry / views の描画は active で
ない track の `impl-plan.json` も decode するため、読み取り互換が必要である。schema 1 は未宣言と
して読む。gate 系の検証対象は active track に限り、既存成果物へ遡及しない。

### D2: batch 順序の構造検査を、宣言された依存関係に対する機械検査として定義する

宣言されたすべての依存辺について、依存先タスクの所属 batch が依存元タスクの所属 batch と同一で
あるか、`batches[]` の宣言順でそれより先行することを検査する。満たさない辺が一つでもあれば
fail-closed とする。入力は `impl-plan.json` の依存宣言と `batches[]` のみで、`batch-plan.json`
側に新しい宣言フィールドを要求しない。依存を宣言していないタスク対は対象外である。

宣言の網羅性は機械では検査できず、impl-plan scope の review が担う（gate は構造適合のみ）。

### 既存決定との関係

本 ADR D1 は `2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D1 のうち
「`impl-plan.json` の schema は変更しない（schema 1 のまま）」の一文を、本 ADR D2 は同 ADR D2 の
cross-file 構造検査の第 3 項を、それぞれ **refines** する。`impl-plan.json` へ持たせるのは依存
関係のみで、`batch-plan.json` の内容・同 D2 の他の内容・同 ADR D3 は変更しない。

schema 凍結の根拠 — 見積りと batch は終端導出物であり、先頭 artifact への埋め込みは書き順を
反転させる — は依存関係には及ばない。依存関係はタスク列と同時に確定するため書き順は反転せず、
batch を再編成しても書き換わらない。残る破壊的移行の負担は schema 1 の読み取り受理で解消する。

## Rejected Alternatives

- **plan order に対する batch 割当の単調性を検査する（依存を宣言しない）**: 新しい宣言を要さ
  ないが代理検査であり、plan order 自体が依存に反していれば通過する。守りたい性質の保証は
  review 判断に残る。
- **`batch-plan.json` に依存宣言を置く**: schema 移行を避けられるが、依存関係は終端導出物では
  なく配置理由と噛み合わない。宣言と plan order が別ファイルに分かれ、線形拡大を file-internal
  不変条件にできない。
- **依存を宣言せず、batch 編成の妥当性を全面的に reviewer へ委ねる**: 非整合は検査項目を削る
  ことで解消するが、強制力の梯子を一段下げる。
- **依存宣言を必須にする**: 宣言漏れと「依存なし」を書式上区別できるが、網羅性は依然機械検査
  できず（空列と宣言漏れは実質同じ）、大半のタスクに冗長なフィールドを課す。

## Consequences

- Good: 元の決定が守ろうとした性質が、宣言された範囲について機械で保証される。
- Good: 依存関係の置き場所が task SoT に一致し、タスク列と同じ一回の書き込みで確定する。
- Good: 宣言と実行順の食い違いが、検出不能な desync ではなく codec の構造的拒否になる。
- Good: 読み取り互換により既存成果物の移行が生じず、views 描画も壊れない。
- Bad: 宣言の網羅性は機械検査できず、未宣言の前提は素通りする。受容する残余リスクとし、判定は
  impl-plan review に委ねる（見積りの自己申告と同性質）。
- Bad: impl-planner の作業増と、codec の schema 2 対応・不変条件追加の一回限りの実装コスト。
- Neutral: 依存を宣言していないタスク対の batch 割当は plan order と無関係でよい。次タスク解決が
  現在の batch に属さないタスクを返す状況の噛み合わせは、本 ADR では扱わない。

## Reassess When

- 依存宣言の網羅性が低く、機械検査が実質的に空振りする状態が常態化した場合。
- 線形拡大の制約が計画作業の妨げになる事例が現れた場合。
- タスクの消化順が plan order 以外の順序で決まるようになった場合。

## Related

- `knowledge/adr/2026-07-28-1521-scope-diff-ceiling-admission-enforcement.md` D1 / D2 — refines 対象。
- `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` — `impl-plan.json` と
  schema 1 の原典。
- `knowledge/adr/2026-03-11-0040-plan-task-integrity.md` — plan と task の参照整合性を構築時に
  検証する前例。D1 の不変条件はこれと同じ位置に載る。
