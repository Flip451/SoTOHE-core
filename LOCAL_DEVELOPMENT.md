# Local Development

## Quick Start

新規参加者は `START_HERE_HUMAN.md` を最初に読むこと。

## Host Requirements

- Docker + docker compose（必須）
- Python 3.11+ はオプション。ただし `guides-*` / `conventions-*` / `architecture-rules-*` などのタスクはホスト上で `python3` を直接呼ぶため、これらを使う場合は必須
- `.tool-versions` は `python 3.12.8` を pin（asdf 利用時の `python3` 解決に使われる）
- Linux で uid/gid が `1000:1000` 以外なら `export HOST_UID=$(id -u) HOST_GID=$(id -g)` を shell profile に追加する
- `verify-*` ゲートは Docker compose 経由（`sotp verify` Rust CLI）で実行する。`verify-latest-track` のみホスト直実行
- Python helper のテストは Docker 経由（`cargo make guides-selftest`, `cargo make scripts-selftest`）
- `*-local` タスクは内部実装（直接実行しない）
- Claude Code hooks（`skill-compliance`, `block-direct-git-ops`, `block-test-file-deletion`）は `bin/sotp hook dispatch ...` で dispatch される

## Compose Setup

```bash
cargo make build-tools   # tool image build
cargo make up            # dev watcher 起動
cargo make logs          # ログ
cargo make down          # 停止
```

`up/down/logs/ps` は dev overlay（`compose.dev.yml`）を使用する。`app` サービスは `bacon`-based のウォッチャーコンテナで、HTTP サーバではない。`tools` サービスは ephemeral で `docker compose run --rm ...` wrapper（`fmt`, `clippy`, `test`, `ci` 等）から起動される。Cargo cache / `target` / pytest cache は repo bind mount を使う。

### tools-daemon（反復作業の高速化）

`tools-daemon` コンテナを起動しておくと、`*-exec` 系タスク（`test-exec`, `clippy-exec`, `fmt-exec`, `check-exec`, `llvm-cov-exec`, `test-one-exec`）を `docker compose exec` 経由で実行でき、`run --rm` の起動オーバーヘッドを回避できる。

```bash
cargo make tools-up      # 起動（バックグラウンド）
cargo make tools-down    # 停止
```

`cargo make ci` / `deny` などの最終ゲートは `run --rm` のまま維持される。`verify-latest-track` はホスト直実行（Docker なし）。

## Useful Commands

`cargo make help` でカテゴリ付きタスク一覧を表示する。主要 wrapper（`bacon`, `test-exec`, `track-pr-*` 等）は表示一覧から確認する。

外部長文ガイドの運用ルール: `knowledge/external/POLICY.md`
capability-to-provider マッピング: `.harness/config/agent-profiles.json`

## Git Notes (Optional)

`git notes` で実装トレーサビリティを記録する。設定方法・remote 共有手順・運用詳細は `track/workflow.md` の「Git Notes」セクションを参照する。

## Troubleshooting

### `cargo make build-tools` が失敗する

- Docker BuildKit が有効か: `export DOCKER_BUILDKIT=1`
- Docker デーモンが起動しているか: `docker info`
- ディスク容量: `docker system df` / 不要イメージ削除 `docker system prune`

### `cargo make up` / `cargo make logs` でコンテナが起動しない

- ログ確認: `cargo make logs`
- 再作成: `cargo make down && cargo make build-tools && cargo make up`
- ボリュームリセット: `docker compose down -v`

注: `cargo make up` は `compose.dev.yml` の `app`（`bacon` ウォッチャー）を起動するもので、HTTP ポートを公開する本番サーバではない。

### sccache が効かない / ビルドが遅い

- キャッシュディレクトリの存在: `ls -la .cache/sccache` / 初期化 `rm -rf .cache/sccache`
- 複数プロジェクト間でキャッシュ共有する場合は `SCCACHE_HOST_DIR` を `~/.bashrc` / `~/.zshrc` に設定し、`mkdir -p "$SCCACHE_HOST_DIR"` してから `cargo make build-tools` する

### `cargo make ci` でテストが通らない

`cargo make test` / `clippy` / `fmt-check` / `deny` / `check-layers` を個別に実行して問題を切り分ける。

### Permission denied (scripts/)

Python スクリプトは実行権限不要。`cargo make` 経由で `python3` を呼ぶため `chmod +x` は不要。
