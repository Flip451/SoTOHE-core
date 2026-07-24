<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 22, yellow: 0, red: 0 }
---

# scaffold の初期化列を単一タスクへ畳む

## Goal

- [GO-01] export 直後の scaffold 利用者が、明示的に選んだ一つのタスクで初期 git repository、lockfile、初期 commit、および bootstrap 済みの状態へ到達できるようにする。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1]
- [GO-02] 出荷 scaffold の branch strategy 既定を新規 repository の実在 branch と一致させ、初回 workflow の不一致 prompt をなくす。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D2]
- [GO-03] 出荷 command adapter が host 固有の進捗トラッキング機能に依存して workflow を停止しないようにする。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D3]

## Scope

### In Scope
- [IN-01] export された overlay Makefile に、初期化列を実行する明示 opt-in の `cargo make init` task を提供する。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [IN-02] 初期化 task の二回目以降の実行を、既に commit を持つ repository では利用者可読な明示エラーで fail-closed に停止させる。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [IN-03] 出荷 branch-strategy 設定を overlay へ置き、出荷既定を base branch と merge target ともに main にする一方、maintainer 運用値は source repository 側に残す。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D2] [tasks: T002]
- [IN-04] 出荷 plan / adr2pr command adapter の進捗トラッキングを、TaskCreate が利用可能な場合だけ使用し、不在時にはテキスト進捗報告へ代替する。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D3] [tasks: T003]

### Out of Scope
- [OS-01] source repository 側の Makefile へ初期化 task を追加すること。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T002]
- [OS-02] 初期化列を sotp の新しい subcommand として実装すること。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [OS-03] placeholder Cargo.lock を overlay に同梱すること、または Cargo.lock を gitignore に入れること。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [OS-04] bootstrap 自体へ git repository 前提チェックを追加すること。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [OS-05] source repository の maintainer 向け develop-base branch strategy を変更すること。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D2] [tasks: T002]

## Constraints
- [CN-01] 初期化 task は利用者が明示的に選ぶ opt-in command であり、hooks 設置前に許される素の git 操作は同一 command 内の初回初期化列に閉じる。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [CN-02] 初期化列は git init -b main、lockfile 生成、初期 commit、bootstrap の順で実行され、bootstrap により生じる lockfile 差分を残さない。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [CN-03] 出荷 Makefile の task 集合を検査する既存の smoke 検査は、初期化 task を出荷 task として検証する。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T002]
- [CN-04] 出荷 branch strategy は新規 repository の main branch と一致し、maintainer の branch strategy は出荷面へ輸出しない。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D2] [tasks: T002]
- [CN-05] 進捗トラッキングは host 提供機能の有無にかかわらず command adapter の完了を妨げない。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D3] [tasks: T003]

## Acceptance Criteria
- [ ] [AC-01] export 直後の未初期化 scaffold で cargo make init を実行すると、main branch の git repository、生成済み Cargo.lock を含む初期 commit、bootstrap 済みの hooks 設定に到達する。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [ ] [AC-02] commit を持つ repository で cargo make init を再実行すると、ref-update guard の原因不明な停止ではなく、二回目以降を拒否する明示エラーで失敗する。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T001]
- [ ] [AC-03] 出荷 overlay は init task を提供し source repository の Makefile は提供せず、出荷 Makefile task 集合を検査する smoke 検査は task の欠落を検出する。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D1] [tasks: T002]
- [ ] [AC-04] export 済み scaffold の branch-strategy.json は overlay から供給され、base branch と merge target は main である。source repository 側の maintainer 運用値は develop のまま保持される。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D2] [tasks: T002]
- [ ] [AC-05] 出荷 plan / adr2pr command adapter は TaskCreate が利用可能なら進捗トラッキングに使い、不在ならテキスト進捗報告に代替して workflow を停止せず完了する。 [adr: knowledge/adr/2026-07-23-0115-scaffold-first-run-experience.md#D3] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/branch-strategy.md#Rules
- knowledge/conventions/bash-write-guard.md#Design Decision
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/testing.md#Rules
- knowledge/conventions/prefer-type-safe-abstractions.md#Rules
- knowledge/conventions/coding-principles.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 22  🟡 0  🔴 0

