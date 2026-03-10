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
