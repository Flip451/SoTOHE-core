# Policy: Branch Strategy

## Purpose

各トラックの実装作業は専用のフィーチャーブランチ `track/<track-id>` で行い、設定済みの base branch への直接変更は避け、PR ベースのマージワークフローを採用する。これにより複数トラックの並列開発と base branch の安定性を両立させ、レビュー履歴と CI 結果を PR 単位で残す。過去の plan-only lane で使われた `plan/<id>` ブランチは現行の自動解決対象ではない。直接の `git merge` / `git rebase` / `git cherry-pick` / `git reset` / `git switch` はガードフックでブロックし、ブランチ操作は `/track:*` または `bin/sotp track branch` を経由させる。

base branch / merge target / merge method の具体値はハードコードせず、`.harness/config/branch-strategy.json` と各トラックの `metadata.json#branch_strategy_snapshot` から解決する（詳細は「設定駆動モデル」節）。

## Scope

- 適用対象: `track/<id>` 実装ブランチの命名とガード方針、トラックブランチの作成・切り替え、現在のトラック解決、branch strategy の設定解決。`bin/sotp track branch` / `bin/sotp track switch-base`、ブランチガードフック。
- 適用外: push / PR 作成 / PR レビュー / マージの手順（それぞれの workflow SSoT が所有 — 「push / PR / マージ手順の所在」節）、トラック内のタスク状態遷移（`.harness/policies/track-lifecycle.md`）、コミットへの構造化メモ付与（`.harness/policies/git-notes.md`）、DRY ゲート。

## Rules

### 設定駆動モデル

branch strategy の実値は 2 段階で解決する。どちらの経路でも、コード・ドキュメントに特定のブランチ名をリテラルで埋め込まない。

1. **グローバル設定** (`.harness/config/branch-strategy.json`): `base_branch` / `merge_target` / `merge_method` の 3 フィールドを持つ。トラックがまだ存在しない bootstrap 操作（`/track:init` によるブランチ作成、`bin/sotp track branch create`）はこのファイルを直接読む（`JsonConfigBranchStrategyAdapter`）。
2. **トラックスナップショット** (`track/items/<id>/metadata.json` の `branch_strategy_snapshot` フィールド): トラック作成時にグローバル設定から 1 回だけ複製され、以後そのトラックの生存期間中は不変。トラック作成後のブランチ操作（`bin/sotp track switch-base`、PR 作成・マージ）はこのスナップショットを読む（`SnapshotBranchStrategyAdapter`）。グローバル設定を後から変更しても、既存トラックの挙動は変わらない。

両アダプタは usecase 層の `BranchStrategyPort` トレイト（`base_branch()` / `merge_target()` / `merge_method()` / `track_prefix()`）を実装する。`track_prefix()` は常に `"track/"` を返す。ブランチ命名規則自体は設定対象外である。

### 現在のトラック解決

- `track/<id>` ブランチにいる場合: ブランチ名から対応するトラックを自動解決する（branch-bound）。
- `plan/<id>` ブランチは plan-only / activate レーンの履歴上の名称であり、現行の track 解決はこれを自動解決対象にしない。残存する live 参照は移行対象の stale guidance として扱う。
- 設定済みの base branch にいる場合: branch 由来の自動解決は行わず `NotTrackBranch` として fail-closed する。READ 系 subcommand が explicit `--track-id` / 引数を定義している場合のみ対象トラックを明示して実行でき、WRITE 系は `track/<id>` ブランチ上で実行する。
- 解決ロジックの実体は `bin/sotp` 側の実装にあり、本書はその外形的な挙動だけを述べる。

### ブランチの作成

- **自動**: `/track:plan <feature>` がトラック成果物作成時にブランチ `track/<track-id>` を自動作成する。作成元は設定済みの base branch（グローバル設定から解決）。
- **手動**: `bin/sotp track branch create <id>` で既存トラックに対してブランチを作成できる。現在のブランチが設定済みの base branch と一致しない場合は失敗する。

### ブランチの切り替え

- `bin/sotp track branch switch <id>` で対象トラックのブランチに切り替える。
- `bin/sotp track switch-base` でアクティブなトラックの `branch_strategy_snapshot` から解決した base branch に切り替え、そのあと ff-only sync で最新取り込みを試みる（`/track:done` が内部で使用する）。
- `bin/sotp git sync` は現在のブランチを ff-only pull するのみの guarded operation。ブランチ切り替えは行わないので、track branch 上で upstream に fast-forward 追従する用途としても使える。

### push / PR / マージ手順の所在

push・PR 作成・PR レビュー・マージの手順は本書の対象外で、それぞれの workflow SSoT が単独で所有する — `.harness/workflows/track/pr-review.md`（push / PR 作成 / レビューサイクル）と `.harness/workflows/track/merge.md`（マージ、タスク完了ガード、マージ実行時の method 指定）。本書が定めるのは merge method の設定値がどこから解決されるかまでで、いつ誰がそれを使うかは merge workflow の側にある。ローカルレビューとその approval 条件は `.harness/workflows/track/review.md` が所有する。手順を本書に複製すると、どちらが正かを読み手が判断できない二重管理になるため、ここでは所在だけを示す。

本書が所有するのは、これらの操作をどのブランチ上で行えるかという制約だけで、それは「現在のトラック解決」節に書いてある。

### ガードポリシー

直接の `git merge` / `git rebase` / `git cherry-pick` / `git reset` / `git switch` はフックでブロックされる。ブランチ操作は `/track:*` または `bin/sotp track branch` を経由すること。

## Examples

- Good: `/track:plan <feature>` でトラックを開始すると `track/<track-id>` ブランチが設定済みの base branch から自動作成され、すべての commit / push / PR 作成がそのブランチ上で行われる。
- Bad: トラックの作業途中で `git switch <base-branch>` を直接実行する（ガードフックでブロックされる。`bin/sotp track branch switch` 経由で別トラックに移る）。

## Exceptions

- 設定済みの base branch 上での緊急 hotfix を想定する場合は別 ADR で取り扱う。本書ではガードフック越えのワークフローを定義しない。

## Review Checklist

- [ ] 新規ワークフローや CI ステップが `track/<id>` ブランチ上での実行を前提にしているか
- [ ] 直接 `git merge` / `git rebase` / `git cherry-pick` / `git reset` / `git switch` を呼ぶ案内が混入していないか
- [ ] track 解決ロジックを変更したときに本書の「現在のトラック解決」節も更新されているか
- [ ] 新規コード・ドキュメントに特定のブランチ名（例: `main`）がリテラルで埋め込まれていないか（`BranchStrategyPort` 経由の解決に置き換える）

## Decision Reference

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — ADR 索引。本書の原典となる ADR はこの索引から辿る
- [.harness/policies/track-lifecycle.md](./track-lifecycle.md) — `track/<id>` ブランチ内でのタスク状態遷移と SSoT 維持
- [.harness/policies/git-notes.md](./git-notes.md) — コミットへの構造化メモ
