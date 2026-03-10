# Local Development

## Quick Start

Read `START_HERE_HUMAN.md` first if you are new to this repository.

## Host Requirements

- Python 3.11+ is required on the host machine.
- host-side Python package management should use `uv`.
- `.tool-versions` は `python 3.12.8` を pin しており、asdf 利用時は `python3` 解決に使われる。
- `takt-*` をホストで使う場合は `uv venv .venv && uv pip install --python .venv/bin/python -r requirements-python.txt` で Python 依存を入れておく。
- `takt-*` を `.venv` で実行する時は `PYTHON_BIN=.venv/bin/python cargo make takt-run` のように interpreter を明示する。
- Docker compose 実行は `HOST_UID` / `HOST_GID` を使ってホスト user に寄せる。Linux で uid/gid が `1000:1000` 以外なら `export HOST_UID=$(id -u) HOST_GID=$(id -g)` を shell profile に入れる。
- `guides-*` / `conventions-*` / `architecture-rules-*` / `takt-failure-report` / `takt-*` は `cargo make` wrapper で内部的に `python3` を実行する。
- ホスト側の検証スクリプト（`scripts/check_layers.py`, `scripts/verify_architecture_docs.py`, `scripts/verify_track_metadata.py`, `scripts/verify_latest_track_files.py`）と `cargo make --allow-private verify-orchestra-local` は `PYTHON_BIN=/path/to/python3.12 ...` で上書きできる。
- Python test は Docker 経由で実行する（`cargo make guides-selftest`, `cargo make scripts-selftest`, `cargo make hooks-selftest`）。
- `*-local` タスクは内部専用（private）で、直接実行しない。
- Claude hooks in `.claude/hooks/` run via `python3`.

1. Build tool image:

```bash
cargo make build-tools
```

Optional (pre-build dev watcher image only):

```bash
cargo make build-dev
```

2. Start the dev watcher service:

```bash
cargo make up
```

3. Watch dev watcher logs:

```bash
cargo make logs
```

4. Stop compose services:

```bash
cargo make down
```

`up/down/logs/ps` use the dev overlay by default.
The `app` service in that overlay is a `bacon`-based watcher container for local feedback,
not a long-running HTTP server.
`tools` is intended to be ephemeral and is used via `docker compose run --rm ...` wrapper tasks (`fmt`, `clippy`, `test`, `ci`, etc.).
The runtime image built from `Dockerfile` starts the minimal `apps/server` HTTP server, while the compose.dev `app` service remains a watcher-only container.
Cargo cache / `target` / pytest cache は repo bind mount 側を使う。`target_cache` named volume は使わないため、ホストの `rust-analyzer` と compose 実行で成果物を共有できる。

### lint-on-save を有効にする（オプション）

`lint-on-save` フックは Rust ファイル編集後に rustfmt + clippy を自動実行する。
このフックは **`tools-daemon` コンテナが起動中の場合のみ動作**し、停止中は無音でスキップされる。

```bash
# 開発開始時に tools-daemon を起動（バックグラウンド）
cargo make tools-up

# 開発終了時に停止
cargo make tools-down
```

`tools-daemon` は `tools` サービスと同じイメージ・ボリュームを使用し、
`sleep infinity` で常駐する。反復作業では `cargo make test-exec` / `test-one-exec` /
`clippy-exec` / `fmt-exec` / `check-exec` / `llvm-cov-exec`
を高速化できる。
一方で `cargo make ci`、`deny`、`verify-*` などの最終ゲートは `run --rm` のまま維持される。

## Useful Commands

- List tasks: `cargo make help`
- Open tools shell: `cargo make shell`
- Show compose services: `cargo make ps`
- Start bacon watcher in dev watcher container: `cargo make bacon`
- Start headless bacon test watcher in dev watcher container: `cargo make bacon-test`
- Fast single test with daemon: `cargo make test-one-exec <test_name>`
- Fast full test suite with daemon: `cargo make test-exec`
- Fast check/clippy with daemon: `cargo make check-exec`, `cargo make clippy-exec`
- Fast coverage with daemon: `cargo make llvm-cov-exec`
- Dependency hygiene after dependency changes: `cargo make deny`, `cargo make machete`
- External guide setup flow: `cargo make guides-setup`
- External guide registry: `cargo make guides-list`
- Fetch one external guide locally: `cargo make guides-fetch <guide-id>`
- Run takt full-cycle directly: `cargo make takt-full-cycle "task summary"`
- Queue a takt task interactively: `cargo make takt-add "task summary"` then `cargo make takt-run`
- Re-render takt runtime personas after profile changes: `cargo make takt-render-personas`

