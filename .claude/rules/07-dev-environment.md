# Development Environment (Rust)

## Toolchain

```bash
# 必須: Docker + docker compose
docker --version
docker compose version

# 任意（補助実行用）
rustup show
rustup component add rustfmt clippy
```

## Task Runner: cargo-make

`Makefile.toml` でタスクを管理する：

```bash
cargo make bootstrap    # 初回セットアップ（venv + Docker + CI 一括）
cargo make build-tools  # ツールコンテナのビルド
cargo make fmt          # rustfmt でフォーマット（compose内）
cargo make clippy       # clippy で静的解析（-D warnings）
cargo make test         # cargo nextest でテスト
cargo make ci-rust      # Rust専用CI（fmt-check + clippy + test + deny + check-layers）アプリ開発内側ループ用
cargo make ci           # 全体CI（ci-rust + scripts/hooks selftest + verify-* すべて）コミット前ゲート
cargo make check-layers     # レイヤー依存関係チェック（標準CIに含む）
cargo make verify-arch-docs # ドキュメント乖離チェック（標準CIに含む）
cargo make workspace-tree   # crate のみの workspace tree を表示
cargo make workspace-tree-full  # crate + 非 crate ディレクトリを含む workspace tree を表示
cargo make add <files>            # 手動の低レベル staging（terminal 直実行用）
cargo make add-all                # worktree 全体を stage（transient scratch file は除外）
cargo make add-pending-paths      # .takt/pending-add-paths.txt から選択的に stage
cargo make track-add-paths        # tmp/track-commit/add-paths.txt から選択的に stage
cargo make commit                 # 手動の低レベル commit（terminal 直実行用）
cargo make commit-pending-message # .takt/pending-commit-message.txt から commit
cargo make note-pending           # .takt/pending-note.md から note を適用して削除
cargo make track-commit-message   # tmp/track-commit/commit-message.txt から commit
cargo make track-note             # tmp/track-commit/note.md から note を適用して削除
cargo make track-transition       # タスク状態遷移 + ビュー自動再生成
cargo make track-sync-views       # plan.md + registry.md を metadata.json から再生成
cargo make track-pr-push          # 現在のトラックブランチを origin にプッシュ
cargo make track-pr-ensure        # PR 作成（既存なら再利用）
cargo make track-pr-merge <pr> --method <merge|squash|rebase>  # CI 待ち → マージ
cargo make track-pr-status <pr>   # PR チェック状況表示
cargo make track-pr-review        # PR レビューサイクル（push + PR作成 + @codex review）
cargo make track-switch-main      # main に切替 + pull
cargo make track-plan-branch      # plan/<id> ブランチを main から作成（計画レビュー用）
cargo make track-activate         # planning-only track を activate して track branch に切替
cargo make track-resolve          # 現在の track phase / next command / blocker を表示
cargo make track-branch-create    # main からトラックブランチを作成して切替
cargo make track-branch-switch    # 既存トラックブランチに切替
cargo make scripts-selftest       # verify / helper スクリプトの回帰テスト
cargo make hooks-selftest         # Claude hook Python セルフテスト
cargo make help                   # カテゴリ付きタスク一覧表示
cargo make shell                  # tools コンテナ内でシェルを開く
cargo make check                  # cargo check（docker compose 経由）
cargo make test-doc               # ドキュメントテスト
cargo make python-lint            # ruff lint（Python scripts / hooks）
```

### `-local` タスクについて

`Makefile.toml` には `fmt-local`, `clippy-local`, `ci-local` などの `-local` サフィックス付きタスクがある。
これらは **コンテナ内部から呼ばれる実装詳細** であり、ホストから直接呼び出してはならない。

```
# NG: ホストから直接呼ぶ
cargo make fmt-local
cargo make ci-local

# OK: compose ラッパー経由で呼ぶ（内部で -local を呼ぶ）
cargo make fmt
cargo make ci
```

**理由**: `-local` タスクはホストの Rust ツールチェーンで実行されるため、
コンテナ内の toolchain バージョンと一致しない可能性がある。
再現性を保つため、常に compose ラッパー（非 `-local` タスク）を使うこと。

## Parallel Worker Isolation (WORKER_ID)

Agent Teams で複数ワーカーが同時にビルドする場合、`WORKER_ID` 環境変数で
`CARGO_TARGET_DIR` を分離して build lock 競合を防ぐ：

```bash
# Worker ごとに固有の ID を設定
WORKER_ID=w1 cargo make test-exec   # → target-w1/
WORKER_ID=w2 cargo make clippy-exec # → target-w2/

# デフォルト（WORKER_ID 未設定）は従来通り target/ を使用
cargo make test-exec                # → target/
```

| 項目 | 動作 |
|------|------|
| `WORKER_ID` 未設定 | `CARGO_TARGET_DIR=/workspace/target`（従来互換） |
| `WORKER_ID=w1` | `CARGO_TARGET_DIR=/workspace/target-w1` |
| sccache | ワーカー間で共有（`SCCACHE_DIR` は共通） |
| 対象タスク | `-exec` サフィックス付きタスク（`test-exec`, `clippy-exec` 等） |

**注意**: `run --rm` ラッパー（`cargo make ci` 等）はホスト側で `CARGO_TARGET_DIR_RELATIVE`
環境変数を設定することで分離可能（例: `CARGO_TARGET_DIR_RELATIVE=target-w1 cargo make test`）。

## Project Bootstrap (Version Research)

プロジェクト開始時に active `researcher` capability で最新版を調査する。
既定 profile では Gemini CLI を使う：

```bash
gemini -p "Research latest stable versions as of today:
- Rust stable toolchain (version + release date)
- cargo-make, cargo-nextest, cargo-deny, cargo-machete
- crates used in Cargo.toml (name, current constraint, latest stable)
Return markdown table with: item | current | latest | recommendation.
Include source links." 2>/dev/null
```

調査結果を `.claude/docs/research/version-baseline-YYYY-MM-DD.md` に保存し、
`Cargo.toml` / `Dockerfile` / `track/tech-stack.md` に反映してから実装を開始する。

## Lint & Format

```bash
rustfmt --edition 2024 src/main.rs
cargo make fmt-check         # CI用（修正なし確認のみ）
cargo make clippy            # 標準 lint
cargo make clippy-tests      # テスト対象のみ個別確認したい時
```

### rustfmt.toml

```toml
edition = "2024"
style_edition = "2024"
max_width = 100
use_small_heuristics = "Max"
```

`rustfmt.toml` には `rustfmt --print-config default` ベースの full catalog を保持する。
`group_imports` / `imports_granularity` は stable rustfmt では warning になるため、
必要な候補値をコメントで残して AI が参照できるようにする。

## Testing

```bash
cargo make test                 # 標準テスト
cargo make test-one-exec test_name  # 特定テスト
cargo make llvm-cov             # カバレッジ
cargo make test-nocapture       # 出力表示（必要時のみ）
```

## Dependency Auditing

```bash
cargo audit          # セキュリティ脆弱性
cargo make deny      # ライセンス・禁止クレート（標準CIに含む）
cargo make machete   # 未使用依存の検出（依存変更時の補助監査）
```

## Pre-commit Checklist

- [ ] `cargo make fmt-check` passes
- [ ] `cargo make clippy` passes
- [ ] `cargo make test` passes
- [ ] `cargo make deny` passes

## Package Management

```bash
cargo add <crate>           # 依存関係追加
cargo add --dev <crate>     # dev-dependency
cargo update                # 依存関係の更新
```
