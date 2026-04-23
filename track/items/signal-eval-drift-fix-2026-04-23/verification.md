# Verification — signal-eval-drift-fix-2026-04-23

> **Track**: `signal-eval-drift-fix-2026-04-23`
> **ADR**: `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` (§D0.0 / §D1.4 / §D3.1 / §D3.2 / §D6.2)
> **Scope**: T001 (spec.rs signal logic) + T002 (tddd/signals.rs drift 調査) + T003 (render.rs validate_track_snapshots 責務縮退) + T004 (ADR line 削除) + T005 (spec-designer agent 定義) + T006 (cargo make ci)

## 検証範囲

本 track の acceptance_criteria (AC-01..AC-04) に対応する手動 / 自動検証手順を以下に記録する。各 task (T001..T006) の実装完了時に結果を追記する。

## 手動検証手順

### T001 (spec.rs::evaluate_requirement_signal informal-priority 修正)

1. `libs/domain/src/spec.rs::evaluate_requirement_signal` が新 logic に書き換わっている: informal_grounds 非空 → Yellow を最優先、次に adr_refs 非空 → Blue、両方空 → Red
2. signature (`adr_refs: &[AdrRef], informal_grounds: &[InformalGroundRef]`) は無変更 (OS-01)
3. 既存 test `test_requirement_signal_adr_refs_take_priority_over_informal` が新 logic に合わせて逆転 (adr_refs 非空 + informal_grounds 非空 → Yellow を assert)
4. 新 test: `adr_refs 非空 + informal_grounds 非空 → Yellow` の明示 case 追加 (AC-01)
5. `cargo make test` で既存 test 群が全 pass

### T002 (tddd/signals.rs drift 調査)

1. `libs/domain/src/tddd/signals.rs` および関連 file 内の signal 評価関数を精査
2. spec 要素の `adr_refs` + `informal_grounds` と同型の drift があるかを確認
3. drift が存在する場合: 同じ informal-priority logic で修正
4. drift が存在しない場合: 調査結果を commit の note / body に記述して close (no-op でも工数 trace として記録)

### T003 (render.rs::validate_track_snapshots Phase 責務縮退)

1. `libs/infrastructure/src/track/render.rs::validate_track_snapshots` で plan.md missing の snapshot は content check を skip (既存 registry.md `if registry_path.is_file()` と同 pattern) または identity-only と view-freshness の 2 関数に分離
2. Phase 0 直後 (track directory + metadata.json のみ、plan.md 未 render) の状態で `cargo make verify-track-metadata` が pass する (AC-03)
3. 新 test: snapshot directory が metadata.json のみの場合に validate_track_snapshots が pass する unit test
4. 既存 test (`validate_track_snapshots_rejects_*` 系) が引き続き pass (plan.md が存在して out-of-sync な場合は error)

### T004 (ADR §D3.1 line 495 / §D3.2 line 515 削除)

1. `knowledge/adr/2026-04-19-1242-plan-artifact-workflow-restructure.md` §D3.1 から `総合 signal = max(adr_refs, informal_grounds): ...` 行が削除されている (AC-02)
2. §D3.2 から `総合 signal = max(spec_refs, informal_grounds): ...` 行が削除されている (AC-02)
3. 他 line (D3.1 line 490/492/493/494、D3.2 line 511-514) は変更なし
4. section anchor (`D3.1` / `D3.2`) は残り、spec.json の adr_refs が引き続き anchor 解決できる

### T005 (spec-designer agent 定義に判断基準追加)

1. `.claude/agents/spec-designer.md` に以下 3 点が追記されている:
   - universal coding principle (no-panics 等) は個別 element の `constraints[]` 等ではなく track-top の `related_conventions[]` で cite する
   - `convention_refs[]` は signal 評価対象外で Blue に貢献しない。convention のみの要素は Red になる
   - 🟡 Yellow の解消方法 3 選択肢 (adr_refs 昇格 / related_conventions 移動 / 要素削除)
2. 追加された記述が `Scope Ownership` / `Rules` / `Decision Criteria` など既存のセクション構造と整合

### T006 (cargo make ci 回帰ゲート)

1. `cargo make ci` (fmt-check + clippy + nextest + test-doc + deny + check-layers + verify-track-metadata + verify-plan-artifact-refs + 他 verify-* 一式) が全 pass (AC-04)
2. T001-T005 の実装による regression がないこと

## 共通検証

1. Phase 1-3 の gate 評価が通過:
   - spec-signals: blue=21 / yellow=0 / red=0
   - type-signals: 全 3 layer 0 entries / 0 findings (空 catalogue)
   - task-coverage: `bin/sotp verify plan-artifact-refs` PASSED

## 結果 / 未解決事項

(実装完了時に記録)

## verified_at

(実装完了時に記録)
