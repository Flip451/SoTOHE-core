# Verification: spec.json SSoT 化

## Scope Verified
- [ ] spec.json スキーマ v1 が定義され、全フィールドが型安全にデシリアライズされる
- [ ] render_spec() が spec.json から現行互換の spec.md を生成する
- [ ] 構造化信号評価が Markdown パースなしで動作する (Blue/Yellow/Red 分類)
- [ ] sources 空配列の要件が Red (MissingSource) と評価される
- [ ] Multi-source 要件で最高信頼度の信号が選択される
- [ ] sotp track signals が spec.json を読み書きする
- [ ] sotp verify spec-signals が spec.json から評価し red==0 ゲートを適用する (red>0 で拒否)
- [ ] sotp verify spec-states が spec.json の domain_states を検証する
- [ ] sotp verify spec-attribution が spec.json の sources を検証する
- [ ] sotp verify spec-frontmatter が spec.json 存在時にスキーマ検証に委譲する
- [ ] sotp verify latest-track が spec.json または spec.md の存在をチェックする (spec.json 優先)
- [ ] 全移行済み verifier (spec-signals, spec-states, spec-attribution, spec-frontmatter, latest-track) が spec.json なし旧 track で spec.md fallback 動作する
- [ ] sync_rendered_views が spec.md を plan.md と並行して自動生成する
- [ ] /track:plan スキルが spec.json を生成する
- [ ] cargo make ci が全テスト通過する

## Manual Verification Steps
1. 新規 track を作成し、spec.json が生成されることを確認
2. `cargo make track-sync-views` で spec.md が正しくレンダリングされることを確認
3. `sotp track signals <track-id>` で spec.json の signals が更新されることを確認
4. `sotp verify spec-signals <spec-path>` が spec.json から評価し、red>0 の場合に拒否することを確認
5. `sotp verify spec-states <spec-path>` が spec.json の domain_states を検証することを確認
6. `sotp verify spec-attribution <spec-path>` が spec.json の sources を検証することを確認
7. `sotp verify spec-frontmatter <spec-path>` が spec.json 存在時にスキーマ検証に委譲することを確認
8. `sotp verify latest-track` が spec.json または spec.md の存在をチェックすることを確認
9. Multi-source 要件で最高信頼度の信号が選択されるテストが通ることを確認
10. sources 空配列の要件が Red と評価されるテストが通ることを確認
11. spec.json のない旧 track で全移行済み verifier が fallback 動作することを確認
12. `cargo make ci` が全テスト通過することを確認

## Result / Open Issues
(実装完了後に記入)

## Verified At
(検証完了後に記入)
