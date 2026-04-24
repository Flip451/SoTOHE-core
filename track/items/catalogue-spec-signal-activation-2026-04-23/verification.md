# Verification — catalogue-spec-signal-activation-2026-04-23 (SoT Chain ② の有効化)

> **Track**: `catalogue-spec-signal-activation-2026-04-23`
> **ADR**: `knowledge/adr/2026-04-23-0344-catalogue-spec-signal-activation.md` (D1-D6)
> **Parent ADR amendments**: `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` (§Follow-up 末尾 2026-04-23 Correction: `check_impl_plan_presence` invariant revert)、`knowledge/adr/2026-04-11-0003-type-action-declarations.md` (WIP-Yellow rule 実装 + Status → Accepted; spec.json/task-coverage.json 外の追加対応、下記 Notes に検証記録)
> **Scope**: AC-01..AC-10 (spec.json acceptance_criteria) + IN-21 (check_impl_plan_presence 削除、spec-designer back-and-forth で追加)

## 検証範囲

本 track の acceptance_criteria (AC-01..AC-10) および追加 in_scope (IN-21) に対応する手動 / 自動検証手順を以下に記録する。各 task (T001..T024 + 後続 task) の実装完了時に結果を追記する。

本 track は **planning 段階完了直後** (Phase 3 完了時点) の scaffold 状態で commit される。具体的な検証結果は実装 phase (`/track:implement`) 各 task 完了後に追記される。

## Scope Verified (pending implementation)

### Acceptance Criteria

- [ ] AC-01: `sotp track catalogue-spec-signals` が所定の input (catalogue) から deterministic な output (signals file) を生成する
- [ ] AC-02: `sotp verify catalogue-spec-refs` が dangling / hash drift / stale を検出し、適切な exit code を返す
- [ ] AC-03: pre-commit で binary gate ERROR が commit をブロック、signal 再計算が commit finalize 前に signals file を更新する
- [ ] AC-04: `cargo make ci` に新 2 task (`verify-catalogue-spec-refs` + `check-catalogue-spec-signals`) が含まれ、全て pass する
- [ ] AC-05: merge gate が SoT Chain bottom-up 順序で実行され、strict=true で Yellow をブロックする
- [ ] AC-06: `architecture-rules.json` の `catalogue_spec_signal.enabled` flag が layer 単位の gate 発火を制御し、SoTOHE-core 自身の dogfood が pass する
- [ ] AC-07: `review_operational` に `*-catalogue-spec-signals.json` glob が追加される
- [x] AC-08: 親 ADR `2026-04-19-1242` §D3.2 の amendment (hash binary gate 分離) 完了 ※T021 plan-artifacts review cycle 内で実施済み
- [ ] AC-09: 親 ADR `2026-04-12-1200` §D5 の amendment (merge gate bottom-up reorder) 完了
- [ ] AC-10: `cargo make ci` が新 unit / integration test を含めて全項目 pass する

### In-Scope (additional)

- [ ] IN-21: `libs/domain/src/impl_plan.rs::check_impl_plan_presence` + `ImplPlanPresenceError` の削除、および `latest_track.rs` / `make.rs` の call site 除去

### Additional ADR Amendments (not in spec.json/task-coverage.json scope, but performed within this track)

- [x] 親 ADR `2026-04-11-0003-type-action-declarations.md`: WIP-Yellow rule を evaluator code (libs/domain/src/tddd/signals.rs) に実装済み、Status を Proposed → Accepted に更新済み。plan-artifacts review cycle 内 (review-fix-lead) で実施完了。spec.json / task-coverage.json には IN として記録されていない (planning stage 後発の追加対応; 正式な IN 番号なし)。検証方法: `bin/sotp track type-signals` で TrackBlobReader / GitShowTrackBlobReader が Red → Yellow に遷移 (pre-implementation 段階での commit ブロック解消) したことを確認済み

## Manual Verification Steps (to be executed at each task completion)

1. 各 task 完了時に関連 acceptance_criteria / in_scope id の checkbox を更新する
2. 各 commit 境界で `cargo make ci` 全通過を確認する。compile-coupled task (例: T006 port 追加 + T011/T012 adapter 追加) は同一 commit にまとめることで CI グリーン状態を維持する。task 単位での CI 通過が技術的に不可能な場合 (trait 定義と実装が別 task に分割されている場合) は、実装セットが揃う commit 境界で確認する
3. Yellow 状態の catalogue entry (全 12 entry、Phase 2 出力) が実装完了で順次 Blue 化していくことを `bin/sotp track catalogue-spec-signals` で追跡する (active track に自動適用、`--layer <layer_id>` 省略で全対象層)。`bin/sotp track type-signals` は SoT Chain ③ (type→implementation signal) 用であり、本 track が導入する SoT Chain ② (catalogue→spec signal) の追跡には `catalogue-spec-signals` サブコマンドを使用する
4. 実装 phase 終盤で T023 により catalogue-spec signal が Blue 化する。Blue 化の条件は ADR D1.1 informal-priority rule に従い「informal_grounds[] 空 + spec_refs[] 非空」であるため、T023 では informal_grounds[] の清書と spec_refs[] の形式化 (anchor + SHA-256 hash 記入) を合わせて実施する
5. 最終 commit 前に各経路の動作を確認する: `bin/sotp verify catalogue-spec-refs` は pre-commit / CI / merge gate の全 3 経路で動作すること; `bin/sotp track catalogue-spec-signals` は pre-commit 経路でのみ動作し (CI / merge gate では再計算しない、D3.5 の設計)、CI と merge gate は pre-commit が persist した signals file を消費する

## Result

- pending (implementation phase 未着手)

## Open Issues

### 記録: Phase 2 の Red 解消経緯 (2026-04-23)

本 track の planning phase 完了後、`bin/sotp track type-signals` 実行で `TrackBlobReader` (usecase) および `GitShowTrackBlobReader` (infrastructure) が Red を emit した。diagnosis の結果、原因は amended ADR `2026-04-11-0003` の WIP-Yellow rule (forward check missing → Yellow) が評価器 code (`libs/domain/src/tddd/signals.rs`) に未実装であったこと。

対応として本 track 内で以下を先行実施:

- `evaluate_trait_methods` および `evaluate_secondary_adapter` を ADR 準拠に修正 (forward check missing → Yellow、reverse check extra → Red の分離)
- 関連 unit test 4 件を Yellow 期待値に更新 (`test_evaluate_secondary_port_yellow_when_returns_mismatch` / `test_evaluate_secondary_adapter_yellow_one_impl_missing` / `test_evaluate_secondary_adapter_yellow_method_signature_mismatch` / `test_evaluate_secondary_adapter_with_two_traits_one_missing_is_yellow`)
- ADR `2026-04-11-0003` Status を `Proposed` から `Accepted` に promote

この対応により `TrackBlobReader` / `GitShowTrackBlobReader` は Red → Yellow に遷移、pre-implementation 段階での commit ブロックが解消された。本修正は当初 impl-plan にない追加対応であり、Option A scope 拡大判断 (domain scope に帰属) として本 track 内に commit 済み。この evaluator fix は T025 (check_impl_plan_presence / ImplPlanPresenceError 削除、まだ未着手の todo task) とは独立した別作業として実施済み。evaluator fix 自体は domain scope reviewer が review・承認済みのため、impl-plan.json に独立 task として追記しない。T025 は impl-plan.json に todo として引き続き残る。

## verified_at

- pending
