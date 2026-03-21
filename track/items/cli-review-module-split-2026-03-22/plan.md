<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# CLI review.rs module directory split (1825 lines → 4 files)

apps/cli/src/commands/review.rs (1825行) を 4ファイルのモジュールディレクトリに分割。ロジック変更なし、純粋な構造リファクタリング。

## Phase 1: モジュールディレクトリ化

T001: review.rs → review/{mod.rs, codex_local.rs, adapters.rs, tests.rs} に分割
mod.rs (~170行): clap structs, ReviewCommand enum, execute dispatch, thin run_* wrappers, execute_*
codex_local.rs (~480行): subprocess管理 — run_codex_local, spawn_codex, tee_stderr, run_codex_child, terminate, artifact management
adapters.rs (~340行): CliRecordRoundStore, CliResolveEscalationStore, CliCheckApprovedStore port trait impls
tests.rs (~735行): 全テスト（mod tests ブロック）

- [~] review.rs → review/ ディレクトリ化 + 4ファイル分割（mod.rs, codex_local.rs, adapters.rs, tests.rs）

## Phase 2: 検証

T002: cargo make ci 全通し + 全対象ファイルが 700行以下（tests.rs 除く）

- [ ] CI 全通し + 行数確認（mod.rs/codex_local.rs/adapters.rs が 700行以下）
