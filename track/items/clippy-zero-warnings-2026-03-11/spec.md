# Spec: Clippy Zero Warnings

## Goal

clippy の warning を常にゼロに保つ。散在する `#![deny(...)]` を一元管理し、workspace 全体の lint ポリシーを `Cargo.toml` + `clippy.toml` レベルで明文化する。

## Scope

- vendored `conch-parser` の warning 抑制（`#![allow(warnings)]`）
- workspace root に `[workspace.lints]` セクション追加（clippy deny lints + rust warnings deny）
- 各 workspace member に `[lints] workspace = true` 追加
- 4つの crate root（`main.rs`, `lib.rs` x3）から重複 `#![deny(...)]` ブロックを除去
- `clippy.toml` を workspace root に新規作成（msrv, threshold 設定）
- CI パス確認

## Current State (Before)

4つの crate root に同一の `#![deny(...)]` ブロックが重複：

```rust
#![deny(
    clippy::indexing_slicing,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable,
    clippy::todo,
    clippy::unimplemented
)]
```

ローカル例外（維持する）：
- `libs/infrastructure/tests/concurrency.rs`: `#![allow(clippy::indexing_slicing, unwrap_used, expect_used, panic)]`
- `libs/infrastructure/tests/schema_compat.rs`: 同上
- `libs/domain/src/guard/policy.rs`: `#![allow(clippy::indexing_slicing)]`
- `libs/domain/src/guard/parser.rs`: `#![allow(clippy::indexing_slicing)]`

## Out of Scope

- vendored crate のコード修正（Rust 2015 → 2024 edition 移行）
- 新しい clippy lint ルールの追加（現行の 7 つの deny lint を維持）
- `Makefile.toml` の clippy フラグ変更

## Constraints

- vendored crate は外部ライブラリのため、最小限の変更に留める
- 既存の CI パイプライン（`cargo make ci`）が壊れないこと
- `-D warnings` フラグは `Makefile.toml` に残す（`[workspace.lints]` との二重ガード）
- モジュール単位の `#![allow(...)]` 例外は引き続きローカルに残す

## Acceptance Criteria

1. `cargo make clippy` の出力に warning が 0 件
2. `cargo make ci` が全てパス
3. `[workspace.lints]` が root `Cargo.toml` に存在し、全 member が継承している
4. crate root ファイル（4箇所）から `#![deny(clippy::...)]` が除去されている
5. `clippy.toml` が workspace root に存在し、`msrv` と threshold が設定されている
6. vendored crate の `lib.rs` に `#![allow(warnings)]` がある
