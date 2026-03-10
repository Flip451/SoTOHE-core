# Docs Directory Guide

`docs/` は、このテンプレートから生成される各プロジェクトで共有する補助ドキュメント置き場です。

## Purpose

- 利用者向けガイド（運用手順、FAQ、トラブルシュート）
- 設計補足（ADRの要約、図、決定背景）
- 外部公開を想定した開発ドキュメント
- 外部長文ガイドの運用方針と索引

プロジェクト固有の実装規約は `project-docs/conventions/` で管理し、`docs/` に混在させないでください。

## What Should Not Be Stored Here

- 一時比較ファイルや移行作業のスクラッチ
- 個人用メモや検証中の断片ファイル

これらは `tmp/` などのローカルスクラッチ領域で扱い、テンプレート本体に含めないでください。

## Managed Docs

- `EXTERNAL_GUIDES.md`: 外部長文ドキュメントの運用方針
- `external-guides.json`: 外部長文ドキュメントの索引
- `../project-docs/conventions/`: プロジェクト固有の設計・実装規約
