# Spec: rstest 導入 — 全 crate パラメータ化テスト移行

## 概要

rstest クレートを workspace 全体に導入し、繰り返しパターンの多いテストを `#[rstest]` + `#[case]` でパラメータ化する。テストの意図を維持しつつ、ボイラープレートを削減する。

## 背景

- 現在 459 テストがあり、特に `guard/policy.rs` (120 tests) で同一構造のテストが大量に存在
- 手動でパラメータをグルーピングしている箇所もあり（例: `test_basename_strips_mixed_case_exe` で 4 つの assert_eq を 1 関数に詰めている）
- rstest はパラメータ化テスト + fixture 注入を提供し、Rust エコシステムで広く採用されている

## ゴール

- rstest を workspace dev-dependency として追加する
- 全 4 crate (domain, usecase, infrastructure, cli) の繰り返しテストパターンをパラメータ化する
- テストカバレッジを維持する（パラメータ展開で同等のテストケース数を保つ）
- tech-stack.md を実装前に更新する（CI ブロッカー回避）

## スコープ

### 対象 crate

| Crate | 主な変換対象 | テスト数 |
|-------|------------|---------|
| `libs/domain` | `guard/policy.rs` (blocked/allowed), `track_phase.rs`, `guard/parser.rs` | ~191 |
| `libs/usecase` | `track_resolution.rs`, `hook.rs`, `worktree_guard.rs` | ~104 |
| `libs/infrastructure` | `track/codec.rs`, `track/render.rs` | ~82 |
| `apps/cli` | `commands/review.rs` 等 | ~82 |

### 変換パターン

1. **#[rstest] + #[case]**: 同一構造で入力だけ異なるテスト群をパラメータ化
2. **#[fixture]**: 共通セットアップコード（`sample_track()`, `StubReader::default()` 等）を fixture 化

### 対象外

- テストロジック自体の変更（assert の追加・削除）
- テストの新規追加
- 非テストコードの変更（rstest は dev-dependency のみ）

## 制約

- rstest のパラメータ展開後のテストケース数が変換前と同等であること
- `cargo make ci` が通ること
- tech-stack.md の更新を最初のタスクとして実行すること

## 完了条件

- [ ] rstest が workspace dev-dependency として追加されている
- [ ] tech-stack.md に rstest が記載されている
- [ ] 全 4 crate で該当する繰り返しテストがパラメータ化されている
- [ ] テストケース数が変換前と同等以上
- [ ] `cargo make ci` が通る
