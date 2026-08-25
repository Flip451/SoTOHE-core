<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# オーケストレーターの文脈摂取を規律化する

## Summary

GL-01 → T001, T005, T006, T007, T011, T015.
GL-02 → T002, T019, T022, T008, T012.
GL-03 → T003, T020, T023, T009, T013.
AC-01 → T001, T005, T006, T007, T011, T015.
AC-02 → T002, T019, T022, T008, T012.
AC-03 → T003, T020, T023, T009, T013.
AC-04 → T004, T021, T024, T010, T014.
AC-05 → T015; AC-06 → T016; AC-07 → T017; AC-08 → T018.

## Tasks (24/24 resolved)

### S1 — Canonical workflow units

> `.harness/workflows/track/*.md` の対象操作を改訂する。IN-01〜IN-04、AC-01〜AC-04。

- [x] **T001**: `.harness/workflows/track/{plan,spec-design,type-design,impl-plan,implement,review}.md` の context / catalogue intake 操作を改訂する。IN-01、IN-02、OS-01、CN-01、CN-03、AC-01。 (`6ab732ece974e0cd9a244935ba998b6b18c3efdb`)
- [x] **T002**: `.harness/workflows/track/pr-review.md` の review-fix 操作を改訂する。IN-01、IN-03、OS-01、CN-01、CN-03、AC-02。 (`6ab732ece974e0cd9a244935ba998b6b18c3efdb`)
- [x] **T003**: `.harness/workflows/track/{pr-review,commit,merge,dry-check,obligation-fulfillment,full-cycle}.md` の長時間 gate / evaluate 操作を改訂する。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-03。 (`1245c37740c6b54808e92e5ff2932184c5b5fab3`)
- [x] **T004**: `.harness/workflows/track/adr2pr.md` の session-update 操作を改訂する。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-04。 (`6ab732ece974e0cd9a244935ba998b6b18c3efdb`)

### S2 — Policy and capability units

> `.harness/policies/*.md` と `.harness/capabilities/*.md` の対象操作を個別に整合させる。IN-01〜IN-04、OS-01、OS-03、CN-01、CN-03、AC-01〜AC-04。

- [x] **T005**: `.harness/policies/*.md`（`consumer-ownership.md` を除く）の context / catalogue intake 操作を workflow SSoT に整合させる。IN-01、IN-02、OS-01、CN-01、CN-03、AC-01。 (`1245c37740c6b54808e92e5ff2932184c5b5fab3`)
- [x] **T006**: `.harness/capabilities/*.md` の context / catalogue intake 操作を workflow SSoT に整合させる。IN-01、IN-02、OS-01、CN-01、CN-03、AC-01。 (`1245c37740c6b54808e92e5ff2932184c5b5fab3`)
- [x] **T019**: `.harness/policies/*.md`（`consumer-ownership.md` を除く）の review-fix delegation 操作を workflow SSoT に整合させる。IN-01、IN-03、OS-01、CN-01、CN-03、AC-02。 (`214f42cdd78a19bab51fd90f577ad03638ea7e73`)
- [x] **T020**: `.harness/policies/*.md`（`consumer-ownership.md` を除く）の long-gate / evaluate 操作を workflow SSoT に整合させる。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-03。 (`4b9a4cf9262f35000ebadbaf84c24b91a2cdcf18`)
- [x] **T021**: `.harness/policies/*.md`（`consumer-ownership.md` を除く）の session-update 操作を workflow SSoT に整合させる。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-04。
- [x] **T022**: `.harness/capabilities/*.md` の review-fix delegation 操作を workflow SSoT に整合させる。IN-01、IN-03、OS-01、CN-01、CN-03、AC-02。 (`214f42cdd78a19bab51fd90f577ad03638ea7e73`)
- [x] **T023**: `.harness/capabilities/*.md` の long-gate / evaluate 操作を workflow SSoT に整合させる。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-03。 (`4b9a4cf9262f35000ebadbaf84c24b91a2cdcf18`)
- [x] **T024**: `.harness/capabilities/*.md` の session-update 操作を workflow SSoT に整合させる。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-04。

### S3 — Claude adapter units

> `.claude/commands/track/*.md` の対象 adapter 操作を更新する。IN-01〜IN-04、AC-01〜AC-04。

