# Spec: Feature Branch Strategy for Track Workflow

## Goal

トラック別フィーチャーブランチの導入により、並行トラック間のコード干渉を排除し、ブランチベースのトラック解決でコンテキスト競合を解消する。

## Scope

### In scope

- `metadata.json` スキーマ v3: `branch` フィールド追加とバリデーション
- ブランチ対応トラック解決（`resolve_track_dir()`）
- `sotp track branch` CLI サブコマンド（ブランチ作成・切替ラッパー）
- ガードポリシー拡張（`merge`/`rebase`/`cherry-pick`/`reset` ブロック）
- `/track:plan` 承認後のブランチ自動作成・切替
- `/track:commit` とレジストリのブランチコンテキスト対応
- Git Notes refspec の bootstrap 自動設定
- レガシートラック移行パス

### Out of scope

- Agent Teams worktree 自動管理（v2 で検討）
- リモートブランチの自動プッシュ（ユーザー手動操作を維持）
- PR の自動作成・マージ（ユーザー手動操作を維持）
- GitHub Actions ワークフローの `track/**` プッシュトリガー（オプション拡張）

## Constraints

- schema v2 との後方互換を維持（`branch` なしのレガシートラックを許容する移行期間）
- `validate_metadata_v2()` は `schema_version` 2 と 3 の両方を受け入れるデュアルリード対応
- エージェントはマージ・ブランチ削除を行わない（ユーザー手動操作）
- `checkout`/`switch` は直接許可せず、ワークフローラッパー経由のみ（`.claude/settings.json` と `Makefile.toml` に明示的なラッパー追加が前提条件）
- ブランチ命名規則: `track/<track-id>`（決定論的、トラック ID から導出）
- 「current track」と「latest track」の明確な区別:
  - **current track**: 現在のブランチ名から決定論的に解決。インタラクティブ操作（`/track:commit`、`/track:review` 等）で使用
  - **latest track**: `updated_at` タイムスタンプベースのグローバル解決。CI 検証・レポート（`verify_latest_track_files.py`）で使用。`main` ブランチ上ではアクティブトラックなし

## Acceptance Criteria

1. `/track:plan` 承認後に `track/<track-id>` ブランチが自動作成され、ワークスペースがそのブランチに切り替わる
2. `metadata.json` に `branch` フィールドが記録され、CI バリデーションで検証される
3. トラック解決が現在のブランチ名を優先し、グローバル `updated_at` に依存しない
4. `merge`/`rebase`/`cherry-pick`/`reset` がガードフックでブロックされる
5. `main` ブランチ上では暗黙の「アクティブトラック」が存在しない
6. `cargo make bootstrap` が Notes refspec を自動設定する
7. レガシートラック（`branch` なし）がタイムスタンプフォールバックで引き続き動作する
8. `cargo make ci` が全チェックを通過する

## Resolves

- WF-21 (HIGH): 暗黙のトランクベース開発と並行トラックの矛盾
- WF-04 (MEDIUM): 最新トラック推論のコンテキスト競合
- WF-05 (LOW): Git Notes のローカル制約

## Related Conventions (Required Reading)

- `project-docs/conventions/security.md`
