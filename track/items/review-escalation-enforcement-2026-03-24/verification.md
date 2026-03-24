# Verification: Review escalation enforcement

## Scope Verified
- [x] code ファイル含む staged diff + NotStarted review → check-approved 拒否
- [x] planning-only ファイルのみ staged diff + NotStarted review → check-approved 通過
- [x] /track:review が record-round を呼んで review state を永続化
- [x] sotp review status が per-group 状態を表示
- [x] 既存の track-commit-message フローが壊れていない
- [x] cargo make ci が全テスト通過

## Manual Verification Steps
1. planning-only コミット (metadata.json のみ) が check-approved を通過することを確認
2. code ファイルを含むコミットが review 未完了で check-approved に拒否されることを確認
3. /track:review 後に record-round が呼ばれ review state が更新されることを確認
4. sotp review status で per-group 状態が表示されることを確認
5. cargo make ci が全テスト通過することを確認

## Result / Open Issues
- T001: usecase テスト 5 件 + CLI テスト 25 件で planning_only フラグのガード動作を検証済み
- T002: review.md Step 2e に record-round 配線を追加。Fix phase Step 1 に事実検証ルールを追加
- T003: sotp review status CLI コマンド実装済み。per-group Fast/Final + code hash (NotRecorded/Pending/Computed) + escalation 表示
- T004: is_planning_only_path: directory prefix + doc extension allowlist (fail-closed)。extract_paths_from_name_status: rename/copy 両方向パス取得
- ADR 2 件追加: review state trust model、planning-only bypass scope
- 既知の制限: metadata.json review セクション手動リセットによる bypass → review.json 分離トラックで対応予定

## Verified At
2026-03-24
