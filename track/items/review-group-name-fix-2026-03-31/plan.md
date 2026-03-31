<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# RVW-37: review.md グループ名 infra → infrastructure 統一

review.md (SKILL.md) のグループ名 'infra' を track/review-scope.json の正式名 'infrastructure' に統一する。
Rust コード変更なし。SKILL.md 内の 3 箇所を置換するのみ。

## グループ名統一

review.md 内の 3 箇所で 'infra' を 'infrastructure' に置換。
briefing ファイル名も briefing-infrastructure.md に変更。

- [x] review.md のグループ分類テーブルで infra → infrastructure に変更
- [x] review.md の cargo make track-local-review 呼び出し例で --group infra → --group infrastructure に変更
- [x] review.md のサマリー出力例で infra-domain → infrastructure 等の表記を修正
