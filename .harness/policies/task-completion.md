# Policy: Task Completion

## Purpose

トラックのタスクが「完了した」と言える条件と、その完了状態が merge に到達するまでに満たしていなければならないことを定める。機械的に強制されるのは merge の一点、しかも「全タスクが解決済みであること」だけである — 完了に伴う残り（commit_hash の記録とその review refresh）を検査するゲートはどこにも無い。したがって規律が担保する範囲は、ゲートの手前ではなく最後まで続く。

## Scope

- 適用対象: タスク状態遷移を誰が実行してよいか、完了状態が commit / push を経て merge ガードに届くための条件、ガードを回避する操作の禁止。
- 適用外:
  - バッチごとの実装 → DFP → done 遷移 → review → commit の実行順序と、commit 後の commit_hash 埋め戻し（lifecycle tail） — `.harness/workflows/track/full-cycle.md`
  - `todo` → `in_progress` 遷移の dispatch 手順 — `.harness/workflows/track/implement.md`
  - commit 時のゲート構成 — `.harness/workflows/track/commit.md`
  - 各 capability の遷移権限の有無 — `.harness/capabilities/*.md`
  - タスク状態の表現（`DonePending` / `DoneTraced` の区別と再 backfill の拒否） — `bin/sotp track transition`

## Rules

### 遷移の実行主体

タスク状態遷移は **orchestrator の専管**である。実装 capability は自分のタスクを遷移させず、完了を orchestrator に報告する。実装 capability からは review / obligation gate / CI / commit の成否が見えないため、そこで打たれた `done` は根拠を持たない。

遷移は `bin/sotp track transition` に限る。`impl-plan.json` を直接編集して状態を書き換えてはならない。

### PR finding の修正主体

PR review の actionable finding が編集を要求する場合、orchestrator は finding ごとの focused briefing を作成し、実装変更を `implementer`、review-scope の修正を `review-fix-lead` に委譲する。親コンテキストの直接編集は委譲失敗時の recovery に限り、通常の修正経路にしてはならない。

委譲先の完了報告後、orchestrator は local review を `zero_findings` まで収束させ、`commit` workflow を完了してから PR review を再実行する。タスク状態の変更や完了報告はこの修正経路を代替せず、状態遷移は引き続き orchestrator が専管する。

### アーキテクチャ変更を含むタスク

ワークスペースのレイヤ構成に触れるタスクは、完了を報告する前に `.claude/skills/architecture-customizer/SKILL.md` の Documentation 更新対象を同期する。同期対象の列挙は skill 側が所有する。

### merge ガードが検査するもの

タスク完了は merge の一点でのみ強制される。`bin/sotp pr wait-and-merge` は、PR head が指す **remote ref 上の** `track/items/<id>/impl-plan.json` を読み、全タスクが解決済み（done または skipped）であることを要求する。`bin/sotp pr push` と `bin/sotp pr review-cycle` はタスク完了を要求しない — 中間 push や PR review は未完了タスクがあっても実行できる。

ガードが見ているのは遷移だけである。commit_hash が埋め戻されていない done タスク（`DonePending`）もガードは解決済みとして通す — 埋め戻しを要求するのは merge ガードではなく full-cycle の lifecycle tail であり、その impl-plan review refresh である。この二つを同一視すると、埋め戻し漏れが merge で止まると誤って期待することになる。実際には止まらず、hash 未記録のまま merge が成立する。

### commit_hash 埋め戻しの未強制 — エスカレーション済みの未解決事項

これは「mechanism 整備の cost が benefit を上回るので規律で代替する」と整理できる状態ではない。drift が実測されているためである: merge 済み 102 トラックのうち **24 トラック** が、commit_hash を持たない done タスクを含んだまま merge されている（うち 7 トラックは全タスクが未記録、直近は 2026-07-19。2026-07-28 時点の計測）。規律だけでは保てていない、というのが観測結果であって、想定される穴ではない。

したがってこれは容認された例外ではなく、**エスカレーション済みの未解決事項**である。mechanism 昇格の再検討条件は既に満たされており、ゲートを設ける決定は commit / merge lifecycle を所有する別 ADR に委ねられている — どこで検査するか（merge ガードで `DoneTraced` を要求する、lifecycle tail の完了を commit gate で見る、等）はその ADR の設計判断であって、本書はそれを先取りしない。

決定が入るまで merge は permissive なままであり、下記の禁止事項と Review Checklist が唯一の担保になる。実測が示すとおり、それは十分な担保ではない。**この状態は「許容されている」のではなく「未解決のまま既知である」と読むこと** — 現に 24 件が通り抜けている以上、ここを規律で守れている前提で他の判断を組み立ててはならない。

### 禁止事項

- タスク状態遷移を未コミット・未 push のまま merge してはならない。ガードは remote ref を読むため、worktree だけの遷移は存在しないのと同じである
- commit_hash 埋め戻しを未コミットのまま PR / merge へ進めてはならない。埋め戻しは `impl-plan.json` を再び変更するため、full-cycle の lifecycle tail が impl-plan scope の review refresh と tail コミットを要求する（上記のとおり、この禁止事項を機械的に止めるものは無い）
- `impl-plan.json` を削除・除外・空タスク化してガードを回避してはならない。不在・取得失敗・タスク 0 件はいずれも fail-closed で BLOCKED になる
- マージ後に merge target ブランチ上でタスク状態を直接編集してコミットしてはならない（PR ワークフローそのものをバイパスする）

## Examples

- Good: 実装 → CI → DFP → orchestrator が done 遷移 → review → コミット（実装 + タスク状態）→ hash 埋め戻し → 埋め戻し diff の impl-plan review refresh → lifecycle tail コミット → PR → merge（ガードが通る）
- Bad: review を通してからタスクを done に遷移する（遷移 diff が承認済み round を stale にし、再レビューが要る）
- Bad: 実装 capability にタスク遷移をさせる
- Bad: 実装コミット後、タスク遷移を push せずに merge を試みる（ガードでブロックされる。push と PR review 自体は途中でも走らせられる）
- Bad: 最終バッチのコミット後、hash を埋め戻したまま lifecycle tail の review refresh と tail コミットを飛ばして PR へ進む（`git status` に `impl-plan.json` / `plan.md` の変更が残ったまま merge が成立し、hash が記録されないまま履歴が閉じる。実測 24 件はこの形である）
- Bad: マージ後に merge target 上で `impl-plan.json` を編集してタスクを done に変更する

## Review Checklist

- [ ] merge 前に全タスクが done/skipped になっているか
- [ ] その遷移が push 済みか（worktree だけの遷移は ref に反映されない）
- [ ] commit_hash 埋め戻しの diff が lifecycle tail としてコミット済みか。ガードは通ってしまうので、ここは機械に頼らず自分で確認する
- [ ] merge target 上での直接 `impl-plan.json` 編集が含まれていないか

## Decision Reference

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — ADR 索引。本書の原典となる ADR はこの索引から辿る
- [.harness/workflows/track/full-cycle.md](../workflows/track/full-cycle.md) — バッチ実行順序と commit_hash 埋め戻しの手順 SSoT
- [.harness/policies/track-lifecycle.md](./track-lifecycle.md) — タスク状態遷移と SSoT 維持
