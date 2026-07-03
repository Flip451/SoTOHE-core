# Branch Strategy Convention

## Purpose

各トラックの実装作業は専用のフィーチャーブランチ `track/<track-id>` で行い、設定済みの base branch への直接変更は避け、PR ベースのマージワークフローを採用する。これにより複数トラックの並列開発と base branch の安定性を両立させ、レビュー履歴と CI 結果を PR 単位で残す。過去の plan-only lane で使われた `plan/<id>` ブランチは現行の自動解決対象ではない。直接の `git merge` / `git rebase` / `git cherry-pick` / `git reset` / `git switch` はガードフックでブロックし、ブランチ操作は必ずワークフローラッパーを経由させる。

base branch / merge target / merge method の具体値はハードコードせず、`.harness/config/branch-strategy.json` と各トラックの `metadata.json#branch_strategy_snapshot` から解決する（詳細は「設定駆動モデル」節）。

## Scope

- 適用対象: `track/<id>` 実装ブランチの PR/ガード方針、トラックブランチの作成・切り替え・マージ・PR レビュー・ブランチ操作ガード、branch strategy の設定解決。`/track:*` コマンド内のブランチ関連ステップ、`cargo make track-branch-*` / `cargo make track-pr-*` / `cargo make track-switch-base` ラッパー、`bin/sotp pr ensure-pr`、ブランチガードフック。
- 適用外: トラック内のタスク状態遷移（`knowledge/conventions/track-lifecycle.md`）、コミットへの構造化メモ付与（`knowledge/conventions/git-notes.md`）、DRY ゲート（`knowledge/conventions/dry-check-workflow.md`）。

## Rules

### 設定駆動モデル

branch strategy の実値は 2 段階で解決する。どちらの経路でも、コード・ドキュメントに特定のブランチ名をリテラルで埋め込まない。

1. **グローバル設定** (`.harness/config/branch-strategy.json`): `base_branch` / `merge_target` / `merge_method` の 3 フィールドを持つ。トラックがまだ存在しない bootstrap 操作（`/track:init` によるブランチ作成、`cargo make track-branch-create`）はこのファイルを直接読む（`JsonConfigBranchStrategyAdapter`）。
2. **トラックスナップショット** (`track/items/<id>/metadata.json` の `branch_strategy_snapshot` フィールド): トラック作成時にグローバル設定から 1 回だけ複製され、以後そのトラックの生存期間中は不変。トラック作成後のブランチ操作（`cargo make track-switch-base`、PR 作成・マージ）はこのスナップショットを読む（`SnapshotBranchStrategyAdapter`）。グローバル設定を後から変更しても、既存トラックの挙動は変わらない。

両アダプタは usecase 層の `BranchStrategyPort` トレイト（`base_branch()` / `merge_target()` / `merge_method()` / `track_prefix()`）を実装する。`track_prefix()` は常に `"track/"` を返す（CN-04、ブランチ命名規則自体は設定対象外）。

### 現在のトラック解決

- `track/<id>` ブランチにいる場合: ブランチ名から対応するトラックを自動解決する（branch-bound）。
- `plan/<id>` ブランチは plan-only / activate レーンの履歴上の名称であり、現行の `libs/usecase/src/track_resolution.rs` は自動解決対象にしない。残存する live 参照は移行対象の stale guidance として扱う。
- 設定済みの base branch にいる場合: branch 由来の自動解決は行わず `NotTrackBranch` として fail-closed する。READ 系 subcommand が explicit `--track-id` / 引数を定義している場合のみ対象トラックを明示して実行でき、WRITE 系は `track/<id>` ブランチ上で実行する。
- 解決ロジックの実体は `libs/usecase/src/track_resolution.rs` の Rust 実装にある。

### ブランチの作成

- **自動**: `/track:plan <feature>` がトラック成果物作成時にブランチ `track/<track-id>` を自動作成する。作成元は設定済みの base branch（グローバル設定から解決）。
- **手動**: `cargo make track-branch-create '<id>'` で既存トラックに対してブランチを作成できる。現在のブランチが設定済みの base branch と一致しない場合は失敗する。

### ブランチの切り替え

- `cargo make track-branch-switch '<id>'` で対象トラックのブランチに切り替える。
- `cargo make track-switch-base` でアクティブなトラックの `branch_strategy_snapshot` から解決した base branch に切り替え、最新を pull する（`/track:done` が内部で使用する）。

### マージワークフロー（track/ ブランチ）

1. トラックブランチですべてのタスクを `done` に遷移し、コミットする。
2. `cargo make track-pr-push` でブランチを push する。
3. `bin/sotp pr ensure-pr` で PR を作成する。
4. `cargo make track-pr-review` で `@codex review` をトリガーし、結果をポーリング・パースする。
5. CI が通過することを確認する。
6. PR をマージする（`bin/sotp pr wait-and-merge`。マージ方式は `--method` 省略時に `branch_strategy_snapshot.merge_method` を使う）。

