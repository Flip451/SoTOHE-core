# Verification: spec-template-foundation-2026-03-18

## Scope Verified

- [ ] T001: block-test-deletion hook が test ファイル削除をブロックすることを確認
- [ ] T002: タスク説明変更時に save が拒否されることを確認
- [ ] T003: spec.md テンプレートに source attribution が含まれることを確認
- [ ] T004: frontmatter に signals optional field が追加されていることを確認

## Manual Verification Steps

1. `sotp hook dispatch block-test-deletion` にテストファイルパスを渡し exit 2 を確認
2. metadata.json のタスク説明を変更して save し、エラーが返ることを確認
3. `/track:plan` で新規トラックを作成し、spec.md に `[source: ...]` が含まれることを確認
4. signals フィールド付き/なしの spec.md frontmatter で `sotp verify spec-frontmatter` が通ることを確認
5. `cargo make ci` が全パスすることを確認

## Result / Open Issues

N/A — implementation not yet started.

## verified_at

N/A — pending implementation.
