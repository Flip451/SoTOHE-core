# Verification: auto-cycle-replace-2026-04-10

## Scope Verified

- [ ] full-cycle.md の書き換え
- [ ] SKILL.md の参照更新
- [ ] track-signals ラッパー追加 + track:plan 手順修記
- [ ] CI 通過

## Manual Verification Steps

- [ ] full-cycle.md がタスクごとの implement（CI・done transition 含む）→ review (zero_findings) → commit ループ構造を定義していることを確認 (T001)
- [ ] full-cycle.md がコミットメッセージをタスク説明から自動生成することを定義していることを確認 (T001)
- [ ] full-cycle.md がタスク失敗時の停止と報告を定義していることを確認 (T001)
- [ ] full-cycle.md から transitional compatibility の記述が削除され正式コマンドとして記述されていることを確認 (T001)
- [ ] SKILL.md の説明が新セマンティクスと整合していることを確認 (T002)
- [ ] cargo make track-signals が Makefile.toml に追加されていることを確認 (T003)
- [ ] track:plan コマンドに track-signals → spec-approve の手順が明記されていることを確認 (T003)
- [ ] cargo make ci 通過 (T004)

## Result / Open Issues

_未実施_

## verified_at

_未設定_
