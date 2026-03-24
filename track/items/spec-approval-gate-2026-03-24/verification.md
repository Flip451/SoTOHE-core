# Verification: CC-SDD-02 明示的承認ゲート

## Scope Verified

- [ ] SpecStatus enum (Draft/Approved) がドメイン層に存在する (T001)
- [ ] approved_at と content_hash が SpecDocument に追加されている (T001)
- [ ] approve() / is_approval_valid() / effective_status() メソッドが動作する (T001)
- [ ] spec.json の serialize/deserialize が approved_at + content_hash を含む (T002)
- [ ] auto-demote（content hash 不一致時の自動降格）が codec decode 時に動作する (T002)
- [ ] spec.md に承認ステータスと approved_at が表示される (T003)
- [ ] sotp spec approve コマンドが動作する (T004)
- [ ] cargo make spec-approve / track-record-round / track-check-approved が許可プロンプトなしで動作する (T005)
- [ ] /track:plan skill が spec.json 生成後に承認フロー案内を出力する (T005)
- [ ] DESIGN.md に CC-SDD-02 の設計決定が記録されている (T006)
- [ ] TRACK_TRACEABILITY.md に spec 承認ステータスの更新ルールが追記されている (T006)
- [ ] 統合テストで承認→変更→自動降格の end-to-end フローが検証されている (T006)
- [ ] cargo make ci が通過する (T006)

## Manual Verification Steps

1. `cargo make test` で SpecStatus enum、approve()、is_approval_valid()、effective_status() のユニットテストが全て通過することを確認 (T001)
2. `bin/sotp spec approve <track-dir>` を直接実行し、spec.json の status が "approved" に変わることを確認 (T004)
3. 承認後の spec.json に approved_at と content_hash フィールドが JSON に出力されていることを確認 (T001, T002)
4. 承認済み spec.json を codec decode し、approved_at と content_hash が正しく復元されることを確認 (T002)
5. spec.json の goal を変更して codec decode し、content_hash 不一致により status が "draft" に自動降格することを確認 (T002)
6. `cargo make track-sync-views` を実行し、spec.md のステータスバッジが更新されることを確認 (T003)
7. `cargo make spec-approve` / `cargo make track-record-round` / `cargo make track-check-approved` が許可プロンプトなしで動作することを確認 (T005)
8. `/track:plan` でテスト用 feature を計画し、spec.json 生成後に `sotp spec approve` の案内が出力されることを確認 (T005)
9. DESIGN.md に SpecStatus enum と content hash の設計決定が記載されていることを確認 (T006)
10. TRACK_TRACEABILITY.md に spec 承認に関する更新ルールが追記されていることを確認 (T006)
11. 統合テストで承認→コンテンツ変更→自動降格の end-to-end フローが検証されていることを確認 (T006)
12. `cargo make ci` が全チェック通過することを確認 (T006)

## Result

- pending

## Open Issues

- なし

## Verified At

- pending
