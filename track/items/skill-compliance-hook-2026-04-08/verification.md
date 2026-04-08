# Verification: WF-67 agent-router 廃止 + skill 遵守フック導入

## Scope Verified

- [ ] agent-router.py + test_agent_router.py 削除済み
- [ ] settings.json / orchestra.rs / DESIGN.md から agent-router 参照除去済み
- [ ] sotp hook dispatch skill-compliance が実装済み
- [ ] /track:* コマンド検出 → SKILL.md フェーズリマインド注入が動作する
- [ ] external guide injection が動作する
- [ ] cargo make hooks-selftest 通過

## Manual Verification Steps

1. `/track:plan <feature>` を送信し、additionalContext に SKILL.md フェーズ要件リマインドが含まれることを確認
2. `/track:implement` を送信し、対応するリマインドが含まれることを確認
3. `guides.json` にエントリがある状態で `/track:plan` を送信し、ガイドサマリーが注入されることを確認
4. `/track:*` 以外のプロンプト（例: 「このコードをレビューして」）を送信し、skill-compliance フックが反応しないことを確認
5. `cargo make ci` が全チェック通過することを確認

## Result

（実装完了後に記入）

## Open Issues

（実装中に発見された問題を記録）

## Verified At

（検証完了日時を記入）
