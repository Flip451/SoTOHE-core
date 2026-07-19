<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# escalation lane の規約・guardian 挙動を decision-freeze ADR D6/D7 に整合させる

## Summary

GO-01 is implemented by T001, T002, and T003. The enforced spec-element mappings are recorded in task-coverage.json.
All type catalogues are empty for this docs-only track; task-contract.json therefore attributes no catalogue entries.

## Tasks (3/3 resolved)

### S1 — Autonomous escalation convention

> Update `knowledge/conventions/pre-track-adr-authoring.md` Phase 1+ escalation-lane guidance (D6; IN-01, CN-01, AC-01).

- [x] **T001**: Update the Phase 1+ escalation-lane guidance in `knowledge/conventions/pre-track-adr-authoring.md`. Validate the document against D6; IN-01, CN-01, and AC-01.

### S2 — Guardian mismatch and track-born ADR contract

> Update guardian escalation-lane guidance in `.harness/capabilities/adr-diagnoser.md`, `.agents/skills/adr-diagnoser/SKILL.md`, and `.harness/capabilities/adr-editor.md` (D5, D6, D7; IN-02, IN-03, CN-01, CN-02, CN-03, AC-02, AC-03, AC-04).

- [x] **T002**: Update the guardian escalation-lane guidance in `.harness/capabilities/adr-diagnoser.md`, `.agents/skills/adr-diagnoser/SKILL.md`, and `.harness/capabilities/adr-editor.md`. Validate the documents against D5, D6, and D7; IN-02, IN-03, CN-01, CN-02, CN-03, AC-02, AC-03, and AC-04.

### S3 — Workflow escalation-lane alignment

> Update escalation-lane guidance in `.harness/workflows/track/{review,plan,adr2pr}.md` (D5, D6, D7; IN-02, IN-03, CN-01, CN-02, CN-03, AC-01, AC-02, AC-03, AC-04).

- [x] **T003**: Update escalation-lane guidance in `.harness/workflows/track/review.md`, `.harness/workflows/track/plan.md`, and `.harness/workflows/track/adr2pr.md`. Validate the documents against D5, D6, and D7; IN-02, IN-03, CN-01, CN-02, CN-03, AC-01, AC-02, AC-03, and AC-04.
