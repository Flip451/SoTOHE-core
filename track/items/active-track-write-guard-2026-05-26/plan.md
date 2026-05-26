<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 完了済みトラック保護を frozen から現在ブランチ紐付きバリデーションへ置換

## Tasks (5/5 resolved)

### S1 — usecase type changes: remove frozen field and replace error variants

> Remove PreCommitTypeSignalsOutput.frozen and replace MetadataLoadFailed / ImplPlanLoadFailed with BranchNotFound / BranchMismatch in PreCommitTypeSignalsError.
> This makes the usecase type contract reflect the new branch-based guard semantics (IN-01 / IN-02 / CN-04).

- [x] **T001**: usecase: PreCommitTypeSignalsOutput / PreCommitTypeSignalsError の型変更。PreCommitTypeSignalsOutput から `frozen: bool` フィールドを削除し、PreCommitTypeSignalsError の `MetadataLoadFailed` / `ImplPlanLoadFailed` variant を `BranchNotFound(String)` / `BranchMismatch(String)` に置き換える。既存のテスト (`test_pre_commit_type_signals_error_variants_exist` / `test_pre_commit_type_signals_interactor_delegates`) を新しい variant とフィールドに合わせて更新する。変更は `libs/usecase/src/pre_commit_type_signals.rs` のみに閉じること（CN-02 / catalogue は domain 不変）。 (`372720001f629b19a235d85b5bde9417488fb066`)

### S2 — CLI layer: replace status-based frozen guard with branch-based guard

> Remove ensure_active_track() and ensure_active_track_catalogue() (status-based frozen guards) from signals.rs and catalogue_spec_signals.rs.
> Introduce ensure_branch_matches_track() as the single branch-based validation helper shared by both commands (IN-01 / IN-02 / IN-03 / CN-03).

- [x] **T002**: CLI signals.rs / catalogue_spec_signals.rs: status ベースの frozen guard を branch-based guard に置き換える。`signals.rs` の `ensure_active_track()` を削除し、branch-based validation（`track/<id>` prefix check + track_id suffix match）を同ファイルに `ensure_branch_matches_track(branch: &str, track_id: &str) -> Result<(), CliError>` として導入する。`catalogue_spec_signals.rs` の `ensure_active_track_catalogue()` および `read_track_status_str()` 呼び出しを削除し、代わりに git から現在 branch を読み取って `ensure_branch_matches_track` を呼ぶ。呼び出し元の他の使用箇所（`execute_type_signals_lenient_with_bindings` 内の branch guard はすでに branch-based なので変更不要）。変更は `apps/cli/src/commands/track/tddd/signals.rs` と `apps/cli/src/commands/track/tddd/catalogue_spec_signals.rs` に閉じること（IN-01 / IN-02 / IN-03 / CN-01 / CN-03 / CN-04）。 (`372720001f629b19a235d85b5bde9417488fb066`)

### S3 — make.rs pre-commit: remove done/archived bypass, introduce branch validation

> Remove the done|archived status bypass in run_pre_commit_type_signals and replace it with a branch match check.
> Align dispatch_track_commit_message with the removal of the optional None frozen-skip path (IN-01 / IN-02 / CN-04).

- [x] **T003**: make.rs: run_pre_commit_type_signals の status ベース frozen guard を branch-based guard に置き換える。現在の `read_track_status_str` による `done | archived` チェックを削除し、代わりに `SystemGitRepo::discover().current_branch()` で取得した branch を `track/<track_id>` と照合するバリデーションに変更する。ブランチが一致しない場合は `None` を返すのではなく `Err(CliError::Message(...))` を返す（fail-closed）。`type_signal_bindings_opt` の `None` を使った frozen/guard skip 経路を廃止し、recompute 成功時は bindings を必ず返す形（例: `Result<(ExitCode, Vec<TdddLayerBinding>), CliError>`）へ寄せたうえで、呼び出し元 `dispatch_track_commit_message` の `None` チェックを削除する。PreCommitTypeSignalsOutput.frozen フィールドの参照をすべて削除する。変更は `apps/cli/src/commands/make.rs` に閉じること（IN-01 / IN-02 / IN-03 / CN-01 / CN-04）。 (`372720001f629b19a235d85b5bde9417488fb066`)

### S4 — render.rs: replace is_done_or_archived guard with branch-based validation

> Replace the is_done_or_archived status guard in sync_rendered_views with a branch comparison that rejects renders for tracks whose branch does not match the current git branch (IN-04 / CN-01 / CN-03).

- [x] **T004**: infrastructure render.rs: sync_rendered_views の is_done_or_archived guard を branch-based validation に置き換える。`derived_status` を使った `let is_done_or_archived = matches!(derived_status.as_str(), "done" | "archived")` をブランチ照合ロジックに変更する。具体的には：(1) `SystemGitRepo::discover_from(root)` で現在の git ブランチを取得し、(2) `track/<id>` 形式のブランチと `metadata.json` の `branch` フィールドを照合し、(3) 一致しない場合は `spec.md` / `<layer>-types.md` レンダリングをスキップするのではなく `RenderError::InvalidTrackMetadata` を返す（fail-closed、CN-01）。plan.md は現行通り guard の外でレンダリングし続ける。変更は `libs/infrastructure/src/track/render.rs` に閉じること（IN-04 / CN-01 / CN-03）。 (`372720001f629b19a235d85b5bde9417488fb066`)

### S5 — Branch-based validation tests + CI gate

> Add unit tests for the branch-based guard in all four changed guard surfaces (signals.rs, catalogue_spec_signals.rs, make.rs, render.rs).
> Verify cargo make ci passes as the final acceptance gate (IN-05 / AC-06).

- [x] **T005**: テスト追加 + CI gate。IN-05 のブランチベースバリデーションテスト追加：(a) branch が `track/<track_id>` と一致するケース（許容）、(b) branch が `track/<other_id>` と一致しないケース（拒否）、(c) branch が `track/` prefix を持たないケース（拒否）。対象モジュール：`signals.rs` の `ensure_branch_matches_track` 単体テスト（T002）、`catalogue_spec_signals.rs` の branch-based guard 単体テスト（T002）、`make.rs` の `run_pre_commit_type_signals` branch guard / fail-closed 経路テスト（T003）、`render.rs` の branch-based guard テスト（T004、`render_tests.rs` に追加）。最後に `cargo make ci`（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass することを確認する（AC-06）。 (`372720001f629b19a235d85b5bde9417488fb066`)
