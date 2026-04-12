# Verification — Review System V1 完全撤去

## Scope verified

- [x] T001: ADR 作成 (Proposed, planning commit 24faabe で実施済み) + `.claude/rules/10-guardrails.md` Escalation Threshold セクション削除 + `knowledge/adr/README.md` 索引追加 + `knowledge/strategy/TODO.md` の RVW-54 から set-approved-head 言及のみ削除 + `knowledge/DESIGN.md` の V1 review 2 セクション (WF-36 Review Escalation Threshold + tamper-proof-review-2026-03-26 Tamper-Proof Review Verdict) 削除 + `knowledge/conventions/hexagonal-architecture.md` RecordRoundProtocol/ReviewJsonReader 参照置換 + `knowledge/conventions/security.md` FsReviewJsonStore 参照置換 (RVW-52/53/55 の obsolete/done マークは T002 コミットで; RV2-07 の done マークは T005 コミットで)
- [x] T002: CLI `ResolveEscalation` / `SetApprovedHead` subcommand 削除 + `ResolveEscalationArgs` / `SetApprovedHeadArgs` struct 削除 + dispatcher 分岐削除 + `run_check_approved` 内 escalation fail-closed gate 削除 (V1 `track.review().escalation()` 参照除去) + `make_timestamp` helper 削除 (V1 resolve_escalation 専用だった) + `make.rs::persist_approved_head` 関数 + `dispatch_commit_from_file` 内呼び出し削除
- [x] T003: `libs/usecase/src/review_workflow/usecases.rs` (1700+ 行 — `RecordRoundProtocol` trait + `record_round` + `record_round_typed` + `resolve_escalation` + `RecordRoundInput` + `RecordRoundError` + `RecordRoundProtocolError` + `ResolveEscalationInput` + validation helpers + 全テスト) ファイル削除 + `libs/usecase/src/review_workflow/scope.rs` (550+ 行 — `DiffScope` + `DiffScopeProvider` + `FindingScopeClass` + `RepoRelativePath` + `ScopeFilterResult` + `ScopeFilteredPayload` + `apply_scope_filter` + `classify_finding_scope` + `partition_findings_by_scope` + 全テスト) ファイル削除 + `libs/usecase/src/review_workflow/mod.rs` prune (`mod usecases;` / `mod scope;` 宣言と関連 `pub use` 削除、`verdict` のみ残す) + `libs/usecase/src/review_workflow/verdict.rs` 行 185/193 の `domain::review::extract_verdict_json_candidates_compact/_multiline` 呼び出しを `domain::review_v2::extract_verdict_json_candidates_compact/_multiline` に切替 (V2 側に等価実装あり)
- [x] T004: `libs/infrastructure/src/git_cli/mod.rs::GitRepository::index_tree_hash_normalizing` trait method + `SystemGitRepo::index_tree_hash_normalizing` 実装削除 (~200 行) + V1 integration tests 7 本 + V1 unit tests 3 本 + `rrz` test helper + `init_git_repo_with_identity` helper + `write_full_metadata` / `setup_integration_repo` helpers 削除 (448 行) + unused imports (`Stdio`, `std::io::Write as _`) 除去 + `libs/infrastructure/src/git_cli/private_index.rs` ファイルごと削除 (PrivateIndex 全体、約 400 行、外部参照ゼロ) + `git_cli/mod.rs` から `pub mod private_index;` 除去 + `libs/infrastructure/src/review_json_codec.rs` ファイルごと削除 (V1 review.json codec、~750 行) + `libs/infrastructure/src/review_json_store.rs` ファイルごと削除 (FsReviewJsonStore、~310 行) + `libs/infrastructure/src/lib.rs` から `pub mod review_json_codec;` / `pub mod review_json_store;` 除去 + `libs/infrastructure/src/track/codec.rs` の `TrackDocumentV2.review` field + V1 review DTO tree (`TrackReviewDocument`, `ReviewGroupDocument`, `ReviewRoundDocument`, `TrackReviewEscalationDocument`, `EscalationPhaseDocument`, `ReviewCycleDocument`, `ConcernStreakDocument`, `ResolutionDocument`) + V1 codec helpers (`review_from_document`, `review_to_document`, `escalation_from_document`, `escalation_to_document`, `escalation_phase_from_document`, `resolution_from_document`, `round_result_from_document`, `round_result_to_document`, `parse_timestamp`, `parse_round_type`, `parse_escalation_decision`, `escalation_decision_to_str`, `parse_review_status`) + V1 codec tests 22 本 削除 + V1 review 型 import 一掃 (`#[serde(flatten)] extra` は保持して既存 30 metadata.json の review フィールドは opaque JSON として素通し)
- [ ] T005: domain `review/` 全 V1 削除 + `RoundType` を `review_v2/types.rs` に移設 + `TrackMetadata.review` field 削除 + `ReviewJsonReader/Writer` ports 削除 + CI / baseline 検証 + ADR を Accepted に更新

