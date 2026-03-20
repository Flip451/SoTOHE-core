# verification: review-usecase-extraction (CLI-02)

## Scope Verified

- spec.md の acceptance criteria 1-10 をカバー

## Manual Verification Steps

1. `cargo make ci` が全て pass すること
2. 以下のコマンドが既存と同じ動作をすること:
   - `bin/sotp review record-round --round-type fast --group test --verdict '...' --expected-groups test --track-id <id> --items-dir track/items`
   - `bin/sotp review check-approved --track-id <id> --items-dir track/items`
   - `bin/sotp review resolve-escalation --track-id <id> --items-dir track/items ...`
   - `bin/sotp review codex-local --model <model> --prompt "test"`
3. 全対象ファイルが 700行以下であること:
   - `libs/domain/src/review/*.rs` — 各ファイル <700行
   - `libs/usecase/src/review_workflow/*.rs` — 各ファイル <700行
   - `libs/infrastructure/src/git_cli/private_index.rs` — <700行
   - `apps/cli/src/commands/review.rs` — <700行
   - 注: `libs/infrastructure/src/git_cli/mod.rs` は既存 1100行超だが本トラックのスコープ外（PrivateIndex 抽出のみ）
4. `cargo make check-layers` が pass すること（レイヤー依存違反なし）
5. アーキテクチャ検証（spec.md acceptance criteria 5）:
   - `apps/cli/src/commands/review.rs` の非テストコード（`#[cfg(test)]` 外）に `domain::` / `infrastructure::` への直接参照（use 文・完全修飾パス共）が残っていないこと（usecase 経由のみ許容。テストコードは除外）
   - `libs/usecase/src/review_workflow/usecases.rs` の各 UseCase が Load → domain 呼び出し → Save のパターンのみであること（ビジネスロジック分岐なし）
   - `run_record_round` / `run_resolve_escalation` / `run_check_approved` の本体が UseCase 呼び出し + エラーマッピングのみであること
   - `extract_verdict_from_session_log` が `libs/usecase/src/review_workflow/` 内に存在し、file read + `domain::extract_verdict_from_content` 呼び出しの薄いオーケストレーターであること（追加のパースロジックを含まない）
6. serde 制約検証（spec.md constraints）:
   - `libs/domain/Cargo.toml` の `[dependencies]` に `serde` / `serde_json` が追加されていないこと
   - `cargo make deny` が pass すること

## Result / Open Issues

- (未実施)

## Verified At

- (未実施)
