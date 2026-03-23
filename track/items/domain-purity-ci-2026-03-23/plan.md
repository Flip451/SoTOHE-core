<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# INF-19: sotp verify domain-purity — domain layer I/O purity CI

domain 層の I/O purity を CI で検証する sotp verify domain-purity サブコマンドを新設。usecase-purity と同じ syn AST パターン。I/O ゼロ確認済みのため即 error モード。

## Phase 1: domain-purity サブコマンド + CI 統合

T001: domain_purity.rs を新設。usecase_purity.rs と同パターンで libs/domain/src/ をスキャン。
既に I/O ゼロ確認済みのため Finding::error で即投入。
CLI に DomainPurity サブコマンドを追加。
Makefile.toml に verify-domain-purity-local タスクを追加し ci-local/ci-container に組み込み。

- [x] domain_purity.rs 新設 + CLI DomainPurity サブコマンド + Makefile.toml CI 統合 + テスト
