# Spec: Dockerfile の更新

## Goal

Dockerfile 内のツールバージョンを version baseline 調査（2026-03-11）の結果に基づいて最新安定版に更新する。

## Scope

- `Dockerfile` の `RUST_VERSION` ARG: 1.93.1 → 1.94.0
- `Dockerfile` の `CARGO_CHEF_VERSION` ARG: 0.1.76 → 0.1.77
- 他のツールバージョンは最新のため変更しない

## Constraints

- アプリケーションコードの変更は含まない
- `Cargo.toml` の変更は含まない
- base image タグ `lukemathwalker/cargo-chef:latest-rust-1.94.0` が存在することを前提とする

## Acceptance Criteria

- [ ] `RUST_VERSION` が 1.94.0 に更新されている
- [ ] `CARGO_CHEF_VERSION` が 0.1.77 に更新されている
- [ ] `cargo make build-tools` が成功する
- [ ] `cargo make ci` が全ゲート通過する
