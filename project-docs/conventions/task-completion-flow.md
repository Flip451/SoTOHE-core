# Task Completion Flow

## Purpose

PR push 前のタスク done 遷移と commit_hash 埋め戻しの正式手順。`sotp pr push` のガードにより仕組みで強制される。

## Scope

- Applies to: `/track:commit` → `/track:pr` → `/track:merge` → `/track:done` のワークフロー全体。track ブランチ上での実装完了後に適用。
- Does not apply to: `plan/` ブランチ（計画 artifacts のみ、ガードがスキップされる）

## Rules

### 正式フロー

```
1. /track:implement → /track:review → /track:commit (実装コミット)
2. 全タスクを done に遷移（commit_hash: None で OK）
   cargo make track-transition track/items/<id> T001 done
   cargo make track-transition track/items/<id> T002 done
   ...
3. /track:commit (タスク状態遷移をコミット)
4. commit_hash 埋め戻し（任意だが推奨）
   - git log --oneline -1 でハッシュ取得
   - cargo make track-transition track/items/<id> T001 done --commit-hash <hash>
   - /track:commit (埋め戻しコミット)
5. /track:pr (ガードが全タスク done/skipped を検証)
6. /track:merge
7. /track:done (main に切替、状態更新不要)
```

### 禁止事項

- マージ後に main 上でタスク状態を直接編集してコミットしてはならない（PR ワークフローをバイパスする）
- `sotp pr push` のガードを回避するために metadata.json を削除してはならない（fail-closed で検出される）
- dirty な metadata.json（未コミットの変更あり）のまま push してはならない（dirty check で検出される）

### ガードの仕組み

`sotp pr push` は以下を順にチェックする:
1. `plan/` ブランチ → スキップ
2. metadata.json が存在するか → なければ BLOCKED（fail-closed）
3. metadata.json が dirty でないか → dirty なら BLOCKED
4. metadata.json が untracked でないか → untracked なら BLOCKED
5. 全タスクが done/skipped か → 未完了なら BLOCKED（タスク ID と状態を表示）

### commit_hash 埋め戻し (WF-40 解消済み)

`TaskStatus` は `DonePending`（hash なし）と `DoneTraced`（hash あり）に分離されている。
`track-transition` で `DonePending` → `DoneTraced` への backfill が可能:

```
cargo make track-transition track/items/<id> T001 done --commit-hash <hash>
```

`DoneTraced` に対する再 backfill は `InvalidTaskTransition` で拒否される（上書き防止）。

## Examples

- Good: 実装コミット後、全タスク done → コミット → `/track:pr`
- Bad: 実装コミット後、直接 `/track:pr`（ガードでブロックされる）
- Bad: マージ後に main 上で metadata.json を編集してタスクを done に変更

## Exceptions

- `plan/` ブランチからの push はガードをスキップする（計画 artifacts はタスク完了を伴わない）
- commit_hash の埋め戻しは任意（`track-transition done --commit-hash` で実行可能）

## Review Checklist

- `/track:pr` 前に全タスクが done/skipped になっているか
- metadata.json がコミット済みか（dirty/untracked でないか）
- main 上での直接 metadata 編集が含まれていないか

## Related Documents

- `.claude/rules/10-guardrails.md` — ガードレール全般
- `apps/cli/src/commands/pr.rs` — `check_task_completion_guard()` 実装
- `libs/domain/src/track.rs` — `all_tasks_resolved()` メソッド
- `tmp/TODO.md` — WF-40（解消済み: DonePending/DoneTraced split）
