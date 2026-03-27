# Verification: autorecord-stabilization-2026-03-26

## Scope Verified

- [ ] track/review-scope.json が存在し、スキーマが妥当
- [ ] ReviewScopePolicy がパスを正しく分類する
- [ ] review_hash が worktree から直接読み、未ステージでも成功する
- [ ] review.json の変更で hash が変わらない
- [ ] 他トラック/planning-only ファイルの変更で hash が変わらない
- [ ] 実装ファイルの変更で hash が変わる
- [ ] review-scope.json が存在しない場合、review_hash が明示的エラーで失敗する（fail-closed）
- [ ] review-scope.json のパターン変更でスコープ分類が切り替わる
- [ ] review-scope.json 自体が hash scope に含まれ、ポリシー変更が旧承認を無効化する
- [ ] RecordRoundProtocolImpl が single-phase で動作する
- [ ] legacy hash が migration error を返す
- [ ] index_tree_hash_normalizing が production path から除去済み

## Manual Verification Steps

1. `cargo make test` — 全テスト通過
2. `cargo make ci` — CI ゲート通過
3. 新規 track（未コミット）で `sotp review codex-local --auto-record` が成功することを確認
4. 並列 review group の record-round が互いの hash を invalidate しないことを確認
5. `.claude/docs/DESIGN.md` の変更が review hash を変化させないことを確認
6. `libs/domain/src/*.rs` の変更が review hash を変化させることを確認
7. `track/review-scope.json` を編集後に review_hash を再計算し、hash が変化する（ポリシー変更が旧承認を無効化する）ことを確認

## Result / Open Issues

- 結果: 未実施
- オープン課題: なし

## Verified At

- 未検証
