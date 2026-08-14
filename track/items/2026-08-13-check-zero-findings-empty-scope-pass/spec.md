<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 13, yellow: 0, red: 0 }
---

# check-zero-findings passes empty scopes

## Goal

- [GO-01] 正規の guarded commit 後の phase pre-entry で `check-zero-findings --scope <scope> --round final` を実行しても、対象 scope の diff が空であることを整合的な不在として扱い、phase entry を妨げないようにする。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1]

## Scope

### In Scope
- [IN-01] `check-zero-findings` が `NotRequired(Empty)` を pass と判定し、pass 出力で対象 scope が空であることを明示する振る舞いを実現する。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [IN-02] D1 の振る舞いを実装した後、`.harness/config/phase-commands.json` に一時的に除去された pre-entry の `check-zero-findings` エントリを復旧する。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D2] [tasks: T2]

### Out of Scope
- [OS-01] 空 scope を pass とするために、最新 round が final zero_findings であるという履歴要件を追加することは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [OS-02] phase gate のために scope diff の base 意味論を再定義することは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [OS-03] pre-entry の `check-zero-findings` エントリを phase 宣言から恒久的に除去することは本 track の対象外とする。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D2] [tasks: T2]

## Constraints
- [CN-01] 空 scope のみを `NotRequired(Empty)` から pass に写し、非空 scope に対する failure の振る舞いは変更しない。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [CN-02] 空 scope による pass の出力は、pass の理由が empty scope であることを利用者に明示しなければならない。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [CN-03] 空 scope pass の健全性は、commit が guarded 経路だけを通り、commit gate が review approval を要求する既存の保証を前提とし、この前提を緩和または代替する変更は含めない。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]

## Acceptance Criteria
- [ ] [AC-01] empty diff の scope に対する `check-zero-findings --scope <scope> --round final` は pass となり、その出力に scope が empty である旨が含まれる。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [ ] [AC-02] 非空 scope において従来 failure となる `check-zero-findings` の条件は、empty scope pass の導入後も failure となる。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [ ] [AC-03] empty scope は final zero_findings verdict の記録がなくても、empty であることだけを理由として pass となる。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D1] [tasks: T1]
- [ ] [AC-04] D1 の実装後、`.harness/config/phase-commands.json` に pre-entry の `check-zero-findings` エントリが存在し、canonical phase entry がその検査を実行できる。 [adr: knowledge/adr/2026-08-13-1736-check-zero-findings-empty-scope-pass.md#D2] [tasks: T2]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 13  🟡 0  🔴 0

