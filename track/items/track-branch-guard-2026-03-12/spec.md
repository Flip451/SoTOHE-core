# Spec: Track Branch Enforcement Guard

## Goal

Track 操作（コミット、タスク状態遷移、タスク追加、オーバーライド設定）が正しいブランチ上でのみ実行されることを、システムレベルで保証する。

## Background

`pr-review-cycle-2026-03-12` トラックの実装時、`track/<id>` ブランチではなく `main` ブランチ上で全実装が行われてしまった。`metadata.json` に `branch` フィールドが存在するが、検証がプロンプト依存のため実効性がなかった。

## Scope

### In Scope

1. **Rust 型レベル保証**: `TrackBranch` value object（`track/<slug>` 形式のバリデーション付き）
2. **Rust ドメインモデル**: `TrackMetadata` に `branch` フィールドを追加
3. **Rust CLI ガード**: `sotp track transition` 実行時に現在ブランチと metadata.json の branch を照合
4. **Python ガード関数**: `verify_track_branch()` — 現在ブランチと metadata.json branch の一致を検証
5. **Python 全変更パスのガード**: `_save_metadata()` にガードを集約し、`transition_task()`, `add_task()`, `set_track_override()` の全パスをカバー
6. **Python コミットガード**: `commit_from_file()` に `--track-dir` オプション追加。`/track:commit` スキルが `tmp/track-commit/track-dir.txt` を書き出し、`commit_from_file()` がそれを読んでブランチ検証。`track-dir.txt` の内容は `track/items/<id>` 形式の repo-relative パスであること、かつ `metadata.json` が存在することをバリデーション。`track-dir.txt` は成功/失敗問わず `commit-message.txt` と同時にクリーンアップ。`TRANSIENT_AUTOMATION_FILES` にも追加。`track-dir.txt` が存在しない場合（非 track コミット）はブランチベースの自動検出にフォールバック
7. **cargo make タスク**: track context と `--skip-branch-check` フラグをガード関数に渡すよう更新

### Out of Scope

- Claude Code hook レベルでの Edit/Write 操作ブロック（将来の拡張）
- ブランチの自動作成（既存の `cargo make track-branch-create` で対応済み）
- main ブランチへの直接コミット防止（push protection は別課題）
- TOCTOU 排除のための git-level ロック（下記 TOCTOU ポリシー参照）

## Constraints

- テスト時（`now` パラメータ指定時）はブランチガードをスキップ可能にする（テスト内で git branch を操作するのは不適切）
- `--skip-branch-check` フラグで CI/testing escape hatch を提供
- 既存の track 操作（`/track:plan` でのブランチ作成前の metadata.json 書き込み等）を壊さない

## Branch Guard Skip Policy

| 条件 | ガード動作 | 理由 |
|------|-----------|------|
| `branch` = null in metadata.json | スキップ | レガシートラック・計画フェーズ（ブランチ作成前）の互換性 |
| Detached HEAD（`current_git_branch()` が `"HEAD"` sentinel を返す） | 拒否 | 曖昧な状態でのトラック操作は危険。`None`（non-repo）とは区別する |
| `--skip-branch-check` フラグ | スキップ | テスト・CI escape hatch |
| `now` パラメータ指定（Python） | スキップ | テスト時の決定論的タイムスタンプモード |
| ブランチ一致 | 許可 | 正常パス |
| ブランチ不一致 | 拒否 | 本機能の主目的 |

## TOCTOU ポリシー

ブランチチェックは best-effort 前提条件であり、git-level ロックではない。チェック→操作間の競合窓（サブ秒）は受容する。脅威モデルは「誤操作防止」であり、悪意ある攻撃への防御ではない。

## Acceptance Criteria

- [ ] `sotp track transition` が間違ったブランチ上で実行された場合、明確なエラーメッセージで拒否される
- [ ] `cargo make track-commit-message` が間違ったブランチ上で実行された場合、コミットが拒否される
- [ ] `cargo make track-transition` が間違ったブランチ上で実行された場合、遷移が拒否される
- [ ] `add_task()` と `set_track_override()` も間違ったブランチで拒否される
- [ ] `branch=null` のトラックではガードがスキップされる
- [ ] Detached HEAD 時はガードが拒否する
- [ ] 正しいブランチ上では全操作が従来通り動作する
- [ ] テスト時にはガードがスキップされ、テストが壊れない
- [ ] `cargo make ci` が全チェック通過する
