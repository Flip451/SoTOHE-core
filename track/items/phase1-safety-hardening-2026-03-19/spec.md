# Spec: Phase 1 Safety Hardening

## Goal

Phase 1 の残り 2 件（GAP-05, GAP-06）を完了し、テストファイル削除ガードのバイパス防止と unsafe コードの混入防止を実現する。

## Scope

- `is_test_file` 関数にパスコンポーネント正規化を追加 [source: gemini-gap-analysis-2026-03-18 G3]
- 3 lib crate root に `#![forbid(unsafe_code)]` を追加 [source: gemini-gap-analysis-2026-03-18 G10]

## Constraints

- `is_test_file` の既存テスト（50+ 件）を壊さないこと [source: convention — source-attribution.md]
- パス正規化は `std::path::Path::components()` を使い、外部クレートを追加しない [source: inference — 標準ライブラリで十分]
- `#![forbid(unsafe_code)]` は lib crate のみ。binary crate (`apps/cli`) は対象外 [source: inference — binary crate では依存の unsafe を制御できず効果が薄い]
- vendored crate (`vendor/conch-parser`) は変更しない [source: inference — vendored code は upstream 管理]

## Acceptance Criteria

1. `../tests/foo.rs`, `./tests/foo.rs`, `foo/../../tests/bar.rs` が `is_test_file` で検出される [source: gemini-gap-analysis-2026-03-18 G3]
2. `tests/../src/main.rs` が `is_test_file` で検出されない（false positive 防止） [source: inference — 正規化により tests/ セグメントが消えるケースの検証]
3. 3 lib crate すべてに `#![forbid(unsafe_code)]` が設定されている [source: gemini-gap-analysis-2026-03-18 G10]
4. `cargo make ci` が通過する [source: convention — 07-dev-environment.md]
5. 既存テスト（1012+ 件）が全通過する [source: convention — 05-testing.md]
