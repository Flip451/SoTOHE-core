<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 完了済みトラック保護を frozen から現在ブランチ紐付きバリデーションへ置換

## Tasks (5/8 resolved)

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

### S6 — plan-only / activate lane removal: source code + usecase deletion

> Delete execute_activate and all activation-specific helpers from activate.rs; give BranchAction::Switch its own simple git-switch implementation.
> Delete libs/usecase/src/track_activation.rs and worktree_guard.rs. Remove TrackCommand::Activate, ActivateArgs, BranchMode, MakeTask::TrackActivate, MakeTask::TrackPlanBranch, and the corresponding Makefile.toml tasks.
> This is a tightly-coupled compile unit: all deletions must land together for cargo make ci to pass (IN-07, IN-08, IN-09, IN-10, CN-05, AC-08, AC-09, AC-10, AC-11, AC-12).

- [ ] **T006**: CLI + usecase: plan-only / activate レーンのコアコード削除（コンパイル単位として一括）。(1) `apps/cli/src/commands/track/activate.rs`: `BranchAction::Switch` に独立した `execute_branch_switch` 実装（既存の `branch_exists` + `preflight_branch_operation` + `git switch` のみ）を追加し、`execute_branch` の Switch アームをそこへ差し替える。`execute_activate` および activation 専用ヘルパー群（`activation_*` 関数群、`allow_materialized_activation`、`should_persist_activation_side_effects`、`activation_requires_clean_worktree`、`allowed_activation_dirty_paths`、`persist_activation_commit`、`ensure_clean_worktree`、`git_dirty_worktree_paths`、`activation_artifacts_dirty`、`find_latest_activation_commit`、`is_ancestor`、`activation_switch_label`）を削除する。`reject_branchless_guard_*` の 3 つのテストを `transition.rs` の `#[cfg(test)]` ブロックへ移動する（これらは `track_resolution` のテストであり activate 固有でない）。(2) `apps/cli/src/commands/track/mod.rs`: `TrackCommand::Activate` variant・`ActivateArgs` struct・`BranchMode` enum・`derive_track_status_from_json` 関数（activate 専用）を削除。`use usecase::track_activation::{ActivateTrackOutcome, ActivateTrackUseCase}` import と `TrackCommand::Activate` dispatch arm を削除。(3) `apps/cli/src/commands/make.rs`: `MakeTask::TrackActivate`・`MakeTask::TrackPlanBranch` variant・`dispatch_track_activate`・`dispatch_track_plan_branch` 関数を削除し、`run` マッチアームも除去。(4) `libs/usecase/src/track_activation.rs` を削除。`libs/usecase/src/worktree_guard.rs` を削除。`libs/usecase/src/lib.rs` から `pub mod track_activation;` と `pub mod worktree_guard;` を削除。(5) `Makefile.toml` から `[tasks.track-activate]` と `[tasks.track-plan-branch]` を削除。`cargo make ci` pass を確認（AC-08 / AC-09 / AC-10 / AC-11 / AC-12 / CN-05）。

### S7 — plan-only / activate lane removal: command files + doc references

