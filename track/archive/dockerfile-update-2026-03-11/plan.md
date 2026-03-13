<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Dockerfile の更新

Dockerfile 内の Rust toolchain と cargo-chef のバージョンを最新安定版に更新する

## バージョン更新

- [x] Dockerfile の ARG 更新（RUST_VERSION 1.93.1→1.94.0, CARGO_CHEF_VERSION 0.1.76→0.1.77）

## ビルド検証

- [x] Docker イメージ再ビルド（cargo make build-tools）

## CI 検証

- [x] CI 検証（cargo make ci）で全ゲート通過を確認
