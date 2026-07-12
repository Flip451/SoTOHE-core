# Task Completion Flow

## Purpose

merge 前のタスク done 遷移と commit_hash 埋め戻しの正式手順。`sotp pr wait-and-merge` のガードにより merge タイミングで仕組み的に強制される（push / review-cycle は未完了タスクを許可する — 中間 push や PR review は途中でも走らせられる）。

## Scope

- Applies to: `/track:commit` → `/track:pr` → `/track:merge` → `/track:done` のワークフロー全体。track ブランチ上での実装完了後、**merge** タイミングで検証される。
- Does not apply to: `plan/` ブランチ（計画 artifacts のみ、ガードがスキップされる）

## Rules

### 正式フロー

タスク状態遷移は **orchestrator の専管**（実装 capability は遷移しない）。`done` は
**DFP 通過後・review 前**に打つ — review が最終タスク状態を検査でき、遷移 diff が承認済み
round を stale にしないため。commit_hash だけが commit 後の埋め戻しになる。

```
1. /track:full-cycle が batch ごとに以下を回す（手動実行時も同順序を守る）:
   a. 実装（implementer capability）→ CI
   b. DFP（/track:dry-check）
   c. orchestrator が done へ遷移（commit_hash なし）
      bin/sotp track transition T001 done
      bin/sotp track transition T002 done
      ...
      （track ブランチ上では active track を自動解決する。別トラックを対象にする場合は `--track-id <id>` を追加）
   d. /track:review（最終タスク状態を含む diff をレビュー）
   e. /track:commit（実装 + タスク状態を 1 コミットに含める）
2. commit_hash 埋め戻し（lifecycle tail）
   - git log --oneline -1 でハッシュ取得
   - bin/sotp track transition T001 done --commit-hash <hash>
   - 埋め戻し diff は impl-plan scope の review refresh を経て /track:commit
3. /track:pr（push + PR 作成 + PR review。ここではタスク完了は要求されない）
4. /track:merge（ガードが remote ref の impl-plan.json で全タスク done/skipped を検証）
5. /track:done (設定済みの base branch に切替、状態更新不要)
```

### 禁止事項

- マージ後に merge_target 上でタスク状態を直接編集してコミットしてはならない（PR ワークフローをバイパスする）
- `impl-plan.json` を削除・除外してガードを回避してはならない（不在は fail-closed で BLOCKED）
- タスク状態遷移と commit_hash 埋め戻しを未コミット・未 push のまま merge してはならない（ガードは remote ref 上の impl-plan.json を読むため、worktree だけの遷移は反映されない）

### ガードの仕組み

`sotp pr wait-and-merge` がタスク完了ガードを実行する（WF-66 により merge のみで強制）。
タスク状態の実体は `impl-plan.json`（`metadata.json` は track identity のみ）:
1. branch ref の検証（危険文字は fail-closed）
2. `track/<id>` ブランチか → それ以外は BLOCKED
3. git ref 上の `track/items/<id>/impl-plan.json` を読む
   - 不在 → BLOCKED（タスク一覧を持たない track は merge bypass 経路）
   - 取得失敗 → BLOCKED
4. 全タスクが done/skipped か → 未完了なら BLOCKED（未解決タスク ID を表示）

> **Note**: `sotp pr push` と `sotp pr review-cycle` はタスク完了を要求しない。
> 中間 push や PR review は未完了タスクがあっても実行できる。

### commit_hash 埋め戻し (WF-40 解消済み)

`TaskStatus` は `DonePending`（hash なし）と `DoneTraced`（hash あり）に分離されている。
review 前の `done` 遷移は `DonePending` を作り、batch commit 後に `DoneTraced` へ backfill する:

```
bin/sotp track transition T001 done --commit-hash <hash>
```

（track ブランチ上では active track を自動解決する。別トラックを対象にする場合は `--track-id <id>` を追加）

`DoneTraced` に対する再 backfill は `InvalidTaskTransition` で拒否される（上書き防止）。

## Examples

- Good: 実装 → CI → DFP → orchestrator が done 遷移 → review → コミット（実装 + タスク状態）→ hash 埋め戻し → `/track:pr` → `/track:merge`（ガードが通る）
- Bad: review を通してからタスクを done に遷移する（遷移 diff が承認済み round を stale にし、再レビューが要る）
- Bad: implementer capability にタスク遷移をさせる（review / obligation gate / CI / commit の成否が見えない）
- Bad: 実装コミット後、タスク遷移・hash 埋め戻しを行わずに `/track:merge`（ガードでブロックされる。`/track:pr` と review-cycle 自体は途中でも走らせられる）
- Bad: マージ後に merge_target 上で impl-plan.json を編集してタスクを done に変更

## Exceptions

- `plan/` ブランチの merge はガードをスキップする（計画 artifacts はタスク完了を伴わない）

## Review Checklist

- merge 前に全タスクが done/skipped になっているか（ガードは push 済み ref の impl-plan.json を読む）
- タスク状態遷移と commit_hash 埋め戻しがコミット済みか（worktree だけの遷移は ref に反映されない）
- merge_target 上での直接 impl-plan 編集が含まれていないか

## Related Documents

- `.claude/rules/10-guardrails.md` — ガードレール全般
- `libs/usecase/src/task_completion.rs` — merge 時のタスク完了ガード（push 済み ref の impl-plan.json を読む）
- `libs/domain/src/impl_plan.rs` — `ImplPlanDocument::all_tasks_resolved()`（タスク状態の SSoT）
