# Verification: Incremental review scope

## Scope Verified

- [ ] ApprovedHead newtype（domain 層、40文字 hex 検証）
- [ ] ReviewCycle.approved_head フィールド追加
- [ ] review_json_codec の approved_head encode/decode
- [ ] effective_diff_base ヘルパー（infra 層）
- [ ] record-round でのインクリメンタルスコープ計算
- [ ] check-approved でのインクリメンタルスコープ計算
- [ ] track-commit-message での approved_head 自動記録
- [ ] set-approved-head リカバリコマンド
- [ ] 回帰テスト（承認コミット後のスコープ非拡大）
- [ ] invalid approved_head フォールバックテスト

## Manual Verification Steps

1. `cargo make ci` が通ること
2. track branch で T001 コミット → T002 のみ変更 → レビューが T002 のスコープのみを対象にすることを確認
3. `sotp review set-approved-head` でリカバリできることを確認

## Result

- 未実施

## Open Issues

- なし

## Verified At

- 未検証
