# Spec: 計画完了後の推奨フローに /track:review を追加

## 概要

各スキルコマンドの完了後に表示される「推奨次コマンド」に `/track:review` を追加し、レビューなしでコミットに進むことを防止する。

## 背景

- 現状の推奨フロー: `/track:plan` → `/track:implement` → `/track:commit`
- `/track:review` が推奨フローから欠落しており、レビュースキップが発生しやすい
- `CLAUDE.md` のガードレールルール（レビュー完了までコミット禁止）との不整合

## ゴール

- `/track:plan` と `/track:activate` の推奨次コマンドに `/track:review` を含める（`implement` と `full-cycle` は既に含む）
- 正規フロー: plan → implement → **review** → commit → pr → merge → done

## スコープ

| ファイル | 変更内容 |
|----------|---------|
| `.claude/commands/track/plan.md` | 推奨次コマンドに `/track:review` 追加 |
| `.claude/commands/track/activate.md` | 同上 |
| `.claude/commands/track/review.md` | インラインレビュー禁止、`other` observation group 追加 |
| `DEVELOPER_AI_WORKFLOW.md` | Mermaid フロー図に計画レビュー分岐・pr/merge/done 追加、コマンド一覧表に 3 コマンド追加 |
| `.claude/commands/track/implement.md` | 変更不要（既に `/track:review` を含む） |
| `.claude/commands/track/full-cycle.md` | 変更不要（既に `/track:review` を含む） |

## 完了条件

- [ ] 変更対象ファイルの推奨フローに `/track:review` が含まれていること
- [ ] `cargo make ci` が通ること
