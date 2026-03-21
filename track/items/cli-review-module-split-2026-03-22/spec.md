# spec: cli-review-module-split

## Goal

`apps/cli/src/commands/review.rs` (1825行) を責務別に 4ファイルのモジュールディレクトリに分割し、
各ファイルを 700行以下に収める。ロジック変更なし、純粋な構造リファクタリング。

## Scope

### IN scope

1. `review.rs` → `review/` ディレクトリ化
2. `review/mod.rs`: clap structs, dispatch, thin run_* wrappers (~170行)
3. `review/codex_local.rs`: subprocess管理 — spawn, tee, terminate, poll, artifacts (~480行)
4. `review/adapters.rs`: port trait impl (CliRecordRoundStore 等) (~340行)
5. `review/tests.rs`: 全テスト (~735行)

### OUT of scope

- ロジック変更・リファクタリング
- テストの追加・削除
- 他ファイルの変更（`commands/mod.rs` の `mod review` 宣言は自動対応）

## Constraints

- 純粋なコード移動のみ（ロジック変更禁止）
- `mod.rs`, `codex_local.rs`, `adapters.rs` は各 700行以下
- 既存テスト全 pass
- CLI サブコマンド動作不変

## Related Conventions (Required Reading)

None

## Acceptance Criteria

1. `apps/cli/src/commands/review/mod.rs` が ~170行以下
2. `apps/cli/src/commands/review/codex_local.rs` が ~480行（700行以下）
3. `apps/cli/src/commands/review/adapters.rs` が ~340行（700行以下）
4. `apps/cli/src/commands/review/tests.rs` にテスト集約
5. `cargo make ci` が通る
6. 既存テスト全 pass
