<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0"
---

# RVW-37: review.md グループ名 infra → infrastructure 統一

## Goal

review.md (SKILL.md) のレビューグループ名を track/review-scope.json と一致させ、check-approved のパーティション比較が正しく動作するようにする。

## Scope

### In Scope
- review.md 内のグループ分類テーブルで 'infra' → 'infrastructure' に変更 [source: [audit:review-process-audit-2026-03-31§2.1]] [tasks: T001]
- review.md 内の cargo make track-local-review 呼び出し例で --group infra → --group infrastructure に変更 [source: [audit:review-process-audit-2026-03-31§2.1]] [tasks: T002]
- review.md 内のサマリー出力例の infra 表記を修正 [source: [audit:review-process-audit-2026-03-31§2.1]] [tasks: T003]

### Out of Scope
- Rust コードの変更（CLI, domain, usecase, infrastructure 層はすべて変更なし） [source: [design:rvw-remediation-plan§PhaseA]]
- review-scope.json の変更（正式名 'infrastructure' が既に定義済み） [source: [code:track/review-scope.json:10]]

## Constraints
- Rust コード変更なし — SKILL.md のみの修正 [source: [design:rvw-remediation-plan§PhaseA]]

## Acceptance Criteria
- [ ] review.md 内に 'infra' 単独のグループ名参照が存在しないこと（grep -w infra で 0 件） [source: [audit:review-process-audit-2026-03-31§2.1]] [tasks: T001, T002, T003]
- [ ] review.md 内のグループ名が track/review-scope.json の groups キーと完全一致すること [source: [code:track/review-scope.json]] [tasks: T001, T002, T003]

