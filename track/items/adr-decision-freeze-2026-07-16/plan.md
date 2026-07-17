<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# ADR baseline の累積刻印とバイト照合 binary check による無断改変検出

## Summary

GO-01 → T001, T002, T003, T004, T005, T006, T007.

## Tasks (7/7 resolved)

### S1 — Domain baseline invariants

> Sequence T001 before T002–T007. GO-01.

- [x] **T001**: Implement the T001 domain catalogue entries, including the recorded-copy state, stamp-eligibility policy, and aggregated fail-closed violation responsibilities, in `libs/domain/`; add focused domain tests. IN-01, IN-03, IN-05, CN-01, CN-02, CN-03, CN-04, CN-05, AC-02, AC-03, AC-06, AC-07. (`b530f6aeef2e492e02cd1f2a1476f37bf0199a10`)

### S2 — Application orchestration and durable adapters

> Sequence T002 after T001. GO-01.
> Sequence T003 after T001 and T002. GO-01.

- [x] **T002**: Implement the T002 ADR-baseline command and query use-case catalogue entries, including query output, separately owned read/write storage boundaries, port-specific failures, validation failures, and query-service/interactor responsibilities, in `libs/usecase/`; add focused use-case tests. IN-01, IN-02, IN-03, IN-05, CN-02, CN-03, CN-04, AC-01, AC-03, AC-06, AC-07. (`b530f6ae`)
- [x] **T003**: Implement the T003 filesystem/Git secondary-adapter and ledger-codec catalogue entries in `libs/infrastructure/` and add adapter and transaction regression tests. IN-01, IN-02, IN-03, IN-04, IN-05, CN-01, CN-02, CN-03, CN-04, CN-05, AC-01, AC-02, AC-03, AC-05, AC-06, AC-07. (`b530f6ae`)

### S3 — CLI baseline operation vertical slice

> Sequence T004 after T002. GO-01.
> Sequence T005 after T003 and T004; run `cargo make build-sotp` before T006. GO-01.

- [x] **T004**: Implement the T004 `cli_driver` and `cli` catalogue entries for the ADR-baseline command boundary, including the unresolved composition-facing request, and add CLI-driver and argument-validation tests. IN-01, IN-02, IN-03, CN-02, CN-03, CN-04, AC-01, AC-06. (`b530f6ae`)
- [x] **T005**: Implement ADR-baseline CLI execution, root-command integration, retained dispatch plumbing, and composition-time resolution failures with `AdrBaselineCompositionRoot`; rebuild `bin/sotp` with `cargo make build-sotp`; and add command-level tests. IN-02, IN-03, CN-04, AC-03, AC-04. (`b530f6ae`)

### S4 — Enforcement and reviewer operating policy

> Sequence T006 after T005. GO-01.
> Sequence T007 after T002 and before the review cycle. GO-01.

- [x] **T006**: Wire the completed ADR-baseline commands into `Makefile.toml`; `.harness/workflows/track/{init,review,commit}.md`; and their `.claude/commands/track/{init,review,commit}.md` adapters. Update the corresponding maintainer and user guidance in `CLAUDE.md`, `README.md`, `.claude/rules/{09-maintainer-checklist,10-guardrails}.md`, and `knowledge/conventions/{track-lifecycle,review-protocol}.md`. Leave `.harness/config/signal-gates.json` and the adr_user evaluator untouched. Validate the wiring with the T005 command tests, `cargo make ci`, `cargo make ci-track`, and a focused diff of those untouched artifacts. IN-02, IN-03, IN-05, OS-01, OS-03, CN-03, CN-04, AC-01, AC-04, AC-07, AC-09. (`260fba335848b918223ad96e033b4c4726895d2c`)
- [x] **T007**: Add the read-only `adr-diagnoser` profile in `.harness/config/agent-profiles.json`, its capability SSoT in `.harness/capabilities/adr-diagnoser.md`, and its Codex adapter in `.agents/skills/adr-diagnoser/SKILL.md`; route the mismatch recovery procedure in `.harness/workflows/track/{review,commit,diagnose}.md`. Add the D8 standing clause to `.harness/custom/review-prompts/adr.md` and `.harness/capabilities/review-fix-lead.md`. Validate profile resolution and the declared sandbox, then run focused checks of the named briefing, fixer contract, and workflow routes. IN-04, IN-06, OS-02, OS-04, CN-05, AC-05, AC-08. (`260fba335848b918223ad96e033b4c4726895d2c`)
