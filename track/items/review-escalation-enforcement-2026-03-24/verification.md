# Verification: Review escalation enforcement

## Scope Verified
- [ ] code ファイル含む staged diff + NotStarted review → check-approved 拒否
- [ ] planning-only ファイルのみ staged diff + NotStarted review → check-approved 通過
- [ ] /track:review が record-round を呼んで review state を永続化
- [ ] sotp review status が per-group 状態を表示
- [ ] 既存の track-commit-message フローが壊れていない
- [ ] cargo make ci が全テスト通過

## Manual Verification Steps
1. planning-only コミット (metadata.json のみ) が check-approved を通過することを確認
2. code ファイルを含むコミットが review 未完了で check-approved に拒否されることを確認
3. /track:review 後に record-round が呼ばれ review state が更新されることを確認
4. sotp review status で per-group 状態が表示されることを確認
5. cargo make ci が全テスト通過することを確認

## Result / Open Issues
(実装完了後に記入)

## Verified At
(検証完了後に記入)
