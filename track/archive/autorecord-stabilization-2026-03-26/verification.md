# Verification: autorecord-stabilization-2026-03-26

## Freeze Notice

- 2026-03-29 時点でこのトラックは凍結する。
- 理由: current spec (`WF-59`) の範囲を超えて、review state 分離 / per-group hash / group 独立進行 / cycle freeze / `check-approved` 再設計が必要だと判明したため。
- このトラックは `superseded by new review-state architecture` として扱い、review cycle の収束はここでは追わない。
- 後継設計の叩き台は [tmp/設計方針-2026-03-29-0947.md](/home/flip451/individual/t-rust/templates/SoTOHE-core/tmp/設計方針-2026-03-29-0947.md) を参照。

## Scope Verified

- [x] track/review-scope.json が存在し、スキーマが妥当
- [x] ReviewScopePolicy がパスを正しく分類する (test_classify_* 26 tests)
- [x] review_hash が worktree から直接読み、未ステージでも成功する (collect_review_scope reads worktree)
- [x] review.json の変更で hash が変わらない (ReviewOperational classification excludes it)
- [x] 他トラック/planning-only ファイルの変更で hash が変わらない (OtherTrack/PlanningOnly classification)
- [x] 実装ファイルの変更で hash が変わる (test_hash_changes_when_content_changes)
- [x] review-scope.json が存在しない場合、review_hash が bootstrap policy にフォールバックする（worktree/base_ref 両方で不在時）。低レベルローダーは fail-closed (test_load_config_missing_file_returns_missing_error)、SystemGitHasher は bootstrap fallback を適用
- [x] review-scope.json のパターン変更でスコープ分類が切り替わる (policy is config-driven via globset)
- [x] review-scope.json 自体が hash scope に含まれ、ポリシー変更が旧承認を無効化する (test_classify_review_scope_json_is_implementation)
- [x] RecordRoundProtocolImpl が single-phase で動作する (uses record_round, not record_round_with_pending)
- [x] legacy hash が check_commit_ready で StaleCodeHash を返す (test_check_commit_ready_legacy_hash_is_stale)
- [x] index_tree_hash_normalizing が production review path から除去済み (SystemGitHasher uses review_scope, RecordRoundProtocolImpl uses single-phase)
- [x] StoredReviewHash / LegacyHash 型は完全削除済み (CodeHash::computed の rvw1 validation に集約)
- [x] record_round_with_pending は production review path から除去済み (single-phase protocol)

## Manual Verification Steps

1. [x] `cargo make test` — pass
2. [x] `cargo make ci` — pass
3. [ ] 新規 track（未コミット）で `sotp review codex-local --auto-record` が成功することを確認 — レビューサイクルで検証中
4. [x] 並列 review group の record-round が互いの hash を invalidate しないことを確認 — `review-scope.json` で `remove_fields: ["review"]` + `fixed_fields: { updated_at }` により `metadata.json.review` サブツリー全体を hash scope から除外。`normalize_track_file_for_hash` のユニットテストで検証済み。残存する JSON key-order 起因の hash 変動は RVW-31 (review.json 完全分離) で対処予定
5. [x] `.claude/docs/DESIGN.md` の変更が review hash を変化させないことを確認 — PlanningOnly 分類で除外
6. [x] `libs/domain/src/*.rs` の変更が review hash を変化させることを確認 — Implementation 分類で包含
7. [x] `track/review-scope.json` 自体が hash scope に含まれる — bootstrap rule で Implementation

## Result / Open Issues

- 結果: 実装本体は完了し、`cargo test --workspace --quiet` / `cargo make ci` は通過。ただし review-state architecture の再設計が必要になったため、このトラックは 2026-03-29 に凍結した。
- live review status は固定値をこの文書に複写せず、都度 `bin/sotp review status --track-id autorecord-stabilization-2026-03-26` を正とする。
- 当初の残作業だった T019: review cycle の収束と track artifact 同期は、このトラックでは完了させず後継トラックへ移管する。
- live 状態は rerun の進行で変動するが、このトラックでは以後それを完了条件として追わない。
- 継続 open issue:
  - RVW-20: ACCEPTED finding の仕組み化 + dispute adjudication
  - RVW-21: per-group 独立レビュー進行
  - ~~RVW-23: is_planning_only_path と review-scope.json の SSoT 統合~~ ✅ 修正済み
  - RVW-24: 降格ロジック見直し
  - RVW-25: domain 値オブジェクト徹底
  - RVW-30: track-commit-message の add-all 自動実行
  - RVW-31: review state を review.json に分離 + 内部 checksum
  - RVW-32: same_round_and_zero_findings 制約の扱い再設計（新 review-state architecture 側で再検討）
  - WF-60: 設計⇆実装の自動遷移
- 2026-03-29 仕様更新: parallel auto-record を優先するため `metadata.json.review` を review hash から除外。review verdict provenance / artifact attestation は `tamper-proof-review-2026-03-26` で扱う
- 後継トラックへ移す中核論点:
  - review state の `metadata.json` からの分離
  - group 独立進行
  - per-group hash
  - cycle freeze / partition freeze
  - `check-approved` の latest-success 判定規則

## Verified At

- 2026-03-28 (implementation + CI)
- 2026-03-28 (`is_planning_only_path` を `track/review-scope.json` 駆動へ統合)
- 2026-03-28 (shared env/cwd test lock 導入で `cargo test --workspace` 並列安定化)
- 2026-03-28 (`resolve_items_dir_for_runtime` と review hash path handling の追加修正、review fast round 再開)
- 2026-03-29 (parallel auto-record 優先の仕様変更、rendered view 再同期、`cargo make ci` 再通過)
- レビュー: 進行中
