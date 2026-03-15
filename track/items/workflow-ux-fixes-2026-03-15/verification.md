# Verification: Track ワークフロー UX 改善

## 自動検証

- [ ] `cargo make ci` 通過
- [ ] `cargo make scripts-selftest` 通過
- [ ] `cargo make hooks-selftest` 通過

## 手動検証

- [ ] `cargo make track-commit-message` の出力が数行に収まること（CI 詳細が表示されないこと）
- [ ] `plan/` ブランチで `cargo make track-pr-push '<track-id>'` が成功すること
- [ ] `plan/` ブランチで `cargo make track-pr-ensure '<track-id>'` が PR を作成できること
- [ ] `plan/` ブランチで引数省略時に `cargo make track-pr-push` / `track-pr-ensure` がエラーを返すこと（fail-closed）
- [ ] `plan/` ブランチで `cargo make track-pr-review` が `track/<id>` ブランチ要求エラーを返すこと（fail-closed 維持）
- [ ] PR body に git 関連キーワードを含んでもフックでブロックされないこと
- [ ] `cargo make add-all` が tmp/ の gitignore 対象を無視して正常終了すること
- [ ] `track/workflow.md` に `plan/` ブランチの PR ワークフローが記載されていること

## 結果

- 検証日時: (未実施)
- 結果: (未実施)
