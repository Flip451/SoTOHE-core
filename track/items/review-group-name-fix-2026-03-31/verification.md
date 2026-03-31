# Verification: review-group-name-fix-2026-03-31

## Scope Verified

- [x] review.md グループ名統一（T001-T003）

## Manual Verification Steps

1. `grep -w 'infra' .claude/commands/track/review.md` が 0 件であること → ✅ 確認済み
2. review.md 内のグループ名が `track/review-scope.json` の `groups` キー（domain, usecase, infrastructure, cli, harness-policy）と一致すること → ✅ 確認済み

## Result / Open Issues

全 3 箇所の `infra` → `infrastructure` 置換完了。Rust コード変更なし。

追加変更:
- `.claude/skills/track-plan/SKILL.md`: metadata.json の review section に関する古い記述を削除
- `.claude/commands/track/plan.md`: `task_refs` → `task_ids` の修正（codec 互換性修正）

## verified_at

2026-03-31