外部長文ガイドの運用ルールは `docs/EXTERNAL_GUIDES.md` を参照する。
takt wrapper (`cargo make takt-*`) は active profile を使って runtime persona と host/provider を自動適用する。
profile-aware な `takt` 実行の正式導線は wrapper のみとし、direct `takt` は補助用途に限る。
`takt-*` の host 実行では `requirements-python.txt` の PyYAML が必要になる。導入は `uv` を使う。wrapper は `PYTHON_BIN` を最優先し、未指定なら `.venv/bin/python`、最後に `python3` を使う。

Queue 運用の最短手順:

1. `cargo make takt-add "task summary"`
2. `cargo make takt-run`
3. pending task に複数 profile snapshot が混在していたら、queue を整理してから再実行する

## Git Notes (Optional)

このテンプレートは `git notes` で実装トレーサビリティを記録する。
notes はデフォルトで `git fetch` / `git push` に含まれないため、
チーム開発やマシン間で共有するには以下を設定する:

```bash
# clone ごとに一度実行（fetch 時に notes を自動取得）
git config --add remote.origin.fetch "+refs/notes/*:refs/notes/*"

# notes を remote に push
git push origin "refs/notes/*"
```

notes は補助情報であり、失われてもワークフローは壊れない。
詳細は `track/workflow.md` の「Git Notes」セクションを参照。

## Troubleshooting

### `cargo make build-tools` fails

```text
Error: failed to solve: ...
```

- Docker BuildKit が有効か確認: `export DOCKER_BUILDKIT=1`
- Docker デーモンが起動しているか確認: `docker info`
- ディスク容量を確認: `docker system df`、不要イメージを削除: `docker system prune`

### `cargo make up` / `cargo make logs` でコンテナが起動しない

```text
Error response from daemon: ...
```

- ログを確認: `cargo make logs`
- コンテナを再作成: `cargo make down && cargo make build-tools && cargo make up`
- ボリュームをリセット（キャッシュ削除）: `docker compose down -v`

注:

- `cargo make up` は `compose.dev.yml` の `app` サービスを起動する
- この `app` は `bacon` ウォッチャーであり、HTTP ポートを公開する本番サーバではない

### sccache が効かない / ビルドが遅い

- sccache キャッシュディレクトリが存在するか確認: `ls -la .cache/sccache`
- キャッシュ削除で初期化: `rm -rf .cache/sccache`

#### 複数プロジェクト間でキャッシュを共有する（オプション）

デフォルトでは sccache データは bind mount でリポジトリ直下の `./.cache/sccache` に保存される。
`SCCACHE_HOST_DIR` を指定すると別のホストディレクトリをマウントし、複数プロジェクト・複数コンテナ間で
キャッシュを再利用できる。

```bash
# ~/.bashrc / ~/.zshrc などに追記して恒久設定にすることを推奨
export SCCACHE_HOST_DIR="$HOME/.cache/sccache"
export HOST_UID="$(id -u)"
export HOST_GID="$(id -g)"

# ディレクトリを作成してから compose を起動する
mkdir -p "$SCCACHE_HOST_DIR"
cargo make build-tools
```

設定後は `docker compose down -v` でボリュームを削除してもキャッシュが失われない。

### `cargo make ci` でテストが通らない

1. ローカルでテストを確認: `cargo make test`
2. clippy エラーを確認: `cargo make clippy`
3. フォーマットを確認: `cargo make fmt-check`
4. 依存関係を確認: `cargo make deny`
5. レイヤー違反を確認: `cargo make check-layers`

### Permission denied (scripts/)

Python スクリプトは実行権限不要。`cargo make` 経由で `python3` を呼び出すため、`chmod +x` は不要。
