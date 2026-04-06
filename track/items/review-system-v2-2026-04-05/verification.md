# Verification: Review System v2

## Scope Verified

- [ ] Domain 純粋型が domain 層に配置され、I/O を持たない
- [ ] UseCase port traits が usecase 層に配置されている
- [ ] ReviewCycle が before/after hash でレビュー中の変更を検出する
- [ ] スコープ独立: 1 スコープの変更が他スコープの approval を無効化しない
- [ ] .commit_hash 進行後に diff スコープがインクリメンタルに縮小する
- [ ] review.json v2 が findings を含むスコープ毎の rounds 履歴を保持する
- [ ] v1 frozen scope 関連コードが完全に削除されている

## Manual Verification Steps

- [ ] 複数スコープの並列レビュー → review.json が fs4 ロックで破損しない
- [ ] コミット後に .commit_hash が更新され、次のレビュースコープが縮小する
- [ ] review-scope.json 変更が harness-policy スコープの StaleHash として検出される
- [ ] 存在しないスコープ名で review() を呼ぶと UnknownScope エラーが返る
- [ ] cargo make ci が全て通過する

## Result / Open Issues

（実装後に記入）

## verified_at

（実装後に記入）