- [x] **T007**: `.claude/commands/track/{plan,spec-design,type-design,impl-plan,implement,review}.md` の context / catalogue adapter 操作を更新する。IN-01、IN-02、OS-01、CN-01、CN-03、AC-01。 (`23fea17dcd37af18f0cddd7cc48bafe10fa17706`)
- [x] **T008**: `.claude/commands/track/pr-review.md` の review-fix adapter 操作を更新する。IN-01、IN-03、OS-01、CN-01、CN-03、AC-02。 (`23fea17dcd37af18f0cddd7cc48bafe10fa17706`)
- [x] **T009**: `.claude/commands/track/{pr-review,commit,merge,dry-check,obligation-fulfillment,full-cycle}.md` の gate / evaluate adapter 操作を更新する。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-03。 (`1245c37740c6b54808e92e5ff2932184c5b5fab3`)
- [x] **T010**: `.claude/commands/track/adr2pr.md` の session-update adapter 操作を更新する。IN-01、IN-04、OS-01、OS-03、CN-01、CN-03、AC-04。 (`23fea17dcd37af18f0cddd7cc48bafe10fa17706`)

### S4 — Codex adapter units

> `.agents/skills/track-*/SKILL.md` の対象 adapter 操作を更新する。IN-01〜IN-04、AC-01〜AC-04。

- [x] **T011**: `.agents/skills/track-{plan,spec-design,type-design,impl-plan,implement,review}/SKILL.md` の context / catalogue adapter 操作を更新する。IN-01、IN-02、OS-01、OS-02、CN-01、CN-03、AC-01。 (`1245c37740c6b54808e92e5ff2932184c5b5fab3`)
- [x] **T012**: `.agents/skills/track-pr-review/SKILL.md` の review-fix adapter 操作を更新する。IN-01、IN-03、OS-01、OS-02、CN-01、CN-03、AC-02。 (`23fea17dcd37af18f0cddd7cc48bafe10fa17706`)
- [x] **T013**: `.agents/skills/track-{pr-review,commit,merge,dry-check,obligation-fulfillment,full-cycle}/SKILL.md` の gate / evaluate adapter 操作を更新する。IN-01、IN-04、OS-01、OS-02、OS-03、CN-01、CN-03、AC-03。 (`1245c37740c6b54808e92e5ff2932184c5b5fab3`)
- [x] **T014**: `.agents/skills/track-adr2pr/SKILL.md` の session-update adapter 操作を更新する。IN-01、IN-04、OS-01、OS-02、OS-03、CN-01、CN-03、AC-04。 (`6ab732ece974e0cd9a244935ba998b6b18c3efdb`)

### S5 — Always-applied rules

> Root / provider rule 面を分離し、pointer を更新する。IN-01、IN-02、IN-05、AC-01、AC-05。

- [x] **T015**: `CLAUDE.md`、`AGENTS.md`、`.claude/rules/`、`.codex/instructions.md`、`.codex/rules/default.rules`、`.codex/agents/orchestrator.toml` の always-applied / provider rule 面を分離し、pointer を更新する。IN-01、IN-02、IN-05、OS-01、CN-01、CN-02、AC-01、AC-05。 (`6ab732ece974e0cd9a244935ba998b6b18c3efdb`)

### S6 — Consumer documentation

> `README.md` と `.harness/policies/consumer-ownership.md` の所有説明を更新する。IN-05、CN-01、CN-02、AC-06。

- [x] **T016**: `README.md` と `.harness/policies/consumer-ownership.md` の provider compatibility ownership、runtime 文書の self-contained 表現、enforcement boundary の記述を更新する。IN-05、OS-01、CN-01、CN-02、AC-06。 (`1245c37740c6b54808e92e5ff2932184c5b5fab3`)

### S7 — Orchestrator profile

> `.harness/config/agent-profiles.json` の対象 profile default を更新する。IN-06、AC-07。

- [x] **T017**: `.harness/config/agent-profiles.json` の orchestrator profile default を更新する。IN-06、OS-01、AC-07。 (`6ab732ece974e0cd9a244935ba998b6b18c3efdb`)

### S8 — Per-surface confirmation

> 全対象面の個別確認を実行する。AC-08。

- [x] **T018**: Workflow SSoT、thin adapters、policy / capability、always-applied rules、consumer docs、agent profile の各対象面を個別確認する。AC-08。
