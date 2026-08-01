<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 16, yellow: 0, red: 0 }
---

# Remote Strict CI Merge Gate

## Goal

- [GO-01] strict な track-aware merge gate を remote CI と branch protection で強制し、gate を通らない merge を構造的に防ぐ。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1]
- [GO-02] `/track:merge` の発行を merge 承認として一度だけ扱い、重複した確認なしに remote CI green 後の merge へ進める。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D2]

## Scope

### In Scope
- [IN-01] remote CI は merge の判定に strict な track-aware gate を実行し、checkout ref から対象 track を解決する。対象 track を解決できない場合は検査を skip せず失敗として扱う。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T001]
- [IN-02] branch protection は remote CI の track-aware gate が失敗した pull request の merge を拒否する。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T002]
- [IN-03] ローカルでの merge gate 実行は早期検知として維持するが、merge 可否を強制する主体は remote CI と branch protection とする。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T004]
- [IN-04] `/track:merge` は invocation 後に追加のユーザー確認を求めず、remote CI が green になるまで待機して merge を実行する。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D2] [tasks: T003]

### Out of Scope
- [OUT-01] remote CI を伴わずローカル merge gate だけで merge 可否を強制する方式へ戻すことは対象外とする。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T001]
- [OUT-02] 対象 track を解決できない remote workflow を成功または graceful skip として扱う例外経路を設けることは対象外とする。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T001]
- [OUT-03] `/track:merge` の発行後に別の merge 承認確認を再導入することは対象外とする。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D2] [tasks: T003]

## Constraints
- [CN-01] remote strict gate の失敗または対象 track の解決不能は merge を fail-closed で停止させなければならない。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T001, T002]
- [CN-02] `/track:merge` の invocation は merge 承認として扱い、その後の workflow と adapter は追加の確認 prompt を表示してはならない。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D2] [tasks: T003]

## Acceptance Criteria
- [ ] [AC-01] remote CI 上で merge gate が実行されると、checkout ref に対応する track を用いて strict な track-aware signal 検査が実行される。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T001]
- [ ] [AC-02] checkout ref から対象 track を解決できない remote CI 実行は graceful skip や成功ではなく失敗になり、merge 可と報告されない。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T001]
- [ ] [AC-03] remote strict gate が失敗する pull request は branch protection により merge できず、gate が成功するまで保護 branch へ取り込めない。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T002]
- [ ] [AC-04] ローカルの merge gate 実行経路は早期検知として利用可能なまま残り、remote gate の結果を置き換える唯一の merge 強制手段にはならない。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D1] [tasks: T004]
- [ ] [AC-05] `/track:merge` の発行後、workflow と adapter は追加の確認を求めず、remote CI が green になるのを待って merge を実行する。 [adr: knowledge/adr/2026-07-29-0839-remote-strict-ci-merge-gate.md#D2] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 16  🟡 0  🔴 0

