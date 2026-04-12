# Review System V1 完全撤去 — metadata.json review field + V1 review.json codec + escalation + index_tree_hash_normalizing

## Status

Accepted (implemented in track `reviewstate-v1-removal-2026-04-12`, 2026-04-12)

## Context

### V2 移行の完了と V1 残存

ADR `2026-04-04-1456-review-system-v2-redesign.md` (Accepted) は Review System v1 を構造的問題 (frozen scope / current partition / check_approved の 3 者スコープ不整合、HashMap 循環、並列レビュー干渉) を理由に廃止することを決定した。同 ADR は廃止対象として以下を列挙している:

- `CycleGroupState` / frozen scope
- `ReviewCycle` (domain 層)
- `has_scope_drift`
- `reclassified_paths_outside_cycle_groups`
- `check_cycle_staleness_any`
- `ReviewPartitionSnapshot`
- `RecordRoundProtocol`
- `effective_diff_base`
- `GroupPartition`
- `DiffScope`

本 ADR の起草時点 (2026-04-12) で `has_scope_drift` / `ReviewPartitionSnapshot` / `effective_diff_base` / `GroupPartition` / `reclassified_paths_outside_cycle_groups` / `check_cycle_staleness_any` は既に削除済み (`grep` で該当なし)。V2 実装 (`SystemReviewHasher`, `ReviewCycle<R,H,D>`, `FsReviewStore`, `ReviewJsonV2`, `FsCommitHashStore`) は `apps/cli/src/commands/review/compose_v2.rs` の composition root で production path に組み込まれ、`codex-local` / `check-approved` / `status` CLI サブコマンドは全て V2 を使用する。

しかし V1 コードは部分的に残存している:

1. **`TrackMetadata.review: Option<ReviewState>`** (`libs/domain/src/track.rs:266`) — V1 `ReviewState` struct (`libs/domain/src/review/state.rs::ReviewState`) を保持する field。metadata.json の `review` セクションに対応
2. **V1 `ReviewState` struct** — `ReviewStatus` / `CodeHash` / `ReviewGroupState` / `ReviewEscalationState` を集約する legacy aggregate
3. **V1 `ReviewJson` / `ReviewCycle` (domain::review::cycle)** + **`review_json_codec.rs`** + **`review_json_store.rs` (`FsReviewJsonStore`)** — V1 schema の review.json 読み書き基盤
4. **`index_tree_hash_normalizing`** (`libs/infrastructure/src/git_cli/mod.rs:95-387` + `private_index.rs:71-...`) — V1 `review.code_hash` を `"PENDING"` に正規化してから `git write-tree` する 10 ステップのハッシュ計算
5. **`RecordRoundProtocol`** + **`record_round` / `record_round_typed`** usecase (`libs/usecase/src/review_workflow/usecases.rs:39-348`) — V1 の 2 相 hash commit プロトコル
6. **`resolve_escalation`** usecase + `ReviewCommand::ResolveEscalation` CLI サブコマンド + `run_check_approved` 内の fail-closed escalation gate — V1 の 3 連続サイクル自動 block 機構
7. **`sotp review set-approved-head`** + **`persist_approved_head`** (`apps/cli/src/commands/make.rs:564-687`) — V1 `review.json.approved_head` を commit 後に書き込む recovery パス

### TDDD-01 ブロッカー

TDDD-01 baseline reverse check (`bin/sotp track domain-type-signals`) が `ReviewState` を `baseline_changed_type` Red として誤検出する。原因は `libs/infrastructure/src/code_profile_builder.rs::build_type_graph` が **short name を key とする HashMap** で型を登録しているため:

- `libs/domain/src/review/state.rs::ReviewState` (V1 struct)
- `libs/domain/src/review_v2/types.rs::ReviewState` (V2 enum)

の 2 つが同じ short name `ReviewState` で衝突し、非決定的な iteration order で baseline-capture と signals-evaluate が **異なる** `ReviewState` instance を採取してしまう。結果として構造差分が存在しないにも関わらず `baseline_changed_type` Red が出続け、TDDD-01 トラックの baseline reverse check が常時失敗する。

`build_type_graph` を fully-qualified name keying に修正する案もあるが、V1 `ReviewState` が既に構造的に dead code である現状を考えると、同名衝突の根源を削除する方がクリーン。

### V1 各残存要素の dead 状態

各残存要素の使用状況を grep + call site 解析で検証した結果、以下を確認した:

