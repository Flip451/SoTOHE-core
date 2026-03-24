<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Review escalation enforcement — planning-only bypass guard + record-round wiring

/track:review が record-round を呼ばないため review state が NotStarted のままコミットできるすり抜けを修正。
check-approved の planning-only fast-path に staged diff スコープガードを追加し、
code ファイルが含まれる場合は Approved ステータスを要求する。

既存の domain 層 (RoundType::Fast/Final, ReviewGroupState, check_commit_ready) は変更不要。
CLI と skill の配線が主な作業。

## check-approved bypass guard

check-approved CLI が staged files を planning-only allowlist と照合し、
code ファイルがあれば planning_only=false で usecase を呼び出す。
usecase は planning_only==false のとき NotStarted+empty groups の fast-path を無効化。

- [x] check-approved planning-only bypass guard: staged files vs allowlist → --planning-only flag
- [x] 統合テスト: planning-only bypass が code diff で拒否されることを検証

## record-round wiring + visibility

/track:review skill の Step 2d (verdict aggregation) 後に record-round を呼ぶよう配線。
sotp review status コマンドで per-group 状態を表示。

- [x] /track:review skill を sotp review record-round に配線 (fast/final 各グループ結果を永続化)
- [x] sotp review status CLI コマンド: per-group Fast/Final 状態表示
