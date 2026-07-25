<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# Phase 0 再収束レーンと派生義務成果物のレビュー範囲

## Goal

- [GO-01] Phase 0 で user 承認後の ADR 文面に修正が入った場合、承認前の収束段階へ戻して review loop と修正後全文の再承認を経ることで、修正前の承認を再利用せずに裁定境界を閉じる。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1]
- [GO-02] 型カタログから機械導出される obligations.json を review_operational として扱い、再導出だけで内容レビューを失効させない一方、書き手が明示的に作成する test-bindings.json はレビュー対象に維持する。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1]

## Scope

### In Scope
- [IN-01] knowledge/conventions/pre-track-adr-authoring.md を、Phase 0 の user 承認後に修正が加わった ADR を承認前の収束 loop へ戻し、収束後に修正後全文への user 再承認を要求する規範の正として更新する。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T1]
- [IN-02] Phase 0 の再収束を案内する workflow SSoT と対応する command / skill adapter を、前記 convention を規範の正として参照する記述へ同期し、独自の手順再記述や承認済み状態のまま guardian lane を継続する経路を残さない。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T2, T3]
- [IN-03] .harness/config/review-scope.json の review_operational に track ごとの obligations.json を加え、同成果物を内容レビューのスコープ分類から除外する。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1] [tasks: T4]

### Out of Scope
- [OS-01] test-bindings.json を review_operational へ加えること、または test-bindings.json の内容レビューを免除することは本 track の対象外とする。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1] [tasks: T4]
- [OS-02] commit gate の test-obligation check、.harness/config/signal-gates.json、または adr_user signal 評価を変更することは本 track の対象外とする。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1] [tasks: T4]
- [OS-03] 承認記録の ADR front-matter 所在、Phase 0 境界刻印、commit gate の ADR byte 照合、または adjudication-ready 経路を変更することは本 track の対象外とする。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T1, T2, T3]

## Constraints
- [CN-01] 修正が意味変更か、guardian が決定保存的と判定したかを問わず、承認後に変更された文面へ修正前の user 承認を流用してはならない。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T1, T2, T3]
- [CN-02] 再収束の具体的手順と Phase 0 境界処理は pre-track-adr-authoring convention を唯一の規範の正とし、workflow SSoT と command / skill adapter はその規範を重複して定義しない。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T1, T2, T3]
- [CN-03] review_operational への分類基準は成果物の重要度ではなく、その内容が機械導出であることとし、明示的 authoring を含む test-bindings.json へ拡張してはならない。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1] [tasks: T4]

## Acceptance Criteria
- [ ] [AC-01] pre-track-adr-authoring convention は、user 承認後に ADR 文面が修正された場合に承認前の収束 loop を再開し、findings の収束後に修正後の全体を user へ再提示して再承認を得ることを定める。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T1]
- [ ] [AC-02] Phase 0 の再収束を記述する workflow SSoT と対応する command / skill adapter は pre-track-adr-authoring convention を参照し、承認済みの状態を維持したまま通常の guardian lane を継続する手順を指示しない。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T2, T3]
- [ ] [AC-03] 再収束した修正後の ADR 文面は、修正前の user 承認を流用せず、user の再承認を得るまで Phase 0 裁定境界を閉じない。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T1, T2, T3]
- [ ] [AC-04] .harness/config/review-scope.json の review_operational は track/items/<track-id>/obligations.json に対応する pattern を含み、obligations.json は内容レビューのスコープへ分類されない。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1] [tasks: T4]
- [ ] [AC-05] review_operational に test-bindings.json の pattern は追加されず、test-bindings.json は内容レビューの対象として残る。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1] [tasks: T4]
- [ ] [AC-06] 変更後も commit gate の test-obligation check は維持され、obligations.json の review_operational 分類は同 check の実行を変更しない。 [adr: knowledge/adr/2026-07-25-0715-derived-obligation-artifact-review-scope.md#D1] [tasks: T4]
- [ ] [AC-07] 変更後も承認記録は ADR front-matter にあり、境界刻印、commit gate の byte 照合、adjudication-ready 経路はいずれも従来の扱いを維持する。 [adr: knowledge/adr/2026-07-25-0716-phase0-post-approval-reconvergence-lane.md#D1] [tasks: T1, T2, T3]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Phase 0 — 裁定境界まで
- knowledge/conventions/adr.md#Lifecycle: pre-merge draft vs post-merge record
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- .claude/rules/maintainer-checklist.md#Maintainer Checklist

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