- `TrackMetadata.review` field: production write path なし (V2 は `review.json` と `.commit_hash` を使用)
- `ReviewState` struct: `TrackMetadata` field 経由のみで参照される
- V1 `ReviewJson` / `ReviewCycle`: CLI `set-approved-head` と make.rs `persist_approved_head` のみが使用
- `index_tree_hash_normalizing`: production caller なし (テストのみが呼び出す、それら自体 V1 test 固定)
- `RecordRoundProtocol` / `record_round*`: CLI caller が削除済みで dead (`ReviewCommand` から `record-round` サブコマンドは既に除去されている、`apps/cli/src/commands/review/tests.rs:791` のコメント「uses v2 ReviewWriter instead of v1 RecordRoundProtocol」で明言)
- `resolve_escalation` + escalation gate: **V2 には escalation が実装されていない**ため、V1 除去 = escalation 機能の暫定喪失
- `set-approved-head` CLI: `.commit_hash` ファイルで置換済み (ADR `2026-04-04-1456` §v1→v2 マイグレーション節参照)。`Makefile.toml` / `.claude/commands/` / `knowledge/WORKFLOW.md` / `track/workflow.md` / hooks いずれにも参照なし
- `persist_approved_head` (自動コードパス): `apps/cli/src/commands/make.rs:561-566` のコメントで "Soft failure only — v1 codec cannot read v2 review.json, so this will fail on v2 tracks. Do NOT set post_commit_failed for v1 legacy errors." と明示されており、**V2 トラックでは常に silently fail する**。実質 no-op

`knowledge/strategy/TODO.md` にも **RV2-07** (MEDIUM): "v1 domain コード残存 — `track/codec.rs` が v1 review 型 (`ReviewCycle`, `CycleGroupState`, `ReviewState` v1, escalation) に依存し削除不可。track codec のリファクタリングが必要" として本作業が記録されている。

## Decision

V1 review 系統を 1 トラックで一括撤去する。影響範囲が広いが、削除のみであり新規コードは import 経路切替と型の移設のみ (`RoundType` を `review_v2::types` に移設)。

### D1: V1 `ReviewState` struct + metadata.json `review` field 削除

- `libs/domain/src/review/state.rs` をファイルごと削除
- `libs/domain/src/track.rs` の `TrackMetadata.review` field + `review()` / `review_mut()` / `set_review()` アクセサ + ctor init を削除
- `libs/infrastructure/src/track/codec.rs` の `TrackDocumentV2.review` field + `TrackReviewDocument` / `ReviewGroupDocument` / `ReviewRoundDocument` / `TrackReviewEscalationDocument` / `EscalationPhaseDocument` / `ReviewCycleDocument` / `ConcernStreakDocument` / `ResolutionDocument` DTO 群 + `review_from_document` / `review_to_document` / `escalation_from_document` / `escalation_to_document` / `escalation_phase_from_document` / `resolution_from_document` / `parse_review_status` / `parse_round_type` / `parse_escalation_decision` / `escalation_decision_to_str` ヘルパー + 対応する V1 codec テスト一式を削除

### D2: V1 `ReviewJson` / `ReviewCycle` (domain) + V1 review.json codec 削除

- `libs/domain/src/review/cycle/` ディレクトリをまるごと削除 (`ReviewJson`, `ReviewCycle`, `CycleGroupState`, `ReviewStalenessReason`, `CycleError`, `GroupRound`, `GroupRoundOutcome`, `GroupRoundVerdict`, `NonEmptyFindings` (v1), `StoredFinding`)
- `libs/infrastructure/src/review_json_codec.rs` ファイル削除
- `libs/infrastructure/src/review_json_store.rs` ファイル削除
- `libs/infrastructure/src/lib.rs` の `pub mod review_json_codec` / `pub mod review_json_store` 除去
- `libs/domain/src/repository.rs` の `ReviewJsonReader` / `ReviewJsonWriter` port trait 削除
- `libs/domain/src/review/mod.rs` / `libs/domain/src/lib.rs` の V1 cycle 関連 re-export (`ApprovedHead`, `CycleError`, `CycleGroupState`, `GroupRound`, `GroupRoundOutcome`, `GroupRoundVerdict`, `NonEmptyFindings`, `ReviewCycle`, `ReviewJson`, `ReviewStalenessReason`, `StoredFinding`) 除去

### D3: V1 escalation 一式削除

