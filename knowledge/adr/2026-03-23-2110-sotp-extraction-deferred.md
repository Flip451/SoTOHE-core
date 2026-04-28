---
adr_id: 2026-03-23-2110-sotp-extraction-deferred
decisions:
  - id: 2026-03-23-2110-sotp-extraction-deferred_grandfathered
    status: accepted
    grandfathered: true
---
# sotp CLI 外部ツール化は Moat 後に再評価

## Status

Accepted

## Context

テンプレートの非 Rust 部分（CI, hooks, scripts, rules）が SoTOHE 自身に過学習している問題を分析（`tmp/template-overfitting-analysis-2026-03-23.md`）。過学習率は Makefile 50%、hooks 68%、scripts 73%。

根本原因は sotp CLI のソースコードとテンプレートインフラが同一リポジトリに同居していること。解決策として sotp をスタンドアロンツールに切り出し、テンプレートは sotp インストール済み環境で動作する方式を設計（`knowledge/strategy/TODO-PLAN-v4-draft.md`）。

しかし、テンプレート利用者がゼロの段階で配布インフラに投資するのは YAGNI。

## Decision

sotp CLI の物理的なリポ分割・バイナリ配布（SPLIT-03/04/05）と Phase 6（テンプレート外枠）は Moat（Phase 3）完了後に再評価する。

ただし論理的な境界の文書化（SPLIT-01）と bin/sotp パス抽象化（SPLIT-02）は低コストで将来の選択肢を閉じないため、適切なタイミングで実施可能とする。

## Rejected Alternatives

- **今すぐ物理分割**: +6 日の投資。利用者ゼロでは ROI 不明。バージョン互換性管理コスト、自己参照開発ループの断絶リスク
- **完全に棚上げ**: 分析レポートと v4 ドラフトが失われる。知見は保存すべき

## Consequences

- Good: Moat 到達に影響なし（+0 日）
- Good: 分析・設計成果物は保存済みで再利用可能
- Bad: テンプレートの Cargo workspace に sotp ソースが残り続ける
- Bad: 過学習問題の名前が変わっただけで結合度は同じ

## Reassess When

- Phase 3 完了後（Moat が実現し、配布する価値があるか判断可能に）
- テンプレート利用者が現れた場合
- sotp の変更頻度が下がり安定期に入った場合
