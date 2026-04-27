<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 20, yellow: 0, red: 0 }
---

# verify チェーンを file 存在ベースの phase 責務分離に揃える

## Goal

- [GO-01] `cargo make ci` 内の 3 つの verify (`verify-latest-track-local` / `verify-view-freshness-local` / `verify-catalogue-spec-refs-local`) を「file 存在 = phase 状態」原則に従って改修し、Phase 0 only commit が `cargo make ci` を通過できるようにする。これにより `workflow-ceremony-minimization` convention の原則が verify チェーン全体に一貫適用される [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D1: verify チェーンを「file 存在 = phase 状態」原則に揃える]

## Scope

### In Scope
- [IN-01] `libs/infrastructure/src/verify/latest_track.rs` の `verify` 関数を改修し、`impl-plan.json` が存在しない (Phase 0 / Phase 1 / Phase 2) 場合は `spec.md` / `spec.json` / `plan.md` の存在チェックを SKIP する。`impl-plan.json` の存在を検出した時点で従来どおり全 artifact の存在を要求する [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.1: `verify-latest-track-local`] [tasks: T001]
- [IN-02] `libs/infrastructure/src/verify/view_freshness.rs` の `plan.md` absent FAIL を silent SKIP (`continue`) に変更し、`libs/infrastructure/src/track/render.rs:621-624` の `validate_track_snapshots` と同じ挙動に揃える [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.2: `verify-view-freshness-local`] [tasks: T002]
- [IN-03] `apps/cli/src/commands/verify_catalogue_spec_refs.rs` の `read_spec_element_hashes` の挙動を改め、catalogue file の存在を先に検出する分岐に変更する: catalogue absent → silent PASS / catalogue present + `spec.json` absent → FAIL (SoT Chain ② 違反) / catalogue present + `spec.json` present → 従来どおり ref integrity 検査 [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.3: `verify-catalogue-spec-refs-local`] [tasks: T003]
- [IN-04] `apps/cli/src/commands/verify_catalogue_spec_refs.rs` の既存テスト `verify_fails_when_spec_missing` を 2 ケースに分割する: `verify_passes_when_catalogue_absent_and_spec_absent` (新規、Phase 0 状態で PASS) と `verify_fails_when_catalogue_present_and_spec_absent` (改名、SoT Chain ② 違反検出を残す) [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D3: 既存テストの分割と Phase 0 PASS 確認テスト追加] [tasks: T003]
- [IN-05] `latest_track.rs` および `view_freshness.rs` に Phase 0 状態 (`impl-plan.json` absent / `spec.json` absent / `plan.md` absent) で PASS することを確認する unit test を新規追加する [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D3: 既存テストの分割と Phase 0 PASS 確認テスト追加] [tasks: T001, T002]

### Out of Scope
- [OS-01] すでに file 存在ベースに揃っている他の verify (`verify-plan-progress` / `verify-track-metadata` / `verify-track-registry` / `verify-plan-artifact-refs` / `verify-spec-states-current` / `check-catalogue-spec-signals` / `check-approved`) は本決定では変更しない [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D4: スコープ境界]
- [OS-02] `/track:done` ゲート移行や PR-merge 時の strict 検査強化は本 track のスコープ外。別 ADR で扱う [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D4: スコープ境界]
- [OS-03] `metadata.json` への `phase_override` / `intentionally_phase_X_only` マーカー追加は ADR で却下されており、本 track では採用しない [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#A. `metadata.json` に `phase_override` / `intentionally_phase_X_only` マーカーを追加]
- [OS-04] `cargo make ci` の分割 (`ci-fast` / `ci-final` の 2 段階化) は ADR で却下されており、本 track では採用しない [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#C. `cargo make ci` を `ci-fast` / `ci-final` の 2 段階に分割]
- [OS-05] 該当 3 verify の warning 格下げ (error → warn) は ADR で却下されており、本 track では採用しない [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D. 該当 3 verify を warning 格下げ (error → warn)]
- [OS-06] 環境変数 / コマンドフラグによる個別 skip (例: `SKIP_LATEST_TRACK_VERIFY=1`) は ADR で却下されており、本 track では採用しない [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#E. 環境変数 / コマンドフラグでの個別 skip (例: `SKIP_LATEST_TRACK_VERIFY=1`)]

## Constraints
- [CN-01] phase の自動判定は artifact 存在チェックに統一する。`metadata.json` 上への追加マーカーや `phase_override` フィールドのような人工状態は導入しない [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D1: verify チェーンを「file 存在 = phase 状態」原則に揃える] [tasks: T001, T002, T003]
- [CN-02] catalogue absent + `spec.json` absent の組み合わせは silent PASS とする。catalogue present + `spec.json` absent は引き続き FAIL とし、SoT Chain ② の上下関係を反映する [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.3: `verify-catalogue-spec-refs-local`] [tasks: T003]
- [CN-03] `verify-latest-track-local` の条件分岐は `impl-plan.json` の存在を基準とする。`impl-plan.json` が存在する場合のみ `spec.md` / `spec.json` / `plan.md` の存在を要求する。これは convention Examples 「`verify-latest-track-local` が `impl-plan.json` の存在を検出したときのみ task 項目をチェックする」と同じ条件分岐を spec/plan チェックに拡張したものである [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.1: `verify-latest-track-local`] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] Phase 0 状態 (`metadata.json` のみ存在し `spec.json` / `<layer>-types.json` / `impl-plan.json` / `plan.md` が不在) のトラックに対して `cargo make ci` が pass する (3 つの対象 verify がいずれも FAIL しない) [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#Positive] [tasks: T001, T002, T003]
- [ ] [AC-02] `verify-latest-track-local` が Phase 0 状態 (`impl-plan.json` absent) で PASS し、Phase 3 状態 (`impl-plan.json` present) では `spec.md` / `spec.json` / `plan.md` の存在を従来どおり要求して FAIL する unit test が pass する [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.1: `verify-latest-track-local`] [tasks: T001]
- [ ] [AC-03] `verify-view-freshness-local` が `plan.md` absent 状態で FAIL せず silent SKIP (PASS) し、`plan.md` present 状態では freshness チェックを従来どおり実行する unit test が pass する [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.2: `verify-view-freshness-local`] [tasks: T002]
- [ ] [AC-04] `verify-catalogue-spec-refs-local` が catalogue absent + `spec.json` absent の状態 (Phase 0) で PASS し、catalogue present + `spec.json` absent の状態で FAIL し、catalogue present + `spec.json` present の状態で ref integrity 検査を従来どおり実行する unit test が pass する (`verify_passes_when_catalogue_absent_and_spec_absent` / `verify_fails_when_catalogue_present_and_spec_absent` のテスト名で確認できる) [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#D2.3: `verify-catalogue-spec-refs-local`] [tasks: T003]
- [ ] [AC-05] `cargo make ci` (fmt-check + clippy + nextest + deny + check-layers + verify-*) が全て pass する。変更された 3 verify の新規 / 修正 unit test を含めて全テストが pass する [adr: knowledge/adr/2026-04-27-0324-phase-aware-verify-gates.md#Positive] [tasks: T001, T002, T003]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 20  🟡 0  🔴 0