## Manual verification steps

### 層境界 / ビルド検証

- [ ] `cargo check` が T002 完了時点 (CLI のみ除去) で通る
- [ ] `cargo check` が T003 完了時点 (usecase 除去) で通る
- [ ] `cargo check` が T004 完了時点 (infra 除去) で通る
- [ ] `cargo make ci` が T005 完了時点 (domain 除去 + RoundType 移設完了) で PASS する (fmt-check + clippy + test + deny + check-layers + verify-*)
- [ ] `cargo make deny` が通る
- [ ] `cargo make check-layers` が通る (domain ← usecase ← infrastructure ← cli 依存順を維持)

### TDDD-01 ブロッカー解消の回帰確認 (T005 の acceptance criteria)

- [ ] `bin/sotp track baseline-capture --force <some-active-track-id>` を実行し、ログに `same-name type collision for ReviewState` warning が **出ない** ことを確認する
  - 補足: 本トラックが対象とするのは `ReviewState` 衝突のみ。`domain::verify::Finding` vs `domain::review_v2::Finding` 等、本トラックのスコープ外の同名衝突が引き続き warning に出る場合は別トラックで扱う (このトラックの accept/reject には影響しない)
- [ ] `bin/sotp track domain-type-signals <some-active-track-id>` を実行し、出力の `baseline_changed_type` セクションに `ReviewState` エントリが **出ない** ことを確認する (本トラックの直接目的)

### grep 回帰検査 (dead reference ゼロ)

以下のすべてのコマンドが結果 0 件であること:

- [ ] `grep -r 'ReviewState' libs/domain/src/ --include='*.rs' | grep '/review/' | grep -v '/review_v2/'` — 結果 0 件であること (V1 `ReviewState` の参照が残っていない; V2 は `libs/domain/src/review_v2/` なので除外する; T005 完了後は `libs/domain/src/review/` ディレクトリ自体が消えているため `grep -r ... libs/domain/src/review/` 形式はパス不在でエラーになる)
- [ ] `grep -rn 'persist_approved_head' apps/ libs/`
- [ ] `grep -rn 'set_approved_head\|SetApprovedHead' apps/ libs/`
- [ ] `grep -rn 'index_tree_hash_normalizing\|normalized_tree_hash' apps/ libs/`
- [ ] `grep -rn 'RecordRoundProtocol\|record_round\b' apps/ libs/` を実行し、出力が **コメント行のみ** であることを確認する (T005 完了後、`apps/cli/src/commands/review/codex_local.rs:46` と `apps/cli/src/commands/review/tests.rs:791` は `//` コメント行として残存することが想定される。それ以外に非コメント行での参照が存在する場合は削除漏れ)
- [ ] `grep -rn 'resolve_escalation\|ResolveEscalation' apps/ libs/`
- [ ] `grep -rn 'ReviewEscalation\|EscalationPhase\|ReviewConcernStreak\|ReviewCycleSummary' apps/ libs/`
- [ ] `grep -rn 'ReviewJsonReader\|ReviewJsonWriter\|FsReviewJsonStore' apps/ libs/`
- [ ] `grep -rn 'domain::review::extract_verdict_json_candidates' apps/ libs/` — V1 path 参照が全部 V2 に置き換わっている
- [ ] `grep -rn 'track\.review()' apps/ libs/` — `TrackMetadata.review()` アクセサ呼び出しが 0 件

### 既存 30 metadata.json の素通し確認 (D9 passthrough 検証)

- [ ] `cargo make test` が PASS すること。libs/infrastructure/src/track/codec.rs に unknown JSON field (review blob) を TrackDocumentV2 として roundtrip するユニットテストが存在し、T004 完了後もそのテストが維持されていること (D9 passthrough 検証。注: `cargo make test -p infrastructure` は誤り — cargo-make の `-p` フラグはプロファイル指定であり Cargo パッケージ指定ではないため、`cargo make test` で全クレートテストを実行して infrastructure テストが通ることを確認する)
- [ ] `git diff main...HEAD -- track/items/` を実行し、`track/items/reviewstate-v1-removal-2026-04-12/` 以外のパスに変更が存在しないことを確認する (本トラックは D9 に従い既存 metadata.json に一切触れない。三点ドット構文 `main...HEAD` で merge-base からの差分を取ることで、自トラックのファイル以外の変更を検出できる)

### D10 ドキュメント / TODO.md 更新確認

