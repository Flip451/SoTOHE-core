<!-- Generated from metadata.json + impl-plan.json — DO NOT EDIT DIRECTLY -->
# ADR decision 根拠信号機: review 優先化 + grounding 値オブジェクト検証

## Tasks (6/6 resolved)

### S1 — Domain layer: DecisionGroundRef newtype

> Introduce the validated DecisionGroundRef value object in domain, satisfying IN-04 (newtype with try_new rejecting empty/whitespace), AC-05, AC-06, AC-07, and CN-01 (fail-closed, no silent None normalization).
> Placing the type in domain and exporting it from adr_decision satisfies CN-04 (no serde in domain).
> EmptyDecisionGroundRef variant is added to ValidationError so the existing DomainError::from(ValidationError) propagation chain carries the new error kind without changes to DomainError.

- [x] **T001**: Add DecisionGroundRef newtype to domain: new struct in grounds.rs, try_new/as_str impls, trait impls (Debug/Clone/PartialEq/Eq), and EmptyDecisionGroundRef variant in ValidationError; export via adr_decision::mod.rs. (`daa79d36e81f013ce9a890c8eab68c9c1626b582`)

### S2 — Domain layer: AdrDecisionCommon field type migration

> Migrate user_decision_ref/review_finding_ref from Option<String> to Option<DecisionGroundRef> in AdrDecisionCommon (IN-05).
> Updating the test helper common_with in evaluator.rs is mandatory because it constructs AdrDecisionCommon directly; it must use DecisionGroundRef::try_new to compile after the signature change.
> Depends on T001 (DecisionGroundRef must exist before AdrDecisionCommon can reference it).

- [x] **T002**: Update AdrDecisionCommon to use Option<DecisionGroundRef>: change user_decision_ref/review_finding_ref field types, update new() signature, update accessor return types to Option<&DecisionGroundRef>; update evaluator.rs test helper common_with to construct via DecisionGroundRef::try_new. (`daa79d36e81f013ce9a890c8eab68c9c1626b582`)

### S3 — Domain layer: classify_grounds priority inversion

> Invert the priority inside classify_grounds so review_finding_ref is checked first (IN-01, D1 core fix).
> Update the DecisionGrounds doc comment to describe the new review-priority rule (IN-02).
> Rename and update the test that previously asserted user_ref wins, so it now asserts both-ref yields ReviewFindingRef (IN-03, AC-01).
> Depends on T002 because common.user_decision_ref() and common.review_finding_ref() now return Option<&DecisionGroundRef>; classify_grounds checks .is_some() which works on that type without further change, but the compilation of the updated test helper requires T002.

- [x] **T003**: Invert classify_grounds priority in evaluator.rs (review_finding_ref checked before user_decision_ref); update grounds.rs doc comment to describe review-priority rule; rename and invert test_evaluate_adr_decision_user_ref_takes_priority_over_review_ref to assert both-ref yields ReviewFindingRef. (`daa79d36e81f013ce9a890c8eab68c9c1626b582`)

### S4 — Infrastructure layer: DTO-to-domain conversion hardening

> Modify decision_dto_to_entry so that Option<String> DTO fields are converted to Option<DecisionGroundRef> via DecisionGroundRef::try_new, with Err mapped to AdrFrontMatterCodecError::InvalidDecisionField (IN-06, CN-01 fail-closed).
> Add parse test verifying that an empty string for user_decision_ref or review_finding_ref is rejected (AC-08).
> Depends on T002 (AdrDecisionCommon::new signature now takes Option<DecisionGroundRef>).

- [x] **T004**: Update decision_dto_to_entry in infrastructure parse.rs: convert Option<String> DTO fields to Option<DecisionGroundRef> via DecisionGroundRef::try_new, propagating Err as AdrFrontMatterCodecError::InvalidDecisionField; add parse test for empty placeholder rejection. (`daa79d36e81f013ce9a890c8eab68c9c1626b582`)

### S5 — Convention update: adr.md grounds table

> Update knowledge/conventions/adr.md so that the review_finding_ref row in the YAML front-matter grounds table reflects the new rule: review present yields Yellow regardless of user_decision_ref (IN-07, AC-10).
> This is an independent documentation change with no Rust compilation dependency; can be committed in any order relative to S1-S4.

- [x] **T005**: Update knowledge/conventions/adr.md grounds table: change review_finding_ref row to state review-present yields Yellow regardless of user_decision_ref; remove the old 'user_decision_ref 未設定なら 🟡' wording. (`daa79d36e81f013ce9a890c8eab68c9c1626b582`)

### S6 — Baseline verification: legacy ADR D1 superseded state

> Confirm that 2026-04-27-1234-adr-decision-traceability-lifecycle.md D1 already has status: superseded and superseded_by set at the baseline commit (IN-08, AC-09, CN-05).
> The ADR file was already modified by the adr-editor run included in the Phase 0-2 baseline (HEAD 0a9114df). This task records the explicit check and closes the spec element without further code change.

- [x] **T006**: Verify that the legacy ADR 2026-04-27-1234 D1 front-matter already has status: superseded and superseded_by pointing to 2026-06-16-0042#D1 (baseline state at HEAD 0a9114df); no code change required — confirm and close. (`0a9114df4b9d4f901da8533bd5f10e10574e9b16`)
