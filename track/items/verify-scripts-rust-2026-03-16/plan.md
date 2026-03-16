<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# STRAT-03 Phase 5: verify script 群の Rust 移行

残存する Python verify スクリプト群 (verify_tech_stack_ready.py, verify_latest_track_files.py, verify_architecture_docs.py, verify_orchestra_guardrails.py) および check_layers.py を sotp verify サブコマンドに移行し、CI 検証パスから Python 依存を削減する。STRAT-03 Phase 5。

## 基盤: 依存追加 + ドメイン型

workspace に toml, regex クレートを追加し、ドメイン層に検証結果型を定義する。

- [x] workspace Cargo.toml に toml, regex クレート追加 + libs/infrastructure の Cargo.toml に依存追加
- [x] Domain 層に検証結果型 (VerifyOutcome, Finding, Severity) を追加 (libs/domain/src/verify.rs)

## Infrastructure 層: 検証ロジックモジュール群

verify スクリプトが依存するロジックを infrastructure 層に Rust モジュールとして実装する。
architecture_rules.py の verify_sync()、convention_docs.py の verify_index()、
settings.json 構造検証、tech-stack TODO チェック、latest track 検証、
check_layers.py の cargo metadata 解析 + レイヤー依存違反検出、
verify_architecture_docs.py のテキストパターンチェック群をカバーする。

- [x] Infra: architecture rules モジュール — architecture-rules.json パース + Cargo.toml/deny.toml 同期検証 (libs/infrastructure/src/verify/architecture_rules.rs)
- [x] Infra: arch-docs テキストパターン検証モジュール — _require_file/_require_line チェック群 (workflow gates, traceability markers, conventions references) の Rust 実装 (libs/infrastructure/src/verify/doc_patterns.rs)
- [x] Infra: convention docs インデックス検証モジュール (libs/infrastructure/src/verify/convention_docs.rs)
- [x] Infra: tech-stack readiness 検証モジュール — 未解決マーカー検出 + テンプレートdev/planning-phase バイパス (libs/infrastructure/src/verify/tech_stack.rs)
- [x] Infra: latest track files 検証モジュール — 最新トラックの spec/plan/verification 完全性チェック (libs/infrastructure/src/verify/latest_track.rs)
- [x] Infra: orchestra guardrails 検証モジュール — .claude/settings.json hook/permission/env/agent 構造チェック (libs/infrastructure/src/verify/orchestra.rs)
- [x] Infra: layers 検証モジュール — cargo metadata 解析 + workspace dependency graph 構築 + レイヤー依存違反検出 (libs/infrastructure/src/verify/layers.rs)

## CLI 層: sotp verify サブコマンド群

sotp verify サブコマンドグループに 5 つのサブコマンドを追加する。
各サブコマンドは infrastructure 層のモジュールを呼び出す。

- [x] CLI: sotp verify tech-stack サブコマンド + テスト
- [x] CLI: sotp verify latest-track サブコマンド + テスト
- [x] CLI: sotp verify arch-docs サブコマンド (T003+T004+T005 の infra モジュールを呼び出す) + テスト
- [x] CLI: sotp verify layers サブコマンド (T009 の infra モジュールを呼び出す薄い CLI ラッパー) + テスト
- [x] CLI: sotp verify orchestra サブコマンド + テスト

## 切替・クリーンアップ

Makefile.toml の verify -local タスクを Rust 版に切替え、
移行済み Python スクリプトを削除し、ドキュメントを更新する。

- [x] Makefile.toml: verify-*-local タスクを sotp verify に切替 + check-layers-local を sotp verify layers に切替
- [x] Python 削除: verify_tech_stack_ready.py, verify_latest_track_files.py, verify_architecture_docs.py, verify_orchestra_guardrails.py, check_layers.py 削除 + scripts-selftest 更新 + ドキュメント参照修正
