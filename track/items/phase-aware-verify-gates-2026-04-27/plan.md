<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# verify チェーンを file 存在ベースの phase 責務分離に揃える

## Tasks (2/3 resolved)

### S1 — Infrastructure: latest_track.rs phase-aware skip (IN-01 + IN-05 partial)

> `verify-latest-track-local` は現状 `spec.md` / `spec.json` / `plan.md` をすべて必須チェックするため Phase 0 commit をブロックする。
> T001 は `impl-plan.json` の存在を判定キーとして使い、不在時は spec/plan チェックを丸ごと SKIP する。これは convention Examples の「`impl-plan.json` 存在検出時のみ task 項目をチェックする」と同じ条件分岐を spec/plan チェックに拡張したもの (CN-03)。
> Phase 0 PASS の unit test を同タスクで追加する (IN-05 のうち `latest_track.rs` 分担)。

- [x] **T001**: `libs/infrastructure/src/verify/latest_track.rs` の `verify` 関数を改修する。`impl-plan.json` が不在の場合 (Phase 0 / Phase 1 / Phase 2) は `spec.md` / `spec.json` / `plan.md` の存在チェックを SKIP し、`impl-plan.json` が存在する場合のみ全 artifact の存在を要求する。また、Phase 0 状態 (`impl-plan.json` absent / `spec.json` absent / `plan.md` absent) で PASS することを確認する unit test `test_phase0_track_with_no_artifacts_passes` を追加する。 (`797fce5b926a838dd12aa2b19b0a5103ab1738f7`)

### S2 — Infrastructure: view_freshness.rs plan.md absent silent SKIP (IN-02 + IN-05 partial)

> `verify-view-freshness-local` は `plan.md` absent を FAIL するため Phase 0 commit をブロックする。
> T002 は `plan.md` absent の FAIL を `continue` に変更し、`validate_track_snapshots` と同じ挙動に揃える (IN-02)。
> 既存テスト `test_view_freshness_errors_when_plan_md_missing` のアサーション反転と改名、および Phase 0 PASS の unit test 追加を同タスクで行う (IN-05 のうち `view_freshness.rs` 分担)。

- [x] **T002**: `libs/infrastructure/src/verify/view_freshness.rs` の `plan.md` absent FAIL (lines 84-90) を `continue` (silent SKIP) に変更し、`libs/infrastructure/src/track/render.rs:621-624` の `validate_track_snapshots` と同じ挙動に揃える。`plan.md` absent 状態で PASS することを確認する unit test `test_view_freshness_passes_when_plan_md_absent` を追加し、既存テスト `test_view_freshness_errors_when_plan_md_missing` を `test_view_freshness_skips_when_plan_md_absent` に改名してアサーションを PASS に更新する。 (`04f8179e16c0533281fe009073a37f014af3095b`)

### S3 — CLI: verify_catalogue_spec_refs.rs catalogue-first branching + test split (IN-03 + IN-04)

> `verify-catalogue-spec-refs-local` は `spec.json` absent を `CliError` で hard fail するため Phase 0 commit をブロックする。
> T003 は `read_spec_element_hashes` の呼び出しを catalogue 存在検出の後に移動し、catalogue absent → early PASS を実現する (IN-03, CN-02)。
> 既存テスト `verify_fails_when_spec_missing` を 2 ケースに分割することで、Phase 0 PASS と SoT Chain ② 違反検出の両方を独立したテストとして表現する (IN-04)。

- [~] **T003**: `apps/cli/src/commands/verify_catalogue_spec_refs.rs` の `read_spec_element_hashes` を改修する。catalogue file の存在を先に検出する分岐に変更する: catalogue absent → silent PASS (早期リターン)、catalogue present + `spec.json` absent → 引き続き FAIL (SoT Chain ② 違反)、catalogue present + `spec.json` present → 従来どおり ref integrity 検査。既存テスト `verify_fails_when_spec_missing` を 2 ケースに分割する: `verify_passes_when_catalogue_absent_and_spec_absent` (Phase 0 状態で PASS) と `verify_fails_when_catalogue_present_and_spec_absent` (SoT Chain ② 違反検出を残す)。