> Delete .claude/commands/track/plan-only.md and .claude/commands/track/activate.md.
> Remove all /track:plan-only and /track:activate references from DEVELOPER_AI_WORKFLOW.md and the affected .claude/commands/track/*.md files (IN-06 partial, AC-07).

- [ ] **T007**: コマンド定義ファイル削除 + ドキュメント参照クリーンアップ。(1) `.claude/commands/track/plan-only.md` と `.claude/commands/track/activate.md` を削除（AC-07）。(2) `DEVELOPER_AI_WORKFLOW.md`: mermaid フローチャートの plan-only / activate ノードとエッジ、§0.4 コマンド比較表の `/track:plan-only` / `/track:activate` 行、§3.1 「plan-only 代替レーン」ブロックを削除。(3) `.claude/commands/track/done.md`: 「no active tracks → /track:plan-only」推奨文言を削除。(4) `.claude/commands/track/status.md`: 「branchless planning-only → /track:activate」推奨ルールを削除。(5) `.claude/commands/track/implement.md` / `full-cycle.md`: 「planning-only tracks must pass through /track:activate」参照を削除。(6) `.claude/commands/track/pr.md`: 「For plan/ branches: merge the PR, then /track:activate」ヒントを削除。(7) `apps/cli/src/commands/pr.rs`: `/track:activate` ヒント・エラーテキストを削除（存在する場合）。`cargo make ci` pass を確認（AC-07 を検証）。

### S8 — residual footprint expunge: plan/ branch handlers, NextCommand::ActivateTrack + TrackPhase::ReadyToActivate, skill-compliance entries, permissions, error-message refs, and coupled tests

> Remove plan/<id> branch handling from pr_workflow.rs / merge_gate.rs / task_completion.rs / track_resolution.rs (IN-11).
> Delete NextCommand::ActivateTrack from the NextCommand enum and TrackPhase::ReadyToActivate from the TrackPhase enum, plus all match arms referencing either variant (IN-12).
> Remove skill-compliance detector entries for /track:plan-only and /track:activate (IN-13).
> Remove track-activate / track-plan-branch permission entries from .claude/settings.json and orchestra.rs (IN-14).
> Purge /track:activate, /track:plan-only, track-activate, track-plan-branch references from error messages, render text, and doc files (IN-15).
> Delete all tests coupled to the removed code (IN-16).
> Fix transition.rs clippy items-after-test-module ordering error introduced during T006 test relocation.
> T006 + T007 + T008 are implemented as one tightly-coupled bundle; AC-13 grep zero-hit check and cargo make ci are the final gates (AC-13).

- [ ] **T008**: residual lane footprint の完全削除 + transition.rs clippy 修正（T006 + T007 + T008 の一括 ci gate）。(1) clippy 修正: `apps/cli/src/commands/track/transition.rs` で T006 移動後の `reject_branchless_guard_*` テストが `execute_transition` 本番関数より前に配置されている items-after-test-module エラーを修正する（テストブロックをファイル末尾へ移動）。(2) IN-11: `pr_workflow.rs` / `merge_gate.rs` / `task_completion.rs` / `track_resolution.rs` 等の共有ユースケースモジュールから `plan/<id>` ブランチ判定・分岐・関連エラーパスを削除する。(3) IN-12: `libs/domain/src/track_phase.rs` にある 2 つの enum から activate 関連 variant をそれぞれ削除する: `NextCommand` enum から `NextCommand::ActivateTrack` variant を削除し、`TrackPhase` enum から `TrackPhase::ReadyToActivate` variant を削除する。削除後は、これらの variant を参照しているすべてのマッチアームおよびパターンも合わせて除去する。(4) IN-13: `libs/domain/src/skill_compliance/mod.rs`（または同等モジュール）の skill-compliance 検出エントリのうち `/track:plan-only` と `/track:activate` に対応するものを削除する。(5) IN-14: `.claude/settings.json` の `permissions.allow` から `track-activate` / `track-plan-branch` エントリを削除し、`apps/cli/src/commands/verify/orchestra.rs`（または同等の検証コード）からも対応するパーミッションエントリを削除する。(6) IN-15: `git_cli` / `git_workflow` / `track_resolution` / `pr_workflow` / `render.rs` 等のエラーメッセージ・レンダーテキスト、および `.claude/commands/track/plan.md` / `.claude/rules/07-dev-environment.md` ドキュメント内の `/track:activate` / `/track:plan-only` / `track-activate` / `track-plan-branch` 参照を除去する。(7) IN-16: IN-11〜IN-15 の削除対象コードに紐づくすべてのテスト（`plan/` ブランチハンドリング、`NextCommand::ActivateTrack` および `TrackPhase::ReadyToActivate`、skill-compliance activate/plan-only エントリ、パーミッションエントリ、activate 参照文字列を検証するテスト）を削除する。(8) AC-13: リポジトリ全体 grep で `/track:activate` / `/track:plan-only` / `track-activate` / `track-plan-branch` / `execute_activate` / `track_activation` / `worktree_guard` が `knowledge/adr/**` とこのトラック自身の成果物以外でヒットゼロになることを確認したうえで `cargo make ci` を実行して pass することを確認する。T006 + T007 + T008 は互いに密結合したコンパイル単位として一括実装する。
