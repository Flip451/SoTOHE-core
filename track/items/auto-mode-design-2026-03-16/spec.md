# Spec: MEMO-15 /track:auto — Auto Mode Design Spike

## Feature Goal

`/track:auto` は、トラック全体のタスクリストを自律的に処理する実行モードを提供する。
人間は設計判断のエスカレーション時のみ介入し、それ以外は6フェーズのサイクルが自動的に回る。

本トラックは**設計スパイク**であり、動作する実装は含まない。
成果物は設計ドキュメント + domain 層の型定義（enum/struct シグネチャのみ、メソッド body は設計ドキュメントに擬似コードとして記載）。

## Scope

### In Scope

- 6フェーズステートマシンの設計（plan → plan-review → type-design → type-review → implement → code-review）
- `auto-state.json` スキーマ設計（`track/items/<id>/auto-state.json` に配置）
- `.claude/auto-mode-config.json` スキーマ設計（フェーズ→capability マッピング + 運用パラメータ）
- 専門エージェントブリーフィング設計（6フェーズ×プロンプトテンプレート）
- エスカレーション UI 設計（`/track:auto` CLI + `--resume` フロー）
- `/track:full-cycle` との統合・移行パス設計
- domain 層 `AutoPhase` enum + 関連型の型定義（シグネチャのみ）
- DESIGN.md への設計決定記録

### Out of Scope

- 動作する実装（usecase 層、infrastructure 層、CLI 実行ロジック）
- テストコード（型定義のコンパイル確認のみ）
- `agent-profiles.json` の実際の変更
- Makefile.toml の変更
- CI/CD パイプラインの変更

## Constraints

- domain 層の型定義は既存の `TrackMetadata`, `TrackTask`, `TaskTransition` 等と整合させる
- `auto-mode-config.json` のフェーズ→capability マッピングは `agent-profiles.json` の capability 名を参照する（provider 解決は agent-profiles に委譲）
- `auto-state.json` は `track/items/<id>/` に配置する（エフェメラルなセッション状態のため git 追跡しない。`.gitignore` に追加する）
- 6フェーズの巻き戻しルールは reviewer の findings severity に基づく
- エスカレーション時は状態を永続化してプロセスを停止し、`--resume` で再開できる設計とする
- 型定義は enum/struct のフィールドとメソッドシグネチャのみとし、メソッド body は設計ドキュメント内に擬似コードとして記載する。`todo!()` や `unimplemented!()` は本番コードに含めない

## Acceptance Criteria

- [ ] `AutoPhase` enum が domain 層に定義されている（6フェーズ + Escalated + Committed）
- [ ] フェーズ遷移ルール（前進・巻き戻し・エスカレーション）がドキュメントと Mermaid 図で記述されている
- [ ] `auto-state.json` の JSON Schema が定義されている
- [ ] `auto-mode-config.json` の JSON Schema が定義されている
- [ ] 専門エージェントブリーフィングのプロンプト構造が設計されている
- [ ] `/track:auto` CLI インターフェース（引数・オプション）が設計されている
- [ ] `--resume` による再開フローが設計されている
- [ ] `/track:full-cycle` との共存・移行パスが設計されている
- [ ] DESIGN.md に Auto Mode セクションが追加されている
- [ ] 型定義（enum/struct）がコンパイルエラーを起こさない
