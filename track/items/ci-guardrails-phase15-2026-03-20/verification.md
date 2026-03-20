# Verification: CI Guardrails Phase 1.5

## Scope Verified

- [ ] WF-54: review guard fix
- [ ] module-size verify subcommand
- [ ] domain-strings verify subcommand
- [ ] clippy::too_many_lines attribute
- [ ] view-freshness verify subcommand (plan.md only)
- [ ] registry.md gitignore (STRAT-04)
- [ ] cargo make ci integration

## Manual Verification Steps

1. `/track:plan` で新 track を作成し、metadata.json に review section があることを確認
2. review section を手動削除した metadata.json で `sotp review check-approved` がエラーを返すことを確認（fail-closed）
3. review state = NotStarted の状態で、`tmp/track-commit/commit-message.txt` にテストメッセージを書き、`cargo make track-commit-message` が review guard でブロックされずに進むことを確認（CI 通過が前提）
3. `sotp verify module-size --project-root .` を実行し、400行超ファイルに WARNING、700行超ファイルに ERROR が出ることを確認
4. `sotp verify module-size` が vendor/ 配下のファイルをスキップすることを確認（vendor/ 内に長大ファイルがあっても報告されない）
5. `sotp verify domain-strings --project-root .` を実行し、既存の pub String フィールドが検出されることを確認
6. `sotp verify domain-strings` が newtype（`pub struct Foo(String)` の内部フィールド）を報告しないことを確認（negative test）
7. `cargo make clippy` で `too_many_lines` 警告が出ることを確認
8. `sotp verify view-freshness --project-root .` を実行し、乖離がない場合に pass することを確認
9. plan.md を手動編集し、view-freshness が fail することを確認
10. `.gitignore` に `track/registry.md` が含まれていることを確認（`grep registry .gitignore`）
11. `cargo make track-sync-views` で registry.md が正常に生成されることを確認
12. registry.md を削除後、`cargo make track-sync-views` で再生成されることを確認
13. `sotp verify track-registry` を実行し、registry.md のファイル有無ではなく metadata.json の整合性で pass/fail することを確認（registry.md を削除した状態でも metadata.json が正しければ pass する）
14. registry.md を参照するワークフローコード（planning-only commit 検証、activation dirty-path handling）が untrack 後も正常動作することを確認（`grep -r 'registry.md'` で参照箇所を洗い出し、各箇所が file existence ではなく生成 or metadata ベースで動作することを検証）
15. `cargo make ci` が全新規ゲートを含めて pass することを確認

## Result / Open Issues

(pending)

## verified_at

(pending)
