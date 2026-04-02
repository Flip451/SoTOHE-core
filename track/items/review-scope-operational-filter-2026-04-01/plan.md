<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# review_operational パターンによる review.json scope 除外

review-scope.json の review_operational パターンで review.json を diff scope から除外し、multi-group レビューの check-approved が安定動作するようにする。
partition() は変更せず、その前段で operational ファイルをフィルタする。

## Operational Pattern Loading

review-scope.json から review_operational パターンを読み込む。
<track-id> プレースホルダーを実際の track ID で展開する。
TrackId は validated slug なので glob injection は不可能。

- [x] review_operational ローダー + <track-id> placeholder 展開のテスト・実装

## Pre-partition Filter

展開済み operational パターンで diff ファイルリストをフィルタ。
review_adapters.rs の execute() 内の2箇所 + review/mod.rs の check-approved 1箇所に適用。
review_adapters.rs が 994 行のため、ヘルパーは review_group_policy.rs または新モジュールに配置。

- [x] pre-partition フィルタヘルパー: operational パターンにマッチするパスを diff リストから除外するテスト・実装

## Regression Tests

cycle 作成時に review.json が scope に含まれない検証。
連続 record-round で hash が安定する検証。

- [x] 回帰テスト: cycle 作成時に review.json が frozen scope に含まれないことを検証
- [x] 回帰テスト: 連続 record-round で review.json 変更後も other グループの hash が安定することを検証
