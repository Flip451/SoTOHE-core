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

## Lifecycle: pre-merge draft vs post-merge record

ADR が `main` にマージされているかで扱いが変わる:

- **Pre-merge (current working branch / open PR)**: ADR はまだ draft。 レビューや実装で欠陥・矛盾・見落としが判明したら **同じファイルを直接編集** して構わない。新 ADR で supersede する必要はない。pre-merge の段階で design を整える目的に沿う。
  - 判定: `git log main -- <adr-file>` が empty (当該 ADR が main に存在しない) なら pre-merge 扱い
- **Post-merge (merged to `main`)**: ADR は永続 record として不変。semantic content の変更は新 ADR で supersede または refinement する。既存 ADR に許容される編集は (1) typo 修正、(2) broken cross-reference 修正、(3) newer ADR への back-reference 追加 のみ。
  - 新 ADR は `## Related` で旧 ADR を参照。旧 ADR は当時の decision の歴史 record として残す。

この使い分けは `.claude/agents/adr-editor.md` の editing rules にも反映されている。

## Decision Reference

- [knowledge/adr/README.md](../../knowledge/adr/README.md) — ADR テンプレート・索引
- [knowledge-restructure-design-2026-03-20.md](../strategy/knowledge-restructure-design-2026-03-20.md) — 元の設計メモ
