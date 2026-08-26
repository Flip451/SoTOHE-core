<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 22, yellow: 0, red: 0 }
---

# Bounded-wait termination assertions for descendant-process tests

## Goal

- [GO-01] 子孫プロセスの終了保証を、即時の単発状態観測ではなく、有界時間内の再観測により最終的な非存在として検証する。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1]
- [GO-02] adr2pr workflow は、計画成果物 commit 後、最初の implementation batch 後、PR lane 開始時を、耐久状態から親 session を再開できる phase boundary として宣言する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4]
- [GO-03] 自動的にコンテキストを管理する host は、宣言済みの phase boundary を停止せず user に refresh を要求せずに継続する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4]

## Scope

### In Scope
- [IN-01] Unix/Linux cfg の `libs/infrastructure/src/review_v2/review_fix_runner/launch_context.rs` にある `test_version_probe_terminates_descendant_after_clean_pipe_drain` を、有界待ちによる再観測の検証様式へ変更する。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [IN-02] 終了保証の判定では、対象が終端状態に達したか消滅するまで再観測し、遷移中の中間状態の観測を失敗として扱わない。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [IN-03] `.harness/workflows/track/adr2pr.md` の parent-session refresh/resume contract、Step 9、gate table を、境界を refresh/resume point として宣言し、unconditional stop と one-way handoff を削除する内容へ改訂する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T002]
- [IN-04] `.claude/commands/track/adr2pr.md` で Claude Code の automatic compaction を automatic context management として扱い、境界ごとの `/clear` または fresh-session 要求を削除する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T002]
- [IN-05] `.agents/skills/track-adr2pr/SKILL.md` の note (5) と cross-reference を、Codex では automatic context management が無い場合にだけ refresh を要求できる内容へ改訂する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T002]

### Out of Scope
- [OUT-01] `TrustedLaunchContext::run_version_probe` とその子孫 cleanup を含む production の終了保証実装は変更しない。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [OUT-02] 同型の即時観測テストを repository 全体で探索または変更することは、この track の対象外とする。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]

## Constraints
- [CN-01] 再観測には有限の時間上限を設け、失敗はその上限を超過した場合に限る。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [CN-02] 対象テストの子孫プロセス終了を再観測する有界待ちには、秒単位で表す固定された有限の時間上限を設け、その上限は対象テストごとの名前付き定数として再観測処理にのみ適用する。上限が過大でも終了保証の判定を誤らせず、超過時のみ失敗とする。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [CN-05] 既存テストの probe script の detached background child について、kernel に reap されて消滅した場合と、非実行中の zombie として残る場合のいずれも、D1 が要求する「終端状態に達した、または消滅した」を満たす。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [CN-06] 最初の implementation batch の `--single-batch` と、git および track artifacts から再開位置を導出する既存の規則は維持し、停止・handoff の意味だけを変更する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T002]
- [CN-07] user への refresh 要求は、自動的にコンテキストを管理できない host に限る条件付きの任意経路とし、無条件の step にしてはならない。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] 対象テストは、即時の単発 `/proc/<pid>/stat` 状態 assert に依存せず、終端状態または対象消滅を確認して成功する。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [ ] [AC-02] 対象テストの有界待ち上限は、秒単位で設定される名前付きのテストごとの定数で表される。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [ ] [AC-03] 対象テストは、有界待ちの時間上限を超過した場合にのみ失敗する。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [ ] [AC-04] production の `TrustedLaunchContext::run_version_probe` と子孫 cleanup の差分は空である。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [ ] [AC-05] 遷移中の状態を一度以上観測しても、時間上限内に終端状態または消滅を観測できれば対象テストは成功する。 [adr: knowledge/adr/2026-08-26-0211-bounded-wait-termination-assertions.md#D1] [tasks: T001]
- [ ] [AC-06] `.harness/workflows/track/adr2pr.md`、`.claude/commands/track/adr2pr.md`、`.agents/skills/track-adr2pr/SKILL.md` のいずれも、自動的にコンテキストを管理する host に対して、宣言済みの境界で無条件に停止すること、または user に `/clear` や fresh session を要求することを指示しない。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T002]
- [ ] [AC-07] adr2pr の SSoT は、計画成果物 commit 後、最初の実装 batch 後、PR lane 開始時の三つの境界と、git および track artifacts からの既存の durable-state resume 導出規則を保持する。 [adr: knowledge/adr/2026-08-22-0145-orchestrator-context-discipline.md#D4] [tasks: T002]

## Related Conventions (Required Reading)
- knowledge/conventions/coding-principles.md#Rules
- knowledge/conventions/testing.md#Testing Convention

## Signal Summary

### Stage 1: Spec Signals
🔵 22  🟡 0  🔴 0

