# Verification: SPEC-05 Domain States 信号機 Stage 2

## Scope Verified
- [ ] DomainStateEntry に transitions_to が追加されている (省略/空配列/値ありの3パターン)
- [ ] transitions_to の参照整合性検証が動作する
- [ ] syn AST スキャナーが型名 + 遷移関数を検出する
- [ ] Result/Option アンラップが正しく動作する
- [ ] Per-state 信号評価 (Blue/Yellow/Red) が基準通りに判定される
- [ ] 終端状態 (transitions_to: []) が型存在のみで Blue になる
- [ ] transitions_to 省略が最大 Yellow になる
- [ ] sotp track domain-state-signals が spec.json を更新する
- [ ] sotp verify spec-states が red==0 gate を適用する
- [ ] sotp verify spec-states が Stage 1 前提条件を検証する
- [ ] rendered spec.md に Signal + Transitions 列が表示される
- [ ] plan.md に Stage 1 + Stage 2 信号サマリーが表示される
- [ ] spec.json スキーマが正しくデコードされる (codec round-trip)
- [ ] spec.json → spec.md レンダリングが正しく動作する (sync_rendered_views)
- [ ] cargo make ci が全テスト通過する

## Manual Verification Steps
1. spec.json が正しくデコード・エンコードされることを確認 (codec round-trip テスト)
2. `cargo make track-sync-views` で spec.md が spec.json から正しく生成されることを確認
3. テスト用 domain コードに enum + 遷移関数を用意し、syn スキャナーが検出することを確認
2. `sotp track domain-state-signals <track-id>` で spec.json が更新されることを確認
3. 終端状態 (transitions_to: []) が Blue と判定されることを確認
4. transitions_to 未宣言の状態が Yellow と判定されることを確認
5. 型が存在しない状態が Red と判定されることを確認
6. Result/Option でラップされた遷移先が正しく検出されることを確認
7. transitions_to の参照先が domain_states にない場合にエラーになることを確認
8. Stage 1 (spec signals) が red>0 の場合に spec-states が拒否することを確認
9. `cargo make ci` が全テスト通過することを確認

## Result / Open Issues
(実装完了後に記入)

## Verified At
(検証完了後に記入)
