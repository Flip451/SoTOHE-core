# Observations — spec-ref-embedded-hash-removal-2026-06-11

手動観測ログ（machine-non-verifiable）。

## レーン間調整（2026-06-11）

- ベースラインコミット時、ワークツリーにあった agent-profiles.json の codex 切替（rfl/dfl）が live 設定依存の dry.rs wrapper テスト 2 件を壊して full CI をブロックした。テストを fixture ベース（`new_with_provider`）に修正。他レーンはこのテスト追加（2b6bd285, 2026-06-10）以前に分岐しているため、本ブランチを先にマージすれば main は壊れない。マージ順は本トラック → codex 切替レーンが安全。
- 型シグナル評価器の既知制限: `From<serde_json::Error>` / `From<std::io::Error>` が identity key `From<Error>` に縮退して衝突。単一の `From<serde_json::Error>` 宣言（action: modify）で偽陽性 Red を回避。評価器側の将来修正候補: `build_impl_identity_map` の generic 引数を full path で識別。

## 移行方針（lazy migration）

- schema_version 3→4 は本トラックのカタログのみ移行。他トラックの v3 カタログは現行ゲートが読まないため残置。読まれた時点で `UnsupportedSchemaVersion { actual: 3, expected: 4 }` で fail-closed（IN-08 の明示挙動）。

## ループ運用の観測

- RFP 修正後の `cargo make fmt` 漏れで infrastructure スコープが stale 化し、再レビュー 1 周（fmt は fixer 完了直後に必ず実行すべき）。
- Codex reviewer が一度 "model at capacity" で abort。リトライ 1 回で回復（ガードレールのリトライ規定どおり）。
- 旧 bin/sotp（T001 前ビルド）は v4 カタログを読めないため、T001 直後に `cargo make build-sotp` が必須（バイナリ・スキーマ遷移は前トラックの Makefile 遷移と同型）。
