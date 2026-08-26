<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# Bounded-wait termination assertions for descendant-process tests

## Summary

GO-01 → T001. The settled task verifies eventual descendant termination with a bounded re-observation loop and leaves production cleanup behavior unchanged.
GO-02 and GO-03 → T002. .harness/workflows/track/adr2pr.md, .claude/commands/track/adr2pr.md, and .agents/skills/track-adr2pr/SKILL.md: realign the parent-session refresh / resume wording. Anchor: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4. IN-03; IN-04; IN-05; CN-06; CN-07; AC-06; AC-07.

## Tasks (2/2 resolved)

### S1 — Bounded descendant-termination observation

> Update test_version_probe_terminates_descendant_after_clean_pipe_drain in libs/infrastructure/src/review_v2/review_fix_runner/launch_context.rs to use bounded descendant-termination re-observation. IN-01; IN-02; OUT-01; OUT-02; CN-01; CN-02; CN-05; AC-01; AC-02; AC-03; AC-04; AC-05.

- [x] **T001**: Update test_version_probe_terminates_descendant_after_clean_pipe_drain in libs/infrastructure/src/review_v2/review_fix_runner/launch_context.rs to replace the immediate descendant-state assertion with bounded re-observation. IN-01; IN-02; OUT-01; OUT-02; CN-01; CN-02; CN-05; AC-01; AC-02; AC-03; AC-04; AC-05. (`d22f3467b5c26faf319e80e2711395e27625ece6`)

### S2 — Adr2pr parent-session resume wording

> Update .harness/workflows/track/adr2pr.md, .claude/commands/track/adr2pr.md, and .agents/skills/track-adr2pr/SKILL.md. Anchor: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4. GO-02; GO-03; IN-03; IN-04; IN-05; CN-06; CN-07; AC-06; AC-07.

- [x] **T002**: Update .harness/workflows/track/adr2pr.md, .claude/commands/track/adr2pr.md, and .agents/skills/track-adr2pr/SKILL.md. Anchor: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4. GO-02; GO-03; IN-03; IN-04; IN-05; CN-06; CN-07; AC-06; AC-07.
