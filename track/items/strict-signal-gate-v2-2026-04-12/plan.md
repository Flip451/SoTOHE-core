<!-- Generated from metadata.json — DO NOT EDIT DIRECTLY -->
# Strict Spec Signal Gate v2 — Yellow blocks merge (fail-closed)

ADR 2026-04-12-1200 (Strict Spec Signal Gate v2) の実装。CI = interim mode (yellow を warning 可視化 + PASS) / merge gate = strict mode (yellow ブロック) の二層モード設計。
domain / usecase / infrastructure / cli の 4 層分離: pure check rules を domain に、orchestration + port を usecase に、adapter と git_cli::show プリミティブを infrastructure に、thin composition を CLI に配置。
既存の check_tasks_resolved も同じ TrackBlobReader port を共有する usecase 配置に consolidate し、CLI 層から git 直呼び出しを完全に排除。既存 reject_symlinks_below を再利用して symlink / submodule を fail-closed で拒絶。
前回試行 (track/strict-signal-gate-2026-04-12, PR #92 close 済み) の教訓を反映: 計画を飛ばさず、fail-closed 真理値表を事前確定し、Planner レビューを経てから実装する。

## domain: Feedback → Yellow 降格

SignalBasis::Feedback を Blue から Yellow に再マッピングする (D1)。
domain::signal のユニットテスト (rstest cases) を更新し、新しいマッピングを反映する。
infrastructure::verify::spec_signals の関連 fixture / テストも feedback を Yellow 扱いに揃える。

- [x] domain: SignalBasis::Feedback → Yellow 降格 + domain::signal / infrastructure::verify::spec_signals のテスト更新 36ca580022f90a742cca9897b5cec0b32f8a4da4

## domain: check_spec_doc_signals 追加

check_spec_doc_signals(&SpecDocument, strict: bool) -> VerifyOutcome を libs/domain/src/spec.rs に追加する (D2 / D5.1)。
strict=true では yellow を Finding::error、strict=false では yellow を Finding::warning として emit する (D8.6)。
Red / None / all-zero は strict に関わらず常に Finding::error。
D1–D6 の domain 層ユニットテストを追加する。

- [x] domain: check_spec_doc_signals(&SpecDocument, strict: bool) -> VerifyOutcome を libs/domain/src/spec.rs に追加 + D1–D6 tests (strict=false で yellow を Finding::warning として emit) 9302f2fdbcb1d7b625279047d592e1b18f2547ce

## domain: check_domain_types_signals 追加

check_domain_types_signals(&DomainTypesDocument, strict: bool) -> VerifyOutcome を libs/domain/src/tddd/catalogue.rs に追加する。
entries 非空 / coverage 完全 / Red / Yellow (strict で error、non-strict で warning + 型名リスト) を検査する。
D7–D13 の domain 層ユニットテストを追加する。

- [x] domain: check_domain_types_signals(&DomainTypesDocument, strict: bool) -> VerifyOutcome を libs/domain/src/tddd/catalogue.rs に追加 + D7–D13 tests (Red/空/coverage 常に error, Yellow は strict で error / non-strict で warning) 7a0db96d3220426aa5b6ae19737710c3014fbb0b

## domain: validate_branch_ref + RefValidationError 新設

libs/domain/src/git_ref.rs を新設し、validate_branch_ref(&str) -> Result<(), RefValidationError> を追加する (D2.0 / D4.2)。
RefValidationError enum (DisallowedCharacter / Empty) を thiserror で定義する。
禁止文字: .., @{, ~, ^, :, 空白, 制御文字。
D14–D22 の domain 層ユニットテストを追加する。

- [ ] domain: libs/domain/src/git_ref.rs 新設 + validate_branch_ref + RefValidationError (DisallowedCharacter / Empty) + D14–D22 tests (.., @{, ~, ^, :, 空白, 制御文字を拒否)

## infra: verify_from_spec_json リファクタ + symlink 拒絶

verify_from_spec_json を thin wrapper にリファクタし、T002 / T003 の domain 純粋関数に delegate する。
Stage 2 NotFound を BLOCKED から skip に変更する (D2.1)。
std::fs::read_to_string 直前に libs/infrastructure/src/track/symlink_guard.rs::reject_symlinks_below を呼ぶ (D4.3 CI 経路)。
既存テストを更新し、test_verify_from_spec_json_with_missing_domain_types_returns_error を削除、S1–S5 (symlink 拒絶) テストを追加する。

- [ ] infra: verify_from_spec_json を T002/T003 に delegate する thin wrapper にリファクタ + Stage 2 NotFound → skip + reject_symlinks_below integration (D4.3 CI 経路) + S1–S5 tests + 旧 missing_domain_types テスト削除

## usecase: merge_gate モジュール + TrackBlobReader port

libs/usecase/src/merge_gate/ モジュールを新設する (D5.2)。
BlobFetchResult<T> enum (Found/NotFound/FetchError) と TrackBlobReader port trait (read_spec_document / read_domain_types_document / read_track_metadata) を定義する。
check_strict_merge_gate(&str, &impl TrackBlobReader) -> VerifyOutcome を実装する (strict=true 固定)。
U1–U18 の MockTrackBlobReader を使った分岐網羅テストを追加する。

- [ ] usecase: libs/usecase/src/merge_gate/ 新設 — BlobFetchResult<T> + TrackBlobReader port (read_spec_document / read_domain_types_document / read_track_metadata) + check_strict_merge_gate orchestration + U1–U18 MockTrackBlobReader tests

## usecase: task_completion モジュール

libs/usecase/src/task_completion.rs を新設する (D9)。
check_tasks_resolved_from_git_ref(&str, &impl TrackBlobReader) -> VerifyOutcome を実装する。
plan/ branch skip / validate_branch_ref / read_track_metadata / all_tasks_resolved 検査を行う。
K1–K7 の MockTrackBlobReader テストを追加する。

- [ ] usecase: libs/usecase/src/task_completion.rs 新設 — check_tasks_resolved_from_git_ref (plan/ skip + validate_branch_ref + read_track_metadata + all_tasks_resolved) + K1–K7 MockTrackBlobReader tests

## infra: git_cli::show プリミティブ (symlink 拒絶対応)

libs/infrastructure/src/git_cli/show.rs を新設する (D5.3)。
BlobResult enum (Found<Vec<u8>>/NotFound/CommandFailed)、TreeEntryKind enum (RegularFile/Symlink/Submodule/Other/NotFound)、git_show_blob、git_ls_tree_entry_kind、fetch_blob_safe、is_path_not_found_stderr を pub(crate) で実装する。
LANG=C LC_ALL=C LANGUAGE=C を Command の env に必ず設定する (D4.1)。
fetch_blob_safe は git_ls_tree_entry_kind でモード検査 → git_show_blob の 2 段呼び出し (D4.3)。
プリミティブのユニットテストを追加する。

- [ ] infra: libs/infrastructure/src/git_cli/show.rs 新設 — BlobResult / TreeEntryKind / git_show_blob / git_ls_tree_entry_kind / fetch_blob_safe / is_path_not_found_stderr (pub(crate), LANG=C 固定, 2 段 symlink 検査)

## infra: GitShowTrackBlobReader adapter

libs/infrastructure/src/verify/merge_gate_adapter.rs を新設し、GitShowTrackBlobReader struct を定義する (D5.3)。
TrackBlobReader port の 3 メソッド (read_spec_document / read_domain_types_document / read_track_metadata) を実装する。
内部で fetch_blob_safe を呼び、BlobResult → BlobFetchResult<T> への変換、UTF-8 decode、JSON decode を行う。
A1–A16 の実 git repo fixture テスト (symlink / submodule commit 含む) を追加する。

- [ ] infra: libs/infrastructure/src/verify/merge_gate_adapter.rs 新設 — GitShowTrackBlobReader が TrackBlobReader port を実装 (fetch_blob_safe → BlobFetchResult<T> 変換 + codec decode) + A1–A16 tests (symlink/submodule fixture 含む)

## cli: pr.rs thin wrapper 化

apps/cli/src/commands/pr.rs::wait_and_merge を薄いラッパー化する。GitShowTrackBlobReader::new(repo_root) を構築し、task_completion / merge_gate を順次呼ぶ。
既存 check_tasks_resolved を task_completion::check_tasks_resolved_from_git_ref への thin wrapper に書き換える。
既存の check_tasks_resolved_* テストを削除し、C1–C4 の最小 wrapper テストのみ残す。
CLI 層から std::process::Command::new("git") の直呼び出しが完全に消えたことを確認する。

- [ ] cli: apps/cli/src/commands/pr.rs::wait_and_merge + check_tasks_resolved を GitShowTrackBlobReader + usecase::merge_gate / task_completion の thin wrapper に書き換え + 既存テスト移植 + C1–C4 最小 wrapper tests

## docs: source-attribution convention 更新

knowledge/conventions/source-attribution.md に Signal 列を追加する。
Blue ソース (document, convention) と Yellow ソース (feedback, inference, discussion) を明示する。
feedback が Yellow に降格した理由と upgrade (ADR 作成) ガイダンスを追記する。

- [ ] docs: knowledge/conventions/source-attribution.md に Signal 列追加 + feedback Yellow 降格の upgrade ガイダンス追記

## build: Makefile.toml CI interim mode 組み込み

Makefile.toml に verify-spec-states-current-local タスクを新設する (D8.2)。
branch-bound な track 解決: track/* ブランチなら sotp verify spec-states <path> を interim mode で実行、plan/* / main / その他は skip ログ。
ci-local / ci-container の dependencies に verify-spec-states-current-local を追加する (D8.4)。
--strict は付けない (CI は interim mode)。

- [ ] build: Makefile.toml に verify-spec-states-current-local 新設 (branch-bound, interim mode, --strict なし) + ci-local / ci-container dependencies に追加

## docs: CI 統合回帰テスト記録

I1–I11 の CI 統合回帰テストを手動実行し、結果を verification.md に記録する。
本 track の cargo make ci が PASS / yellow 可視化 warning が CI ログに出る / red は BLOCKED / main / plan/* は skip / merge gate は strict で yellow BLOCKED の各シナリオを確認する。

- [ ] docs: CI 統合回帰テスト I1–I11 を手動実行 + verification.md に結果記録

## docs: ADR Accepted 化

knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md の Status を Proposed から Accepted に更新する。
実装完了 + CI 通過 + レビュー完了後に実施する最終ステップ。

- [ ] docs: knowledge/adr/2026-04-12-1200-strict-spec-signal-gate-v2.md を Proposed → Accepted に更新
