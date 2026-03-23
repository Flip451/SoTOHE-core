# ADR 導入 — 検証

## Scope Verified

- [ ] knowledge/adr/ ディレクトリ構造
- [ ] ADR テンプレートと採番規則
- [ ] 既存ドラフト移動（4 件）
- [ ] DESIGN.md ADR 分解（13 件）
- [ ] Convention 文書 + CLAUDE.md 参照

## Manual Verification Steps

1. `knowledge/adr/README.md` を読み、テンプレートと索引が正しいか確認
2. `knowledge/adr/` 内の ADR ファイルが採番規則（YYYY-MM-DD-HHMM-slug.md）に従っているか確認
3. `knowledge/adr/` に 17 件以上の ADR が存在するか確認（ドラフト 4 + DESIGN.md 13）
4. DESIGN.md Key Design Decisions テーブルの全 13 行に対応する ADR へのリンクがあるか確認
5. `tmp/adr-drafts/` が空または存在しないことを確認（移動完了の証明）
6. `project-docs/conventions/adr.md` が ADR/Convention の関係を説明しているか確認
7. CLAUDE.md の参照リストに `knowledge/adr/` が含まれるか確認

## Result

(未実施)

## Open Issues

(なし)

## Verified At

(未実施)