以下は各タスクのコミット完了後に確認する (done/obsolete マークはコードを実際に削除したコミットで行う):

- [ ] T001 完了後: `knowledge/strategy/TODO.md` の RVW-54 エントリから `set-approved-head` / `persist_approved_head` 言及が削除されている
- [ ] T001 完了後: `knowledge/DESIGN.md` に V1 review state (ReviewState, escalation, review.json codec 等) への言及が残っていないこと — `grep -n 'ReviewState\|ReviewEscalation\|RecordRoundProtocol\|review_json\|set-approved-head\|resolve-escalation\|record-round\|persist_approved_head\|index_tree_hash_normalizing\|record_round_with_pending\|check_commit_ready\|ReviewCycleSummary\|ReviewRoundResult\|ReviewStatus\|record_round\b' knowledge/DESIGN.md` で結果 0 件であること (注: `RoundType` は D7 で `review_v2/types.rs` に移設されて存続するため grep 対象外)
- [ ] T001 完了後: `knowledge/conventions/hexagonal-architecture.md` に `RecordRoundProtocol` / `ReviewJsonReader` への参照が残っていないこと — `grep -n 'RecordRoundProtocol\|ReviewJsonReader' knowledge/conventions/hexagonal-architecture.md` で結果 0 件であること
- [ ] T001 完了後: `knowledge/conventions/security.md` に `FsReviewJsonStore` への参照が残っていないこと — `grep -n 'FsReviewJsonStore' knowledge/conventions/security.md` で結果 0 件であること
- [ ] T001 完了後: `knowledge/adr/README.md` に `2026-04-12-1800-reviewstate-v1-decommission.md` が索引追加されている
- [ ] T002 完了後: `knowledge/strategy/TODO.md` の RVW-52 が obsolete マークされている (persist_approved_head のdirty review.json 副作用 — コードを削除した同一コミットで)
- [ ] T002 完了後: `knowledge/strategy/TODO.md` の RVW-53 が obsolete マークされている (set-approved-head CLI と自動リカバリ機構の削除 — コードを削除した同一コミットで)
- [ ] T002 完了後: `knowledge/strategy/TODO.md` の RVW-55 が done マークされている (persist_approved_head 残骸の削除 — コードを削除した同一コミットで)
- [ ] T005 完了後: `knowledge/strategy/TODO.md` の RV2-07 が done マークされている (V1 domain コード全体削除 — コードを削除した同一コミットで)

### ドキュメント整合性確認

- [ ] `grep -rn 'sotp review record-round\|sotp review resolve-escalation\|sotp review set-approved-head' .claude/ knowledge/ track/` が想定範囲内のみ残すこと:
  - 残存 OK: 過去のトラック artifact (`track/items/*/metadata.json`, `plan.md`, `spec.md`, `verification.md`)、過去の archive、過去の `knowledge/strategy/rvw-remediation-plan.md` / `knowledge/strategy/refactoring-plan-2026-03-19.md` 等の履歴ドキュメント、`knowledge/research/` 配下の調査ドキュメント、`knowledge/adr/` 内の過去 ADR (例: `2026-03-24-1200-review-state-trust-model.md`、`2026-03-29-0947-review-json-per-group-review-state.md` 等 — 歴史的コンテキストとして V1 コマンド名を含む)、`knowledge/adr/2026-04-12-1800-reviewstate-v1-decommission.md` (本 ADR 自体はコンテキストとして V1 API 名を含む)
  - 残存 NG: 現行運用ドキュメント (`.claude/rules/*.md` — ただし `10-guardrails.md` の該当セクション削除後)、`.claude/commands/`、`knowledge/WORKFLOW.md`、`track/workflow.md`
- [ ] `knowledge/adr/2026-04-12-1800-reviewstate-v1-decommission.md` の Status が `Accepted` に更新されていること
- [ ] `knowledge/adr/README.md` の索引に本 ADR が追加されていること (ドメインモデル・型設計セクションが自然な配置先。T001 で実施)

### ADR 整合性

- [ ] ADR §D1–§D10 の全 Decision が実装されている (T001–T005 の各タスク境界で確認)
- [ ] ADR §Rejected Alternatives A–E との整合性 (今回の実装が却下された代替案に逆行していない)
- [ ] ADR §Consequences Good 項目のうち少なくとも 3 項目が実測で確認できる (TDDD-01 collision 解消 / dead code 削除 / index_tree_hash_normalizing 消失)
- [ ] ADR §Consequences Bad 項目のうち escalation 喪失・set-approved-head 喪失がユーザー向けメッセージ (コミットログ / PR 本文) で明示されていること

## Result / Open issues

(T005 完了後に記録)

## verified_at

(T005 完了後に記録)
