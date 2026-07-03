<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# レビュー負荷軽減 — findings 全件報告と下流 artifact の再記述禁止

## Tasks (4/4 resolved)

### S1 — D1 — findings 全件報告規律の framework-owned surface への配置

> 全件報告規律を `.harness/workflows/track/review.md` Step 3 の briefing 構成規則に義務化記述として追加し、`.harness/custom/review-prompts/` 11 ファイルに挿入済みの規律文を除去する（利用者所有 severity policy には配置しない）。既存の severity 基準文言は変更しない。

- [x] **T001**: harness-policy — (1) `.harness/workflows/track/review.md` Step 3 (per-scope briefing 構成規則): add a mandate that every per-scope briefing (`tmp/reviewer-runtime/briefing-{scope}.md`) include one sentence requiring findings matching the severity policy to be enumerated in full for the round, with the same sentence stating that the severity constraints remain unchanged. (2) Ensure the paragraph beginning "When multiple findings match this severity policy" is absent below `## What to report` in all 11 `.harness/custom/review-prompts/` files (domain.md, usecase.md, infrastructure.md, cli.md, cli_composition.md, cli_driver.md, adr.md, spec.md, types.md, impl-plan.md, harness-policy.md) — leave files unchanged where the paragraph is already absent. The full-report discipline lives only on the framework-owned surface, not in the consumer-owned severity policies. Do not edit any severity-category bullet text. IN-01/CN-01/AC-01. (`43c5cc91dee11f97e88bf97c6b2244eff0ea5bad`)

### S2 — D2 — 再記述禁止 convention の新設（anchor cite 中心式）

> `knowledge/conventions/no-upstream-restatement.md` を anchor cite 中心の規範として確定し、相対参照（数値状態）ルールを本文から除去する。README Current Files への登録を維持する。

- [x] **T002**: harness-policy — `knowledge/conventions/no-upstream-restatement.md` (scaffolded via `bin/sotp conventions add`, already registered in `knowledge/conventions/README.md` Current Files): finalize as an anchor-cite-centric convention. Keep the `## Scope` entries (applies to `impl-plan.json` task text / `plan.sections[].description` and `<layer>-types.json` `docs` / `intent`; excludes `spec.json` per OS-01, pre-track artifacts per OS-02, workflow docs per OS-03) and the anchor-cite rule (reference behaviour via `AC-NN` / `IN-NN` / `CN-NN` cite instead of restating upstream prose). Remove the relative-reference numeric-state rule from `## Rules`, its literal-`schema_version` Bad example from `## Examples`, and its item from `## Review Checklist`. IN-02/AC-02. (`43c5cc91dee11f97e88bf97c6b2244eff0ea5bad`)

### S3 — D3 — reviewer severity policy の更新

> impl-plan.md の実行可能性基準を anchor cite ベースに書き換え、impl-plan.md / types.md の両方に再記述 finding class を追加する。D2 と同一 track 内で完結させる（CN-02）。

- [x] **T003**: harness-policy — In `.harness/custom/review-prompts/impl-plan.md`'s `## What to report` list: (1) rewrite the `task description non-executable` bullet's criterion so a task description is executable when it names the target file/symbol, the operation, and an anchor cite (`AC-NN` / `IN-NN` / `CN-NN` / spec element id), dropping the former requirement that the description state "what the expected behaviour is" (IN-03/AC-03). (2) Add a new finding-class bullet to the same list: a task or plan section that restates an upstream ADR's or spec.json's design rationale or behaviour contract in prose instead of citing it by anchor (IN-04, impl-plan.md half of AC-04). CN-02/AC-05. (`43c5cc91dee11f97e88bf97c6b2244eff0ea5bad`)
- [x] **T004**: harness-policy — In `.harness/custom/review-prompts/types.md`'s `## What to report` list, add a new finding-class bullet (parallel to the one T003 adds to impl-plan.md): a catalogue entry's `docs` / `intent` field that restates an upstream ADR's or spec.json's design rationale or behaviour contract in prose instead of citing it by anchor. IN-04, types.md half of AC-04. CN-02/AC-05. (`43c5cc91dee11f97e88bf97c6b2244eff0ea5bad`)
