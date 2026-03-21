# spec: review-usecase-extraction (CLI-02)

## Goal

`apps/cli/src/commands/review.rs` (2317行) に集中した domain/infrastructure 直接操作ロジックを
適切なレイヤーに移動し、本トラックで新規作成・分割するファイルを 700行以下に収める。

## Design Principle

- **domain（厚い）**: 全ビジネスロジックを集中。純粋関数・型・状態遷移はすべて domain に置く
- **usecase（薄い）**: domain オブジェクトの組み合わせ（Load → domain 呼び出し → Save）のみ
- **CLI（薄い）**: clap structs + subprocess 管理 + usecase 呼び出し + ExitCode mapping のみ
- **infrastructure**: I/O 実装詳細（git index 操作、ファイルシステム等）

## Scope

### IN scope

1. `PrivateIndex` + `resolve_real_index_path` → `infrastructure::git_cli::private_index` に移動
2. `domain::review.rs` (2375行) → モジュールディレクトリ化（6ファイル分割）
3. `usecase::review_workflow.rs` (936行) → モジュールディレクトリ化（4ファイル分割）+ UseCase 新設
4. CLI `run_record_round` / `run_resolve_escalation` / `run_check_approved` → UseCase 呼び出しに圧縮
5. 純粋ロジックの domain 集約:
   - `classify_review_verdict` (usecase → domain、シグネチャを domain 型に変更)
   - `resolve_full_auto` + `ModelProfile` (usecase → domain)
   - `file_path_to_concern` (usecase → domain)
   - `extract_verdict_from_content` (CLI → domain、`&str` を受け取る純粋関数に分離)
6. `agent-profiles.json` I/O の infrastructure 分離:
   - `AgentProfiles` / `ProviderConfig` serde 型 + ファイル読み込み → infrastructure
   - `resolve_full_auto_from_profiles` は infrastructure I/O + domain 純粋関数の組み合わせに再構成
7. デッドコード削除: `ReviewGroupState::from_legacy_final_only`（呼び出し箇所ゼロ）
8. `extract_verdict_from_session_log` を usecase に移動（file read + domain 純粋関数呼び出しの薄いオーケストレーター。CLI は usecase 経由で呼び出す）

### OUT of scope

- CLI `pr.rs` の分割 (CLI-01: 別トラック)
- CLI `activate.rs` の分割 (ERR-09b: 別トラック)
- GAP-03: `Verdict` / `ReviewPayloadVerdict` の統合 (別トラック)
- domain に serde 依存を追加する変更
- infrastructure の `codec.rs` / `orchestra.rs` / `git_cli.rs` 本体等の分割（`git_cli/mod.rs` は T001 で PrivateIndex を抽出するのみ。残りの 1100行超の分割は別トラック）

## Constraints

- domain 層に serde / serde_json を追加しない
- `ReviewFinalPayload`, `ReviewFinding`, `ReviewPayloadVerdict` 等の serde 依存型は usecase に残す
- `REVIEW_OUTPUT_SCHEMA_JSON` は外部ツール設定なので usecase に残す
- 既存の公開 API (`sotp review record-round` 等) の CLI インターフェースは変更しない
- `architecture-rules.json` の `module_limits.max_lines: 700` を全対象ファイルで遵守

## Related Conventions (Required Reading)

- `project-docs/conventions/prefer-type-safe-abstractions.md`
- `project-docs/conventions/typed-deserialization.md`

## Acceptance Criteria

1. `apps/cli/src/commands/review.rs` の本番コードが ~700行以下
2. `domain::review` が 6ファイルに分割され、全ファイル 700行以下
3. `usecase::review_workflow` が 4ファイルに分割され、全ファイル 700行以下
4. `infrastructure::git_cli` がモジュールディレクトリ化され、`private_index.rs` が独立かつ 700行以下（`git_cli/mod.rs` の既存 1100行超の分割はスコープ外）
5. 全ビジネスロジックが domain に集約（usecase は Load → domain → Save のみ）
6. `cargo make ci` が通る
7. 既存テストが全て pass（テストの移動は許容）
8. CLI のサブコマンド動作が変わらない（外部インターフェース不変）
9. CLI `review.rs` の非テストコード（`#[cfg(test)]` 外）に `domain::` / `infrastructure::` への直接参照（use 文・完全修飾パス共）が残らない（usecase 経由のみ。テストコードは除外）
10. `libs/domain/Cargo.toml` に serde / serde_json が追加されていない
