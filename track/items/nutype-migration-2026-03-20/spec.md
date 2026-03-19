# Spec: Nutype Migration

## Goal

6 つの手書き String newtype を `nutype` マクロ宣言に置き換え、ボイラープレート（Display impl, as_str, new コンストラクタ）を約 120 行削減する。

## Scope

- `TrackId`, `TaskId`, `CommitHash`, `TrackBranch` → nutype + 既存 validation 関数 [source: ids.rs]
- `NonEmptyString` → nutype + sanitize(trim) [source: ids.rs]
- `ReviewConcern` → nutype + sanitize(trim, lowercase) [source: review.rs]
- 全レイヤーの呼び出し元: `new()` → `try_new()` 移行

## Scope 外

- `Timestamp` — 多フィールド構造体（nutype 非対応） [source: Gemini 調査]
- `CodeHash` — enum 型（nutype 非対応）
- `Verdict`, `ReviewStatus`, `RoundType` — enum 型（strum で対応済み）

## Constraints

- nutype 0.6 を使用。serde feature は不要 [source: Gemini 調査]
- 既存の validation 関数 (`is_valid_track_id` 等) をそのまま再利用する [source: prefer-type-safe-abstractions.md]
- `as_str()` は `AsRef<str>` derive で代替。既存の `as_str()` 呼び出しは `.as_ref()` に変更
- 既存テスト（1068 件）を壊さないこと

## Acceptance Criteria

1. 6 型すべてが `#[nutype(...)]` マクロで宣言されている
2. 手書きの `Display` impl, `as_str()` メソッド, `new()` コンストラクタが削除されている
3. 全呼び出し元が `try_new()` を使用している
4. `cargo make ci` が通過する
5. ids.rs の行数が 366 行から 250 行以下に減少する

## Related Conventions (Required Reading)

- `project-docs/conventions/prefer-type-safe-abstractions.md`
