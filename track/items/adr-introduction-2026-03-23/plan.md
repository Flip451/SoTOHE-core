<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# ADR 導入 — knowledge/adr/ 新設 + 既存判断の ADR 化

ADR（Architecture Decision Records）を knowledge/adr/ に導入し、設計判断の「なぜ」と「却下した選択肢」を体系的に記録する仕組みを構築する。
フル知識ディレクトリ再編（knowledge/ 統合）は別トラックとして先送り。

## ADR ディレクトリ基盤

T001: knowledge/adr/README.md を作成。
Nygard 式テンプレート（+ Rejected Alternatives + Reassess When）。
採番規則: YYYY-MM-DD-HHMM-slug.md。
索引テーブルを含む。

- [ ] knowledge/adr/README.md 作成（テンプレート + 運用ルール + 索引）

## 既存ドラフト移動

T002: tmp/adr-drafts/ の 4 ファイルを knowledge/adr/ に移動。
phase-1.5-good-enough, sotp-extraction-deferred, two-stage-signal-architecture, spec-code-consistency-deferred。

- [ ] tmp/adr-drafts/ の 4 ADR を knowledge/adr/ に移動

## DESIGN.md 既存判断の ADR 化

T003: DESIGN.md Key Design Decisions テーブルの 13 件を個別 ADR ファイルに分解。
DESIGN.md 側は ADR へのリンクに置換。

- [ ] DESIGN.md Key Design Decisions → 個別 ADR 分解（13 件）

## Convention 文書 + 参照整備

T004: project-docs/conventions/adr.md で ADR 運用ルールを convention 化。
CLAUDE.md の参照リストに knowledge/adr/ を追加。

- [ ] project-docs/conventions/adr.md 作成 + CLAUDE.md 参照追加
