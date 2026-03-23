# ADR 導入 — 仕様

## Goal

設計判断の「なぜ」と「却下した選択肢」を体系的に記録する ADR（Architecture Decision Records）の仕組みを導入する。設計判断が `tmp/` の一時ファイルに埋もれる問題を解消する。

## Scope

### Phase 1（このトラック）

- `knowledge/adr/` ディレクトリ新設
- ADR テンプレート（Nygard 式 + Rejected Alternatives + Reassess When）
- 採番規則: `YYYY-MM-DD-HHMM-slug.md`
- 既存ドラフト 4 件の移動
- DESIGN.md Key Design Decisions 13 件の ADR 化
- convention 文書作成
- CLAUDE.md 参照追加

### Out of Scope（別トラック）

- `knowledge/` フル再編（`.claude/docs/`, `project-docs/`, `docs/` の統合移動）
- DESIGN.md の Canonical Blocks 削除
- `sotp verify doc-links` CI ガード
- 設計メモ `tmp/knowledge-restructure-design-2026-03-20.md` の残り全項目

## Constraints

- Phase 2 の信号機実装（別セッションで進行中）とファイル競合しないこと
- 既存の `project-docs/conventions/` ディレクトリ構造はそのまま維持
- `.claude/docs/DESIGN.md` は残す（ADR へのリンクを追加するのみ）

## Acceptance Criteria

- [ ] `knowledge/adr/README.md` が存在し、テンプレートと索引を含む
- [ ] `knowledge/adr/` に 17 件以上の ADR が存在する（ドラフト 4 + DESIGN.md 13）
- [ ] `tmp/adr-drafts/` が空または存在しない（移動完了の証明）
- [ ] `project-docs/conventions/adr.md` が存在し、ADR と Convention の関係を説明
- [ ] `CLAUDE.md` の参照リストに `knowledge/adr/` が含まれる
- [ ] DESIGN.md Key Design Decisions テーブルの各行に対応する ADR へのリンクがある
