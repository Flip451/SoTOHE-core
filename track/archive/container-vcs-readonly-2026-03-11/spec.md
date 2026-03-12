# Spec: Container Security Hardening

## Goal

Codex `workspace-write` サンドボックスおよび `cargo make shell` 経由のコンテナ内からの直接 git 操作・機密ファイル読取を OS レベルで防止する。Claude Code フックが適用されないサブプロセス環境のセキュリティギャップを解消する。

## Scope

### In scope

- **compose.yml**: `.git` ディレクトリを read-only でマウント（`:ro` フラグ）
- **compose.yml**: `private/` と `config/secrets/` を tmpfs オーバーレイでマスク
- **compose.dev.yml**: 同様の変更を適用
- **security.md**: コンテナレベルの強制範囲をドキュメント化

### Out of scope

- ホスト側の `.claude/settings.json` deny ルール変更（既に適切）
- Codex CLI 自体のサンドボックス制御（プロバイダー側の責務）
- ネットワークレベルの制限

## Constraints

- `.git` ro マウントにより、コンテナ内からの `git add`/`commit`/`push` は `EROFS` で失敗する
- `cargo make ci` 等の CI タスクは `.git` を読み取り専用で参照するだけなので影響なし
- `cargo make fmt`/`clippy`/`test` は `.git` に書き込まないため影響なし
- tmpfs オーバーレイのサイズは最小限（1MB）で、実データを持たない

## Acceptance Criteria

1. `docker compose exec tools git add .` が EROFS エラーで失敗する
2. `docker compose exec tools ls /workspace/private/` が空ディレクトリを返す
3. `docker compose exec tools ls /workspace/config/secrets/` が空ディレクトリを返す
4. `cargo make ci` が全チェック通過する
5. `cargo make fmt`, `cargo make clippy`, `cargo make test` が正常動作する
6. `project-docs/conventions/security.md` にコンテナ強制の記述がある

## Resolves

- TODO SEC-06: Codex workspace-write モードでのフック完全バイパス
- TODO SEC-07: サブエージェント実行時の機密情報保護の崩壊
- TODO SEC-08: コンテナシェル境界のバイパス