- `libs/domain/src/review/escalation.rs` をファイルごと削除 (`EscalationPhase`, `ReviewEscalationBlock`, `ReviewEscalationDecision`, `ReviewEscalationResolution`, `ReviewEscalationState`)
- `libs/domain/src/review/concern.rs` の `ReviewConcernStreak` / `ReviewCycleSummary` を削除。`ReviewConcern` / `file_path_to_concern` は V1 `GroupRound` に紐付いていたため同時に削除
- `libs/usecase/src/review_workflow/usecases.rs` の `resolve_escalation` 関数 + `ResolveEscalationInput` 削除
- `apps/cli/src/commands/review/mod.rs` の `ReviewCommand::ResolveEscalation` variant + `ResolveEscalationArgs` + `execute_resolve_escalation` + `run_resolve_escalation` 削除
- `apps/cli/src/commands/review/mod.rs` の `run_check_approved` 内 fail-closed escalation gate (`if let Some(review_state) = track.review() { ... EscalationPhase::Blocked ... }`) 削除

### D4: V1 `RecordRoundProtocol` + `record_round*` 削除

- `libs/usecase/src/review_workflow/usecases.rs` をファイルごと削除 (`RecordRoundProtocol` trait, `RecordRoundProtocolError`, `record_round`, `record_round_typed`, `RecordRoundInput`, `RecordRoundError`, `validate_round_verdict_inputs`, `validate_stored_findings`, `validate_findings_concern_coverage`, `stored_finding_concern`, 関連テスト)
- `libs/usecase/src/review_workflow/scope.rs` をファイルごと削除 (`DiffScope`, `DiffScopeProvider`, `DiffScopeProviderError`, `FindingScopeClass`, `RepoRelativePath`, `ScopeFilterResult`, `ScopeFilteredPayload`, `apply_scope_filter`, `classify_finding_scope`, `partition_findings_by_scope` — すべて V1 record_round パスの補助関数で、grep 上 module 外参照ゼロ)
- `libs/usecase/src/review_workflow/mod.rs` を prune (`usecases` / `scope` モジュール宣言と re-export を除去、`verdict` モジュールと再エクスポートのみ残す)

### D5: V1 `index_tree_hash_normalizing` 基盤削除

- `libs/infrastructure/src/git_cli/mod.rs` の `GitRepository` trait から `index_tree_hash_normalizing` メソッドを削除 + default impl 削除 + `SystemGitRepo` の 10 ステップ実装 (`Step 1: git show :path` から `Step 10: write-tree` まで) 削除
- `libs/infrastructure/src/git_cli/private_index.rs` の `normalized_tree_hash` メソッド削除。その結果 `PrivateIndex` 本体が未使用になるなら file ごと削除を検討 (実装時に最終確認)
- `libs/infrastructure/src/git_cli/mod.rs` の `index_tree_hash_normalizing_returns_deterministic_hash_for_same_content` / `index_tree_hash_normalizing_ignores_volatile_fields` / `index_tree_hash_normalizing_normalizes_missing_review_section` ユニットテスト削除
- 同 file の `integration_full_protocol_record_set_check` / `integration_source_code_change_fails_check_approved` / `integration_review_status_tamper_fails_check_approved` / `integration_first_round_no_prior_code_hash_succeeds` / `integration_pre_update_freshness_check_detects_code_change_between_rounds` / `integration_updated_at_variation_does_not_affect_hash` / `integration_multi_group_btreemap_produces_stable_hash` 統合テスト削除 (いずれも V1 `ReviewState::record_round_with_pending` / `set_code_hash` / `check_commit_ready` の振る舞い検証)
- `rrz` test helper (V1 `ReviewRoundResult` ファクトリ) 削除

### D6: V1 `set-approved-head` CLI + `persist_approved_head` 削除

- `apps/cli/src/commands/review/mod.rs` の `ReviewCommand::SetApprovedHead` variant + `SetApprovedHeadArgs` + `execute_set_approved_head` + `run_set_approved_head` 削除
- `apps/cli/src/commands/make.rs` の `persist_approved_head` 関数削除
- `apps/cli/src/commands/make.rs::dispatch_commit_from_file` の `persist_approved_head(track_id)` 呼び出し (行 564-566) 削除

V2 代替は既に同じ file の `persist_commit_hash_v2` + `track-set-commit-hash` wrapper として完備されているため、ユーザー向けのリカバリ手段は失われない。

### D7: `RoundType` を `review_v2::types` に移設

`domain::RoundType` (`libs/domain/src/review/types.rs::RoundType`) は `apps/cli/src/commands/review/codex_local.rs:49,62` および `apps/cli/src/commands/review/mod.rs:106,128,129` で V2 パスから使用されている。V1 `review/types.rs` を削除すると `RoundType` も失われるため、V2 `review_v2/types.rs` に移設する:

```rust
// libs/domain/src/review_v2/types.rs
#[derive(Debug, Clone, Copy, PartialEq, Eq, strum::Display, strum::EnumString)]
#[strum(serialize_all = "snake_case")]
pub enum RoundType {
    Fast,
    Final,
}
```

