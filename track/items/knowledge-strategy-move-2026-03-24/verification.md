# knowledge/strategy/ 移動 — 検証

## Scope Verified

- [ ] knowledge/strategy/ ディレクトリ構造
- [ ] 5 ファイル移動 + 日付サフィックス除去
- [ ] .gitignore 除外ルール
- [ ] 全 git tracked ファイルの旧パス参照修正（knowledge/adr/*.md, conventions/*.md 含む）
- [ ] CLAUDE.md + memory + knowledge/adr/README.md 参照更新

## Manual Verification Steps

1. `knowledge/strategy/` に README.md + 5 ファイルが存在するか確認
2. ファイル名に日付サフィックスがないか確認
3. `git ls-files --error-unmatch knowledge/strategy/*` で全ファイルが tracked か確認
4. Grep で旧 tmp/ 戦略文書パスが git tracked ファイルに残っていないか確認（track/items/ 配下の計画文書は除外）
5. CLAUDE.md に knowledge/strategy/ への参照があるか確認
6. knowledge/adr/README.md 内の旧 tmp/ パス参照が新パスに更新されているか確認
7. `cargo make ci` が全テスト通過するか確認

## Result

(未実施)

## Open Issues

(なし)

## Verified At

(未実施)