タスクの `done` 遷移とコミットは必ず track ブランチ上で行う。マージ後の merge target ブランチ上ではブランチガードにより状態遷移がブロックされる。

### PR ベースレビュー（Codex Cloud）

`/track:pr-review` は GitHub PR 上で Codex Cloud の `@codex review` を使った非同期レビューサイクルを実行する。

前提条件:

- Codex Cloud GitHub App がリポジトリにインストール済み
- `gh` CLI が認証済み

フロー:

1. `cargo make track-pr-push` — トラックブランチを origin に push
2. `bin/sotp pr ensure-pr` — PR を作成 or 再利用
3. `cargo make track-pr-review` — `@codex review` コメント投稿 → ポーリング → 結果パース

PR レビュー完了判定:

- Codex Cloud はレビュー完了を PR issue への 👍 reaction で通知する（コメントではない）。
- findings は `pulls/{pr}/comments` の line-level コメントとして到着する。

非同期ポーリング:

- `@codex review` コメント投稿後、GitHub API をポーリング（デフォルト: 15 秒間隔、10 分タイムアウト）。
- トリガータイムスタンプ以降のレビューのみを検出（古いレビューの誤検出を防止）。
- タイムアウト時: bot activity 有無で「GitHub App 未インストール」と「レビュー進行中」を区別。

既存ワークフローとの関係:

- `/track:review` はローカルの高速レビューループとして引き続き利用可能。
- `/track:pr-review` は PR ベースの非同期レビュー（GitHub 上でレビュー履歴が残る）。
- 両者は独立しており、用途に応じて使い分ける。

### NotStarted bypass（check-approved）

PR ベースレビューのみを使用し、ローカルレビューをスキップした場合、`review.json` は作成されない。`bin/sotp review check-approved` は以下の条件を**両方**満たすときに bypass（exit 0）を許可する:

1. `review.json` が存在しない（`review.json` が存在するが読取不能な場合は bypass しない — fail-closed）
2. 全 required scope が `NotStarted` 状態

一度でもローカルレビューが実行され `review.json` が作成されると、bypass は無効になり、全スコープの approval が必要になる。

### ガードポリシー

直接の `git merge` / `git rebase` / `git cherry-pick` / `git reset` / `git switch` はフックでブロックされる。ブランチ操作はワークフローラッパー（`cargo make track-branch-*` や `/track:*` コマンド）を経由すること。

## Examples

- Good: `/track:plan <feature>` でトラックを開始すると `track/<track-id>` ブランチが設定済みの base branch から自動作成され、すべての commit / push / PR 作成がそのブランチ上で行われる。
- Good: PR ベースレビューのみを使う運用で `bin/sotp review check-approved` が `review.json` 不在 + 全 scope `NotStarted` で exit 0 になり、commit ゲートを通過する。
- Bad: トラックの作業途中で `git switch <base-branch>` を直接実行する（ガードフックでブロックされる。`cargo make track-branch-switch` 経由で別トラックに移る）。
- Bad: PR ベースレビューを開始した後にローカルで一度でも `/track:review` を走らせ、その後「ローカル review はやらない方針なので NotStarted bypass を再活性化したい」と望む（一度 `review.json` が作られると bypass は無効化される。全 scope を approve するか、別 PR / 別 track に分割する）。

## Exceptions

- 設定済みの base branch 上での緊急 hotfix を想定する場合は別 ADR で取り扱う。本 convention 内ではガードフック越えのワークフローを定義しない。
- `bin/sotp pr wait-and-merge` や `/track:merge` の使用条件は別の workflow ドキュメント（`/track:merge` コマンド本文）で定義する。本 convention はマージ前段までを扱う。

## Review Checklist

- [ ] 新規ワークフローや CI ステップが `track/<id>` ブランチ上での実行を前提にしているか
- [ ] 直接 `git merge` / `git rebase` / `git cherry-pick` / `git reset` / `git switch` を呼ぶ案内が混入していないか
- [ ] PR ベースレビューだけを使うルートに `review.json` を作る副作用が紛れ込んでいないか
- [ ] track 解決ロジック (`libs/usecase/src/track_resolution.rs`) を変更したときに本 convention の「現在のトラック解決」節も更新されているか
- [ ] 新規コード・ドキュメントに特定のブランチ名（例: `main`）がリテラルで埋め込まれていないか（`BranchStrategyPort` 経由の解決に置き換える）

## Decision Reference

- [knowledge/adr/README.md](../adr/README.md) — ADR 索引。本 convention の原典となる ADR はこの索引から辿る
- [knowledge/conventions/track-lifecycle.md](./track-lifecycle.md) — `track/<id>` ブランチ内でのタスク状態遷移と SSoT 維持
- [knowledge/conventions/git-notes.md](./git-notes.md) — コミットへの構造化メモ
