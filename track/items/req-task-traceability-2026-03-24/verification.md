# Verification: CC-SDD-01 要件-タスク双方向トレーサビリティ

## Scope Verified

- [ ] SpecRequirement に task_refs フィールドが追加されている
- [ ] task_refs が省略された spec.json のデシリアライズで空配列がデフォルトとなる
- [ ] spec.json の round-trip (serialize/deserialize) で task_refs が保持される
- [ ] spec.md に task_refs が表示される
- [ ] sotp verify spec-coverage が in_scope + acceptance_criteria の task_refs 未設定を検出する
- [ ] sotp verify spec-coverage が constraints の task_refs 未設定を CI エラーにしない
- [ ] sotp verify spec-coverage が不正な TaskId 参照を検出する
- [ ] cargo make ci に verify-spec-coverage が含まれている
- [ ] /track:plan skill が新規 spec.json 生成時に task_refs を含めている

## Manual Verification Steps

1. `cargo make ci` が全テスト通過することを確認
2. 新規 spec.json に task_refs を設定し、`sotp verify spec-coverage` が pass することを確認
3. task_refs を意図的に空にして `sotp verify spec-coverage` が fail することを確認
4. 存在しない TaskId を task_refs に設定して検出されることを確認
5. `cargo make track-sync-views` で spec.md に task_refs が反映されることを確認
6. task_refs フィールドが省略された既存 spec.json をデシリアライズし、空配列がデフォルトになることを確認
7. constraints の task_refs を空のまま `sotp verify spec-coverage` を実行し、CI エラーにならないことを確認

## Result / Open Issues

(実装後に記録)

## Verified At

(実装後に記録)
