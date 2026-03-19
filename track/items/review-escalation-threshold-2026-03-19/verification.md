# Verification: Review Escalation Threshold (WF-36)

## Scope Verified

- [ ] Domain 層の型定義と状態遷移ロジック
- [ ] Usecase 層のカテゴリ正規化
- [ ] Infra/Codec 層の metadata.json スキーマ拡張
- [ ] CLI 層の record-round 拡張と resolve-escalation サブコマンド
- [ ] ドキュメント更新

## Manual Verification Steps

1. **エスカレーション発動テスト**:
   - 単一グループ: 同一 concern で 3 連続 closed cycle → 3 回目は正常記録 + `EscalationPhase::Blocked` に遷移
   - 複数グループ: 2 つの expected_groups が同一ラウンドに参加する場合、全グループが同一ラウンドを完了して初めて 1 closed cycle としてカウントされることを確認（グループごとのカウントではなく、ラウンド完了単位）
2. **ハードブロックテスト**: エスカレーション `blocked` 中に `record_round` / `record_round_with_pending` / `check_commit_ready` を呼び出し、すべてが `Err(ReviewError::EscalationActive)` で拒否されることを確認
3. **解除テスト（正常系）**: `resolve-escalation` に `ReviewEscalationResolution` 全フィールド（blocked_concerns, workspace_search_ref, reinvention_check_ref, decision, summary, resolved_at）を渡して解除。解除後に `ReviewStatus::Invalidated` + `code_hash = None` + `EscalationPhase::Clear` + `last_resolution` に record が保存されていることを確認
   - **解除テスト（エラー系）**:
     - `blocked_concerns` が現在の blocked concerns と一致しない場合に `ResolutionConcernMismatch` が返ることを確認
     - `workspace_search_ref` / `reinvention_check_ref` / `summary` / `resolved_at` が空文字の場合に `ResolutionEvidenceMissing` が返ることを確認
     - エスカレーション未発動中に `resolve_escalation` を呼んだ場合に `EscalationNotActive` が返ることを確認
4. **ReviewConcern バリデーションテスト**:
   - 空文字で `ReviewConcern::new("")` を呼んだ場合に `Err(ReviewError::InvalidConcern)` が返ることを確認
   - 有効な slug（例: `"domain.review"`）で `ReviewConcern::new()` が成功することを確認
   - slug 正規化が適用されることを確認（例: 大文字→小文字、スペース→ハイフン等の変換ルールがあれば）
5. **カテゴリ正規化テスト**:
   - reviewer が `category` フィールドを出力しない場合でも file パスから concern が自動導出されることを確認
   - `category = null` かつ `file = null` の finding が `other` concern にフォールバックされることを確認
   - reviewer が `category` を出力した場合にそれが優先されることを確認
5. **streak リセットテスト**:
   - `A → B → A → A` パターン（中断あり）で A の streak が 2 であり、3 に達しないことを確認
   - `A → A → A` パターン（連続）で A の streak が 3 に達し、エスカレーションが発動することを確認
   - `zero_findings` ラウンドに空でない concerns を渡した場合に `InvalidConcern` エラーが返ることを確認
   - `findings_remain` ラウンドに空の concerns を渡した場合に `InvalidConcern` エラーが返ることを確認
6. **concerns 正規化テスト**:
   - 重複した concerns を含むラウンドを record → 保存時に重複排除されていることを確認
   - concerns が異なる順序で渡されても、保存後はソート済みであることを確認
   - resolve_escalation で concerns の順序が異なるが内容が同じリストを渡した場合に正常解除されることを確認
6. **シリアライズテスト**: metadata.json に `review.escalation` セクションが正しく書き込まれ、再読み込みで復元されることを確認。recent_cycles が 10 件を超えた場合に FIFO trim されることも確認
6. **CI ゲート**: `cargo make ci` パス

## Result / Open Issues

_To be filled after implementation_

## verified_at

_To be filled after verification_
