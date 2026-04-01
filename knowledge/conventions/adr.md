# Convention: Architecture Decision Records (ADR)

## Rule

設計判断は `knowledge/adr/` に ADR として記録する。

## When to Write an ADR

- 技術選定（クレート、ツール、フレームワーク）
- アーキテクチャ変更（層構造、依存方向、モジュール分割）
- ワークフロー変更（Phase 戦略、トラック運用方針）
- 却下した選択肢が将来再検討される可能性がある判断

## Format

- Nygard 式 + Rejected Alternatives + Reassess When
- 採番: `YYYY-MM-DD-HHMM-slug.md`
- テンプレートと索引: `knowledge/adr/README.md`

## ADR vs Convention

| | ADR | Convention |
|---|---|---|
| 問い | 「なぜこうした？」 | 「これからどうする？」 |
| 時制 | 過去形 | 現在形 |
| 寿命 | 永続（superseded でも残る） | 現行ルールのみ有効 |

Convention から関連 ADR にリンクするには `## Decision Reference` セクションを追加する。

## Decision Reference

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — ADR テンプレート・索引
- [tmp/knowledge-restructure-design-2026-03-20.md](../../tmp/knowledge-restructure-design-2026-03-20.md) — 元の設計メモ
