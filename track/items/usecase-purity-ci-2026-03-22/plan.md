<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# INF-15: sotp verify usecase-purity — usecase layer hexagonal purity CI lint

usecase 層のヘキサゴナル純粋性を CI で検証する sotp verify usecase-purity サブコマンドを新設。sotp verify module-size / domain-strings と同じパターン。

## Phase 1: サブコマンド実装

T001: libs/infrastructure/src/verify/usecase_purity.rs を新設
禁止パターン: std::fs::, chrono::Utc::now, println!, eprintln!, std::process::Command
スキャン対象: libs/usecase/src/ 配下の .rs ファイル（#[cfg(test)] ブロック除外）
出力: Finding::warning（module-size / domain-strings と同じ VerifyOutcome 形式）

- [x] sotp verify usecase-purity サブコマンド実装 — libs/usecase/src/ の非テストコードをスキャンし、禁止パターン（std::fs::, chrono::Utc::now, println!, eprintln!, std::process::Command）を検出。warning-only

## Phase 2: CI 統合

T002: Makefile.toml に verify-usecase-purity タスク追加、ci-local に組み込み

- [x] cargo make ci に usecase-purity を組み込み + CI 全通し確認