`libs/domain/src/review_v2/mod.rs` で re-export し、`libs/domain/src/lib.rs` は `pub use review_v2::RoundType;` で公開する (または `review_v2::` 経由のみで公開し、`codex_local.rs` / `review/mod.rs` の import を `use domain::review_v2::RoundType` に切り替える)。選択は実装時に決める。

### D8: V1 `Verdict` / `CodeHash` / `ReviewStatus` / `ReviewRoundResult` / `ReviewGroupState` / `ApprovedHead` / `extract_verdict_json_candidates_*` 削除

- `libs/domain/src/review/types.rs` をファイルごと削除 (V1 の `Verdict`, `CodeHash`, `ApprovedHead`, `ReviewStatus`, `RoundType`, `ReviewRoundResult`, `ReviewGroupState`, `extract_verdict_json_candidates_compact`, `extract_verdict_json_candidates_multiline`)。`RoundType` は D7 で移設
- `libs/domain/src/review/error.rs` 削除 (`ReviewError` は V1 パース/検証でのみ使用されており、V2 `review_v2::error::VerdictError` とは別系統)
- V2 には既に `libs/domain/src/review_v2/types.rs` に `Verdict` / `FastVerdict` / `NonEmptyFindings` / `Finding` / `extract_verdict_json_candidates_*` が存在するため、V1 削除後は V2 side のみが残る
- `libs/usecase/src/review_workflow/verdict.rs:185,193` の `domain::review::extract_verdict_json_candidates_*` 呼び出しを `domain::review_v2::extract_verdict_json_candidates_*` に切り替える

### D9: 既存 30 `track/items/*/metadata.json` は migrate しない

- 30 ファイル (全 grep 確認済み) に残存する `review` JSON section は **touch しない**
- codec 側は `#[serde(flatten)] pub extra: serde_json::Map` が unknown field を自動キャプチャするため、V1 `review` キーは decode 時に `extra` に吸収され、encode 時に opaque JSON blob としてそのまま emit される
- プロジェクトポリシーとして後方互換は追跡しない (`DEVELOPER_AI_WORKFLOW.md` + 関連 ADR で明文化)
- 既存 track の `review` data は実質的に dead (V1 `ReviewState::record_round_with_pending` 等の production write path が既に無い) なので、読み書きされない限り塩漬け状態で問題ない
- 将来的に dead data の一括クリーンアップが必要になった場合は別トラックで扱う

### D10: docs 更新

- `.claude/rules/10-guardrails.md` の "Review Escalation Threshold (Enforced by `sotp review record-round`)" セクション (行 95-163) 削除。`sotp review record-round` / `sotp review resolve-escalation` への言及を全て削除
- `knowledge/strategy/TODO.md` の **RV2-07** を done マーク (DONE タグ + 本 ADR / track への参照追加)
- `knowledge/strategy/TODO.md` の **RVW-52** (dirty `review.json` side effect from `persist_approved_head`) を obsolete マーク。D6 で `persist_approved_head` 自体を削除するため、RVW-52 の problem statement が消滅する
- `knowledge/strategy/TODO.md` の **RVW-53** (`/track:commit` に APPROVED_HEAD_FAILED 自動リカバリを追加、つまり set-approved-head 自動実行の追加) を obsolete マーク (set-approved-head CLI 自体が D6 で削除されるため)
- `knowledge/strategy/TODO.md` の **RVW-54** (CLI 統合テストハーネス) は `persist_approved_head` / `set-approved-head` への言及部分のみ削除 (残りの CLI test harness ニーズは継続)
- `knowledge/strategy/TODO.md` の **RVW-55** (v1 `persist_approved_head` 残骸の削除) を done マーク。D6 が `persist_approved_head` 関数本体と `dispatch_commit_from_file` 内の呼び出しを削除するため、RVW-55 の削除目的が本トラックで達成される
- `knowledge/DESIGN.md` に V1 review state 残存の言及があれば削除 (実装時に grep 確認)
- `knowledge/conventions/hexagonal-architecture.md` の `RecordRoundProtocol` / `ReviewJsonReader` への参照を削除する (V1 API が消えることで dead reference になるため)
- `knowledge/conventions/security.md` の `FsReviewJsonStore` への参照を削除する (V1 API が消えることで dead reference になるため)

## Rejected Alternatives

### A: V1 `ReviewState` をリネームするのみで済ませる

