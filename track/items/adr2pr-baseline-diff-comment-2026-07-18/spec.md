<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 18, yellow: 0, red: 0 }
---

# adr2pr 終端に primary ADR baseline diff の PR コメント投稿フェーズを追加する

## Goal

- [GO-01] `/track:adr2pr` の終端で、Phase 0 の init 刻印から primary ADR がどう変化したかとその来歴を PR 上の 1 コメントとして提示し、merge 裁定者が監査できるようにする。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1]

## Scope

### In Scope
- [IN-01] adr2pr の pr-review 終端状態の直後に、primary ADR の init baseline 複製と終端時点の ADR を比較するコメント投稿フェーズを追加する。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001, T002]
- [IN-02] コメントに終端 diff と、primary ADR source の init 記録以後の ledger 記録から構成した来歴表を含める。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001]
- [IN-03] primary ADR の escalation 刻印 reason に、起点入力の由来・要旨と adr-diagnoser 判定要旨を残す要件を規範へ反映する。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D3] [conv: knowledge/conventions/pre-track-adr-authoring.md#機構との整合] [tasks: T003]

### Out of Scope
- [OS-01] 単発の `/track:pr-review` 実行に同じコメント投稿フェーズを適用すること。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1]
- [OS-02] Rust 実装、CI 設定、または ADR-baseline ledger の record 形式を変更すること。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1, knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D3]
- [OS-03] escalation reason に構造化 schema または固定書式を導入すること。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D3]

## Constraints
- [CN-01] 来歴表は primary ADR の filename と一致する source の記録だけを、当該 source の init 記録より後の append 順で扱い、他 source の記録を含めない。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001]
- [CN-02] コメント先頭の宛先は実行時に解決した PR author の login とし、特定の GitHub ユーザー名をハードコードしない。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D2] [tasks: T001]
- [CN-03] 投稿は `gh pr comment` による 1 コメントで完結させ、pr-review の review-request 経路や自動レビューの再依頼として扱わない。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001]
- [CN-04] コメント投稿フェーズは自律実行し、投稿失敗または過去 escalation 記録の欠落は、復元できない事項を「記録なし」と明示して報告する非致命の扱いにする。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1, knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D3] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] adr2pr が machine PASS または user 承認済み Accepted Deviations で pr-review を終えた場合、primary ADR の init baseline と終端文面の diff を含むコメント投稿フェーズへ進む。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001, T002]
- [ ] [AC-02] 来歴表には、選択された escalation 記録ごとの起点入力・adr-diagnoser 判定要旨・刻印 hash・timestamp・刻印導入コミット、および non-semantic-fix 記録の再刻印である旨・hash・timestamp・導入コミットが示される。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1, knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D3] [tasks: T001]
- [ ] [AC-03] 終端 diff が空でもコメントを投稿し、init 後の primary-source 記録がない場合と中間変更記録がある場合を区別して示す。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001]
- [ ] [AC-04] コメントは実行時に取得した PR author への `@mention` で始まり、単一の `gh pr comment` 投稿として作成される。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D2] [tasks: T001]
- [ ] [AC-05] 過去の escalation 記録で必要な来歴情報を復元できない場合、残る情報で投稿を継続し、推測せず復元不能な要素だけを「記録なし」と示す。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001]
- [ ] [AC-06] コメント投稿に失敗しても、完了済みの adr2pr 成果を失敗扱いにせず、投稿失敗を報告する。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D1] [tasks: T001, T002]
- [ ] [AC-07] primary ADR の escalation 刻印では、local review、外部 PR review、spec→ADR signal、または diagnose routing を識別できる起点入力の由来・要旨と、adr-diagnoser 判定要旨を自由記述 reason に含める。 [adr: knowledge/adr/2026-07-18-0340-adr2pr-baseline-diff-comment.md#D3] [conv: knowledge/conventions/pre-track-adr-authoring.md#機構との整合] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/adr.md#Rule
- knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権
- knowledge/conventions/pre-track-adr-authoring.md#機構との整合
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 18  🟡 0  🔴 0

