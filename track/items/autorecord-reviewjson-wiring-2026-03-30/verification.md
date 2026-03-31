# Verification: autorecord-reviewjson-wiring-2026-03-30

## Scope Verified

- [ ] record-round が review.json に書き込む
- [ ] check-approved が review.json から読む
- [ ] metadata.json に review セクションが書き込まれない
- [ ] planner/reviewer が --sandbox read-only で実行される
- [ ] sotp review status が review.json から正しい状態を表示する
- [ ] cargo make ci が全通過する
- [ ] record-round が per-group scope hash を review.json に記録する (T005)
- [ ] check-approved が per-group scope hash を検証し stale 時にブロックする (T006)

## Manual Verification Steps

1. `cargo make ci` 全通過
2. `cargo make build-sotp` で bin/sotp をリビルドしてから以降の手順を実行（WF-08b: 陳腐化バイナリ防止）
3. テスト用トラックで `cargo make track-local-review -- --model gpt-5.4-mini --briefing-file ... --auto-record ...` を実行し、review.json が作成されることを確認
3. `bin/sotp review status` が review.json の内容（cycle, groups, rounds）を正しく表示することを確認
4. `bin/sotp review check-approved` が review.json ベースで判定することを確認
5. planner 呼び出し時に Codex の出力ヘッダーが `sandbox: read-only` であることを確認
6. reviewer 呼び出し時に Codex の出力ヘッダーが `sandbox: read-only` であることを確認（セッションログで検証）
7. auto-record 後に `metadata.json` 内の `review` セクションが存在しない（または空）であることを確認: `jq '.review' track/items/<id>/metadata.json` が null を返すこと
8. `cargo make add-all` 実行後に `review.json` がステージされることを確認: `git diff --cached --name-only` に `track/items/<id>/review.json` が含まれること（review.json は review_operational ファイルとして worktree に直接書き込まれ、コミット時に add-all でステージされる設計）
9. record-round が group の frozen scope ファイル群から review-scope manifest hash を計算し、review.json の round hash に記録していることを確認 (T005)
10. check-approved が各 group の latest round hash を current scope hash と照合し、コード変更後に stale としてブロックすることを確認 (T006)

## Result / Open Issues

- (実施後に記録)

## verified_at

- (実施後に記録)
