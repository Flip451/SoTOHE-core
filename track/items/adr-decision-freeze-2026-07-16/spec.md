<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 25, yellow: 0, red: 0 }
---

# ADR baseline の累積刻印とバイト照合による無断改変検出

## Goal

- [GO-01] track が対象とする user 承認済み ADR の正規文面を track-local baseline として保護し、track 中の無断な意味変更を review 入口・commit・CI で機械的に検出、復元、可視化できるようにする。正規の ADR 修正と user 承認前の draft 起草は、それぞれ定められた経路を通じて妨げない。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D1, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7]

## Scope

### In Scope
- [IN-01] track が対象とする ADR の逐語 baseline 複製と、その複製版を識別する ledger を track 配下に保持する。baseline は無断改変検出と機械復元の錨として利用する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D1] [tasks: T001, T002, T003, T004, T005]
- [IN-02] 初回刻印、後続 cite、正規の再刻印および新規 ADR の凍結域への導入を、orchestrator が対象 ADR を明示して起動する専用 snapshot 経路で扱う。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D2, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T002, T003, T004, T005, T006]
- [IN-03] 刻印済み ADR の現行バイト列を最新 baseline と照合し、不一致または保護対象の必要な刻印の欠落を fail-closed で block する binary check を review 入口、commit gate、track-aware CI に配置する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4] [tasks: T001, T002, T003, T004, T005, T006]
- [IN-04] 検出された不一致を read-only の adr-diagnoser が baseline との差分からトリアージし、非意味的編集だけを再刻印経路へ進め、意味に触れる変更は baseline から機械復元して編集元へ差し戻す。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5] [tasks: T003, T007]
- [IN-05] spec が後続で cite する既存 ADR を track 分岐点の committed 文面から刻印し、track 中に生まれた ADR は user_decision_ref を得るまで draft として刻印要求から除外し、昇格後は刻印を必須にする。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T001, T002, T003, T006]
- [IN-06] ADR reviewer briefing と fixer capability の運用契約に、baseline の意味論を in-place 変更せず、意味に関わる指摘を amendment 提案として報告する常設規定を置く。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D8] [tasks: T007]

### Out of Scope
- [OS-01] chain ⓪（adr_user）信号評価器および signal-gates の strictness 設定を変更すること。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4] [tasks: T001, T002, T006]
- [OS-02] ADR 本文の散文解析、意味判定、または LLM を gate の判定経路に組み込むこと。LLM は block 後の diagnoser トリアージにのみ用いる。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5] [tasks: T003, T007]
- [OS-03] pre-track ADR authoring / promotion の儀式または、track と無関係な ADR を凍結対象へ拡張すること。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T002, T006]
- [OS-04] amendment 提案の内容を本 track 内で user 裁定なしに既存 ADR 本文へ反映すること。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D8] [tasks: T007]

## Constraints
- [CN-01] baseline 複製と ledger は累積かつ append-only とし、既存の複製を上書きまたは削除しない。同一内容の再刻印では複製を重複作成せず ledger のみ追記し、内容が異なる hash prefix の衝突は一意になるまで拡張する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D3] [tasks: T001, T003]
- [CN-02] baseline への書込は snapshot 機構だけに集約し、複製追加と ledger 追記を原子的に行う。書込を行う kind は init、cite、new-adr、non-semantic-fix、escalation の五種に限定し、escalation と new-adr では自己完結した reason を必須にする。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6] [tasks: T001, T002, T003, T004]
- [CN-03] 初回刻印の対象 ADR は orchestrator が knowledge/adr/ 直下の file 名として明示し、cite 刻印は track 分岐点の committed 文面だけを複製元とする。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D2, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T001, T002, T003, T004, T006]
- [CN-04] 保護対象の刻印不足、照合不一致、required reason の欠落、または昇格済み新規 ADR の未刻印は fail-closed とする。一方、分岐点に存在せず user_decision_ref を持たない track 生まれの draft ADR は承認まで刻印要求から除外する。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T001, T002, T003, T004, T005, T006]
- [CN-05] 不一致のトリアージでは非意味的編集のみ再刻印を許し、意味に触れる変更または判断不能な変更は逸脱として扱う。diagnoser は read-only で verdict のみを返し、復元は最新 baseline のバイト列を機械コピーする。 [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6] [tasks: T001, T003, T007]

## Acceptance Criteria
- [ ] [AC-01] track init workflow completes the existing track initialization and then records a verbatim init baseline plus ledger entry for the orchestrator-specified primary ADR; the saved baseline is sufficient to byte-compare and restore that ADR later. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D1, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D2] [tasks: T001, T002, T003, T004, T005, T006]
- [ ] [AC-02] Repeated snapshots preserve every prior baseline and ledger record, allowing a reviewer to byte-diff the init baseline and latest baseline in the track-local directory. A repeated identical hash appends only a ledger entry, while distinct content obtains a non-conflicting baseline filename and full SHA-256 ledger hash. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D3, knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5] [tasks: T001, T003]
- [ ] [AC-03] The binary check passes when each recorded ADR matches its latest baseline and all required baselines exist; it blocks deterministically on a byte mismatch or on a missing required baseline without parsing ADR prose or consulting an LLM. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4] [tasks: T001, T002, T003, T005]
- [ ] [AC-04] The same freeze check runs before review work begins, in the guarded commit path, and in the track-aware CI path used both locally and for PR CI. The review entrance specifically blocks a missing primary-ADR init snapshot before a fixer can modify the worktree. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4] [tasks: T005, T006]
- [ ] [AC-05] On a detected mismatch, the orchestrator receives a diagnoser verdict: a non-semantic verdict permits a new snapshot and retry; a semantic or uncertain verdict restores the latest baseline and returns the originating work through a briefing that requests an amendment proposal instead of an in-place ADR edit. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D5] [tasks: T003, T007]
- [ ] [AC-06] The snapshot operation rejects unsupported kinds, accepts reason only where its kind permits it, and blocks escalation or new-adr snapshots lacking the required self-contained reason. No capability manually writes a baseline copy or ledger record. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D6] [tasks: T001, T002, T003, T004]
- [ ] [AC-07] When a later phase cites an existing ADR, its cited baseline is taken from the track merge-base and commit validation blocks if that cited ADR was not stamped. A track-created ADR remains exempt while it has no user_decision_ref, then blocks if it is promoted without its required new-adr snapshot. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D7] [tasks: T001, T002, T003, T006]
- [ ] [AC-08] ADR reviewer and fixer instructions explicitly prohibit changing ADR meaning from the baseline and direct meaningful findings to an amendment proposal. Non-semantic fixes remain available through the diagnoser and snapshot route. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D8] [tasks: T007]
- [ ] [AC-09] Implementation leaves the adr_user signal evaluator and all signal-gates strictness cells unchanged; the new enforcement is confined to the independent binary freeze check and its orchestration points. [adr: knowledge/adr/2026-07-16-2001-adr-decision-freeze.md#D4] [tasks: T006]

## Related Conventions (Required Reading)
- knowledge/conventions/adr.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/review-protocol.md#Rules
- knowledge/conventions/track-lifecycle.md#Generated Views
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/branch-strategy.md#Rules
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Rule

## Signal Summary

### Stage 1: Spec Signals
🔵 25  🟡 0  🔴 0

