# Product Overview

> このファイルは `/track:setup` 時に設定します。
> プロダクトの「真実の源泉」として使用します。

## Product Name

SoTOHE-core

## Vision

SoT（真実の源泉）指向のRust開発向けハーネスの核となるCLIを提供する

## Target Users

Rust で中〜大規模プロジェクトを開発するチーム。仕様・設計・実装の乖離が起きやすく、真実の源泉が散在する問題を抱えている

## Core Features

1. track ワークフロー管理 CLI
2. メタデータ駆動の状態遷移エンジン
3. CI 連携バリデーション

## Success Criteria

- track コマンド経由で仕様→実装→レビューの全工程を完結できる
- CI ゲート通過率 100% を維持

## Out of Scope

- GUI / Web ダッシュボード

## Stakeholders

| Role | Responsibility |
|------|---------------|
| プロジェクトオーナー | 仕様承認・優先度決定 |
| 開発者 | CLI 実装・テスト |
