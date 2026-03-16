<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# MEMO-15: /track:auto — Auto Mode Design Spike

MEMO-15 /track:auto の設計スパイク。実装は含まず、設計ドキュメント + domain 層型定義（enum/struct シグネチャのみ）が成果物。
6フェーズステートマシン（plan → plan-review → type-design → type-review → implement → code-review）を domain 層に AutoPhase enum として定義し、
auto-state.json（track/items/<id>/）と auto-mode-config.json（.claude/）のスキーマを設計する。

## Domain 層 — AutoPhase ステートマシン設計

6フェーズの AutoPhase enum、遷移ルール（前進・巻き戻し）、エスカレーション条件を domain 層に定義。
型定義（enum/struct シグネチャのみ）として Rust コードに落とし込む。

- [ ] Auto Mode 6-phase state machine 設計 — AutoPhase enum + 遷移ルール + 巻き戻しルールを domain 層に型定義として設計

## 状態永続化スキーマ設計

auto-state.json のスキーマ（エスカレーション状態の永続化）と auto-mode-config.json（フェーズ→capability マッピング + 運用パラメータ）を設計。
Rust 型定義も合わせて設計。

- [ ] auto-state.json スキーマ設計 — phase/round/escalation/context/artifacts の JSON Schema + 対応 Rust 型定義
- [ ] auto-mode-config.json スキーマ設計 — フェーズ→capability マッピング + max_rounds/escalation_policy パラメータ定義

## エージェント・UI 設計

各フェーズの専門エージェントブリーフィング（プロンプト設計）とエスカレーション UI（CLI + --resume フロー）を設計。

- [ ] 専門エージェントブリーフィング設計 — 6フェーズ各エージェントのプロンプトテンプレート + コンテキスト注入ルール
- [ ] エスカレーション UI 設計 — /track:auto CLI インターフェース + --resume フロー + 人間の判断注入 API

## 統合 & ドキュメント

/track:full-cycle との共存設計と、DESIGN.md への統合更新。

- [ ] /track:full-cycle との統合設計 — 既存コードとの共存パス + migration strategy
- [ ] DESIGN.md 統合更新 — 上記すべてを統合した設計ドキュメント（Canonical Blocks 含む）
