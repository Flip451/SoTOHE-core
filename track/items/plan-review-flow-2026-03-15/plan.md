<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# 計画完了後の推奨フローに /track:review を追加

/track:plan, /track:activate, /track:implement 完了後の推奨フローに /track:review ステップを追加し、レビューなしのコミットを防止する。

## スキルファイルの推奨フロー更新

/track:plan と /track:activate の推奨次コマンドに /track:review を追加。DEVELOPER_AI_WORKFLOW.md と /track:review スキルを更新。implement と full-cycle は既に含むため変更不要。

- [x] /track:plan と /track:activate の推奨次コマンドに /track:review を追加。DEVELOPER_AI_WORKFLOW.md のフロー図に計画レビュー分岐・pr/merge/done ステップ・コマンド一覧表を追加。/track:review スキルにインラインレビュー禁止と other グループを追加。implement と full-cycle は既に /track:review を含むため変更不要。

## CI 検証

- [x] cargo make ci が通ることを確認。
