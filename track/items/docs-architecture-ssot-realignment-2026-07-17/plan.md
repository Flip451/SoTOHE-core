<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# 同梱運用ドキュメントのアーキテクチャ記述 SSoT 再編

## Summary

Goal coverage: GO-01 is carried by T001/T002/T003; GO-02 is carried by T001/T002/T003/T004/T005. The enforced element mappings are recorded in task-coverage.json.

## Tasks (5/5 resolved)

### S1 — Convention SSoT consolidation

> Execute T001 for the cited convention artifacts (IN-01/IN-02/CN-01/CN-02/AC-01/AC-02).

- [x] **T001**: `knowledge/conventions/hexagonal-architecture.md`; `knowledge/conventions/coding-principles.md`; `knowledge/conventions/README.md`: remove the retired convention, migrate the retained material, regenerate the convention index, and validate the surviving convention citations (IN-01/IN-02/CN-01/CN-02/AC-01/AC-02). (`261b9ede062dec469457863faae19c243354a844`)

### S2 — Placement and DRY guidance corrections

> Execute T002 for the cited rule and workflow artifacts (IN-04/IN-05/IN-06/AC-05/AC-06/AC-07).

- [x] **T002**: `knowledge/conventions/type-designer-kind-selection.md`; `.harness/capabilities/{dry-fix-lead,implementer,review-fix-lead}.md`; `knowledge/conventions/dry-check-workflow.md`; `.claude/agents/dry-fix-lead.md`; `.agents/skills/dry-fix-lead/SKILL.md`; `.gemini/GEMINI.md`; `.claude/skills/{gemini-system,repomix-snapshot}/SKILL.md`: update the cited R1, R3, and R6 rules, the DRY workflow guidance, and all affected architecture guidance (IN-04/IN-05/IN-06/AC-05/AC-06/AC-07). (`261b9ede062dec469457863faae19c243354a844`)

### S3 — Reference and lifecycle-document migration

> Execute T003 after T001 for the cited reference and lifecycle artifacts (IN-03/IN-07/CN-04/AC-03/AC-04/AC-08).

- [x] **T003**: All current working-tree references to `hexagonal-architecture.md`, including rules, review prompts, skills, entry documents, and Rust doc comments; `.claude/skills/architecture-customizer/SKILL.md`; `.claude/rules/09-maintainer-checklist.md`; `knowledge/conventions/task-completion-flow.md`; `CLAUDE.md`: replace retired-document citations, synchronize the architecture-document checklist, and update the entry-document presentation (IN-03/IN-07/CN-04/AC-03/AC-04/AC-08). (`261b9ede062dec469457863faae19c243354a844`)

### S4 — ADR template alignment

> Execute T004 for the two cited ADR template artifacts (IN-08/AC-09).

- [x] **T004**: `overlay/knowledge/adr/README.md`; `knowledge/adr/README.md`: update both ADR templates to the current front-matter format (IN-08/AC-09). (`261b9ede062dec469457863faae19c243354a844`)

### S5 — Release-PR CI gate exception

> Execute T005 for the cited CI workflow conditions and comment (IN-09/CN-03/AC-10).

- [x] **T005**: `.github/workflows/ci.yml`: update the track-aware gate and branch-recreation conditions and their relationship comment (IN-09/CN-03/AC-10). (`261b9ede062dec469457863faae19c243354a844`)
