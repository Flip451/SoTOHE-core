<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# ADR-baseline の review 入口検査を init 刻印の存在確認のみに縮小する

## Goal

- [GO-01] ADR-baseline review の入口を、主対象 ADR の init 刻印が ledger に存在することを確認する gate に縮小し、Phase 0 の無刻印レビュー・修正ループを規範どおりに収束可能にする。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1]

## Scope

### In Scope
- [IN-01] adr-baseline check-review を、非空 ledger、init record、およびその init baseline 複製の存在を検査する review 入口 gate として扱い、記録済み ledger 複製の整合検証は維持する。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T001]
- [IN-02] review 中に init baseline と現行 ADR が乖離していても、init 刻印の存在を満たす限り review 入口を通過できるようにする。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T001]
- [IN-03] D2 に従い、review / plan / adr2pr の workflow SSoT に、track 内 ADR 編集ごとの adr-diagnoser 判定と、その保全代案または修正不要理由を所見の発生元へ還流する手順を反映する。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D2] [conv: knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権, knowledge/conventions/pre-track-adr-authoring.md#機構追随待ち] [tasks: T002, T003]
- [IN-04] plan / adr2pr の workflow SSoT に、Phase 0 で zero_findings 後の user 承認エスカレーションを自律実行の明示的な例外として扱う carve-out を反映する。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T003]

### Out of Scope
- [OS-01] snapshot kind への専用の承認 kind の追加、または check-commit に承認記録を必須化すること。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1]
- [OS-02] check-commit または track-aware CI における、記録済み ADR と最新 baseline の byte 照合および fail-closed な無断改変検出を変更すること。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1]
- [OS-03] ADR-baseline ledger の形式、累積記録の扱い、または escalation snapshot の既存経路を変更すること。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1]
- [OS-04] adr_user signal 評価、signal-gates の strictness、または spec が cite する ADR の coverage 検査を変更すること。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1]

## Constraints
- [CN-01] review 入口から除く ADR 状態の block 条件は現行 ADR と最新 baseline の byte 不一致だけとし、init designation の欠落と記録済み ledger 複製の不整合は review 前に fail-closed とする。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T001]
- [CN-02] Phase 0 のレビュー・修正ループ中は ledger に中間刻印を追加せず、編集を伴って収束した場合だけ user 承認 ref を対象 decision に記録して既存 escalation 経路で刻印する。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T002, T003]
- [CN-03] 編集を伴わずに Phase 0 が user 承認へ収束した場合は、init 記録を承認文面として維持し追加刻印を行わない。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T002, T003]

## Acceptance Criteria
- [ ] [AC-01] adr-baseline check-review は、ledger が欠落または空、init record がない、または init record が指す baseline 複製がない場合に review 開始前に失敗する。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T001]
- [ ] [AC-02] 有効な init record とその複製を持つ track では、現行 ADR が最新 baseline と byte 不一致でも adr-baseline check-review がその不一致を理由に失敗せず、reviewer または fixer が review を開始できる。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T001]
- [ ] [AC-03] check-commit と track-aware CI は、記録済み ADR の現行文面と最新 baseline の byte 不一致を従来どおり fail-closed で検出し、spec が cite する ADR の coverage 検査も維持する。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T001]
- [ ] [AC-04] review workflow は review 入口を init designation の早期検出として記述し、review 中の byte 不一致を ADR-baseline recovery route へ送らない。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T002]
- [ ] [AC-05] D2 に従い、review、plan、および adr2pr の workflow SSoT は、ADR 編集の直後に adr-diagnoser の決定保存判定を実施し、決定破壊の verdict が示す保全代案または修正不要理由を所見の発生元へ返す。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D2] [conv: knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権, knowledge/conventions/pre-track-adr-authoring.md#機構追随待ち] [tasks: T002, T003]
- [ ] [AC-06] plan と adr2pr の workflow SSoT は、Phase 0 の zero_findings 後に user 承認エスカレーションを実行でき、その承認により ADR 文面が変わる場合は fresh review から再収束させる。 [adr: knowledge/adr/2026-07-17-1203-adr-baseline-review-gate-init-existence-only.md#D1] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/adr.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/track-lifecycle.md#Generated Views
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