**却下理由**: TDDD-01 の同名衝突は解消するが、~7000 行の dead code (`index_tree_hash_normalizing`, `RecordRoundProtocol`, `review_json_codec.rs`, `review_json_store.rs`, `set-approved-head`, V1 cycle types, escalation types) が残る。RV2-07 が未解消のまま残り、将来同様の collision や混乱を招く可能性がある。

### B: escalation を V2 に先行実装してから V1 を削除する

**却下理由**: V2 escalation の再設計 (RV2-06) は独立した設計作業で、本トラック (TDDD-01 のブロッカー解消) の範囲外。escalation redesign を先行させると TDDD-01 が数週間ブロックされる。escalation 機能は CI 自動 block ではなく人間の注意深さで代替可能な advisory 機構であり、暫定喪失の影響は限定的。

### C: 既存 30 `track/items/*/metadata.json` を一括 migrate する

**却下理由**: プロジェクトポリシーが後方互換を追跡しない方針で一貫しており、dead data の migration は価値が低い。既存ファイルは `extra` flatten でデコード可能であり、実害は生じない。マイグレーションスクリプトの追加 (disposable) + 30 ファイル editing の diff churn はレビュー負荷を増やすだけで運用価値がない。

### D: CLI `set-approved-head` を残して V1 review.json codec のみ保持

**却下理由**: `set-approved-head` は `.commit_hash` で完全に代替されており (ADR 2026-04-04-1456 §v1→v2 マイグレーション節の明文)、`Makefile.toml` / skill / workflow ドキュメントいずれからも参照されていない。`persist_approved_head` 自動コードパスも「v1 codec cannot read v2 review.json, so this will fail on v2 tracks」とコメントで明示されている dead code。これを残す唯一の理由は V1 review.json 基盤 (`review_json_codec.rs`, `FsReviewJsonStore`, V1 `ReviewJson`/`ReviewCycle`) を残すためだが、それ自体が削除対象。

### E: V1 削除を段階的に複数トラックに分割する

**却下理由**: V1 review 系統は層間で密結合しており、部分削除は compile 不可状態を生む (`TrackMetadata.review` を削ると codec が通らない、codec を削ると CLI が通らない等)。dead code 削除のみで、レビュー負荷は line count に比例しないため、1 トラックで CLI → usecase → infra → domain の順にタスク分割するのが妥当。

## Consequences

### Good

- `build_type_graph` の `ReviewState` 同名衝突が構造的に解消し、TDDD-01 baseline reverse check の spurious Red が消える
- 副次的に `NonEmptyFindings` / `Finding` / `Verdict` の同名衝突も解消される (v1 side の型が消えるため)
- ~7000 行の dead code が消え、コードベースの認知負荷が下がる
- `index_tree_hash_normalizing` の 10 ステップ複雑な git tree hash pipeline が消え、`SystemReviewHasher` (scope-based manifest hash) のシンプルな設計に一本化される
- V1 `FsReviewJsonStore` と V2 `FsReviewStore` の同時存在による co-ordination hazard (同じ `review.json` パスに異なる schema で書く可能性) が解消する
- RV2-07 (TODO.md) が完全に消化される

### Bad

- `sotp review resolve-escalation` CLI サブコマンドが消失 → 3 連続 closed cycle 自動 block 機能が暫定的に失効 (RV2-06 による V2 escalation 再実装までの間)
- `sotp review set-approved-head` CLI サブコマンドが消失 → ただし `.commit_hash` + `cargo make track-set-commit-hash` で完全代替済みで実質影響なし
- 既存 30 `track/items/*/metadata.json` に dead `review` JSON section が残存し、`extra` flatten で塩漬けされる (読み書きされないため害はないが、視覚的ノイズ)
- `.claude/rules/10-guardrails.md` の "Review Escalation Threshold" セクションが消え、concern slug + workspace search + reinvention check を強制する自動メカニズムが失われる (人手運用に戻る)

## Reassess When

- **RV2-06** (v2 escalation redesign) を再開する時 → 別 ADR で V2 escalation の設計を定義。domain 型の配置、CLI 表現、自動 block の発火条件などを新規に決める
- 既存 30 `track/items/*/metadata.json` の dead `review` section が CI / 検証パイプラインで問題を起こす場合 → 別トラックで一括 cleanup (python one-shot or codec migration)
- `build_type_graph` の HashMap non-determinism が **他の** 同名型で再発する場合 → `build_type_graph` を fully-qualified name keying に修正する別トラックを立てる (本 ADR の対象外)
- `track/workflow.md` / skill / hook のいずれかで `sotp review resolve-escalation` / `sotp review set-approved-head` を再度参照する必要が出た場合 → まず本 ADR を read し、再実装が spec v2 に整合するか評価する
