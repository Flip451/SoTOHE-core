# Spec: Per-Worker Build Directory Isolation

## Goal

並列 Agent Teams ワーカーが同時にビルドを実行した際の `target/` ディレクトリ競合によるデッドロックを防止する。

## Scope

### In scope

- **compose.yml**: `CARGO_TARGET_DIR` を `WORKER_ID` 環境変数で分離可能にする
- **Makefile.toml**: `-exec` タスクに `WORKER_ID` パススルー機構追加
- ドキュメント: 並列ワーカー使用パターンの記載
- sccache 共有の動作確認

### Out of scope

- エフェメラルコンテナ方式（`docker compose run --rm` は既に対応済み）
- Worker orchestration の自動化（Agent Teams 側の責務）

## Constraints

- デフォルト動作（`WORKER_ID` 未設定時）は現行と同じ `/workspace/target` を使用
- sccache はワーカー間で共有（コンパイルキャッシュの効率維持）
- `target-{WORKER_ID}/` ディレクトリは `.gitignore` に追加
- CI パイプラインは単一ワーカーで実行されるため影響なし

## Acceptance Criteria

1. `WORKER_ID=w1 cargo make test-exec` と `WORKER_ID=w2 cargo make test-exec` が同時実行可能
2. デフォルト（`WORKER_ID` なし）で既存動作と変わらない
3. sccache ヒット率が分離前と同等
4. `cargo make ci` が全チェック通過

## Resolves

- TODO CON-05: コンテナリソース単一性 — ビルドロック競合
