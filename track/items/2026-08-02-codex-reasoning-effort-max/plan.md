<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# reasoning effort に max 段を追加し、限定レーンを Luna Max へ移行する

## Summary

GO-01 → T001.
GO-02 → T003, T002, T004.

## Tasks (0/4 resolved)

### provider-compatibility — Provider compatibility

> Update `profiles.rs::supports_effort` and execution-route tests. [IN-01; IN-02; OUT-01; CN-01; AC-01; AC-02]

- [ ] **T001**: Update `libs/infrastructure/src/agent_profiles/profiles.rs::supports_effort` and capability-execution, agent-profile-resolution, and review-execution route tests. [IN-01; IN-02; OUT-01; CN-01; AC-01; AC-02]

### pre-rollout-recovery — Pre-rollout recovery

> Update rollout workflow SSoTs with recovery verification and the B2-to-B3 stop/reload handoff. [IN-05; OUT-03; CN-03; AC-04]

- [ ] **T003**: Update `.harness/workflows/track/implement.md` and `.harness/workflows/track/full-cycle.md` with the pre-rollout recovery procedure, verification coverage, and the B2-to-B3 stop/reload handoff. [IN-05; OUT-03; CN-03; AC-04]

### limited-profile-rollout — Limited profile rollout

> Update selected agent-profile entries and committed-profile-resolution tests. [IN-03; IN-04; OUT-02; CN-02; AC-03]

- [ ] **T002**: Update selected `.harness/config/agent-profiles.json` entries and `AgentProfiles::resolve_execution` committed-profile-resolution tests. [IN-03; IN-04; OUT-02; CN-02; AC-03]

### post-rollout-observation — Post-rollout observation

> Update rollout workflow SSoTs with the completion-time observation-recording procedure. [IN-06; OUT-04; AC-05]

- [ ] **T004**: Update `.harness/workflows/track/implement.md` and `.harness/workflows/track/full-cycle.md` with the completion-time observation-recording procedure and verification coverage; the completion step emits this track's `observations.md` after rollout outcomes exist. [IN-06; OUT-04; AC-05]
