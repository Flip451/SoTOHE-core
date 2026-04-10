<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# full-cycle をタスクごとの implement → review → commit ループに書き換え

/track:full-cycle の動作を「全タスク一括実装 + まとめてレビュー」から「タスクごとの implement → review → commit ループ」に書き換える。
コマンド名は変更せず、中身のセマンティクスのみ変更。Rust コード変更なし。

## full-cycle.md 書き換え

現在の一括実装ロジックを、metadata.json のタスク配列順にループする方式に変更。
各タスクで implement → review (zero_findings) → cargo make ci → commit を完結させる。
デフォルトはコミットメッセージ確認あり、--auto でタスク説明から自動生成。
失敗時はそのタスクで停止してステータス報告。

- [x] full-cycle.md の内容をタスクループ方式に書き換え（transitional compatibility 表記を削除し正式コマンド化） 00f7207

## SKILL.md 参照更新

track-plan SKILL.md 内の full-cycle 説明を新しいセマンティクスに合わせる。

- [x] SKILL.md の full-cycle 説明を更新 00f7207

## track-signals ラッパー + track:plan 手順修正

Makefile.toml に cargo make track-signals ラッパーを追加。
track:plan コマンド (plan.md) に track-signals → spec-approve の正しい手順を明記。

- [x] cargo make track-signals ラッパー追加 + track:plan コマンドに track-signals → spec-approve の手順を明記 00f7207

## CI 検証

cargo make ci で全チェック通過を確認。

- [x] CI 通過確認 00f7207
