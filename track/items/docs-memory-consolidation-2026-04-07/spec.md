<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
---

# 運用ルールのドキュメント移行 + メモリ棚卸し

## Goal

Claude Code memory に蓄積された運用ルール（61件）を git 管理ドキュメントに移行し、全 SoTOHE 利用者が恩恵を受けられるようにする。

## Scope

### In Scope
- review-protocol.md 新規作成 [source: memory feedback files] [tasks: T001]
- language-policy.md 新規作成 [source: memory feedback files] [tasks: T002]
- track/workflow.md 追記 [source: memory feedback files] [tasks: T003]
- .claude/rules/ 追記 [source: memory feedback files] [tasks: T004]
- ForgeCode 比較レポート [source: ForgeCode GitHub README, TermBench 2.0] [tasks: T005]

### Out of Scope
- Rust コード変更 [source: track scope decision]

## Acceptance Criteria
- [ ] memory 件数が 30 件以下に削減されていること [source: session goal] [tasks: T001, T002, T003, T004]
- [ ] 新規 convention ファイルが conventions README に登録されていること [source: conventions workflow] [tasks: T001, T002]

