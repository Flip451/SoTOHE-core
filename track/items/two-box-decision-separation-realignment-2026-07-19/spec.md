<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 13, yellow: 0, red: 0 }
---

# 入力決定と pipeline 産決定の二箱分離への運用文書整合

## Goal

- [GO-01] track 運用文書が、Phase 0 で user 裁定済みの入力 ADR を収束させて境界を閉じ、Phase 1+ で生じる意味変更を別の delta ADR draft として扱う二箱分離モデルを一貫して規定する。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D2, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D3]

## Scope

### In Scope
- [IN-01] pre-track ADR authoring convention が、Phase 0 の同席 in-place 収束、編集ごとの診断監査、user による収束 diff の裁定、review-refinement の経過措置を含む境界閉鎖と、境界後の入力箱 freeze を明記する。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D2, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D3] [conv: knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権] [tasks: T001]
- [IN-02] adr-diagnoser と adr-editor の capability contracts および対応 skill descriptions が、Phase 0 編集監査、Phase 1+ delta 入庫の三択判定、user 裁定の実装差分に対する conformance 再監査、不一致トリアージ、および各 verdict の責務境界を整合して記述する。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D2, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D4, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5] [tasks: T002]
- [IN-03] track plan、review、adr2pr、merge の workflow SSoTs と対応する thin adapters が、ledger 最新記録を基準とする fail-closed 照合、Phase 0 loop、Phase 1+ の delta 起草・入庫経路、採用・棄却後の再作業、および全 protected source を対象とする terminal audit を同じ運用として記述する。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D1, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D3, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D6] [conv: knowledge/conventions/track-lifecycle.md#ADR baseline lifecycle] [tasks: T003]
- [IN-04] 対象文書群が、admission_class、supersedes、refines の目標 front-matter 形式、判定 verdict、gate 発火、conformance、復旧の運用詳細を規定し、schema と review-refinement kind の Rust 実装は後続 code track に委ねる経過措置を明記する。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D1, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D4, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D6] [conv: knowledge/conventions/adr.md#YAML front-matter] [tasks: T002, T003]

### Out of Scope
- [OUT-01] review-refinement kind と admission_class、supersedes、refines の Rust schema・validator 実装は本 track で行わない。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D2]

## Constraints
- [CN-01] 文書は ADR の決定を逐語的に複製せず、各運用文書の読者が担う行為、境界、gate 条件、失敗時の routing を規定する。orchestrator は意味を自己裁定しない。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D4, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D6] [conv: knowledge/conventions/no-upstream-restatement.md#Rules] [tasks: T001, T002, T003]
- [CN-02] 二箱分離の運用は fail-closed を維持する。境界後の入力箱への意味的 in-place 編集、不確実な delta 入庫、未採用 delta の merge、または未解消の復旧・再作業を許容する経路を文書化してはならない。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D1, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D3, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D4, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5] [conv: knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] pre-track ADR authoring convention は、Phase 0 の収束・user 裁定・境界閉鎖手順と、Phase 1+ の入力箱 freeze・delta 箱への起草手順を分離された節または見出しで記述する。各手順文は、adr-editor、adr-diagnoser、orchestrator、user の担当 actor と処理順序を明記する。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D2, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D3] [conv: knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権] [tasks: T001]
- [ ] [AC-02] adr-diagnoser と adr-editor の contracts は、入庫前・入庫後・user 採用・user 棄却・baseline 不一致の各入力に対し、許可できる編集、required re-audit、verdict 形式、fail-closed の戻り先を矛盾なく示す。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D2, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D4, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D6] [tasks: T002]
- [ ] [AC-03] 更新された track workflow SSoTs と thin adapters は、review 入口で ledger integrity のみを確認し、commit gate と track-aware CI で ledger 最新記録との byte 照合を fail-closed に行い、Phase 1+ の meaning change を delta 起草・入庫・user 裁定・merge recovery・terminal audit へ一貫して接続する。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D1, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D3, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D6] [conv: knowledge/conventions/track-lifecycle.md#ADR baseline lifecycle] [tasks: T003]
- [ ] [AC-04] 対象文書群は、admission_class、supersedes、refines、review-refinement と既存 escalation を使う単一経過措置を目標運用として明示し、対応する schema・kind・validator の実装が未着手でも現行 Rust の実装済み仕様であると誤認させない。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D2, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D4, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5] [conv: knowledge/conventions/adr.md#YAML front-matter] [tasks: T002, T003]
- [ ] [AC-05] merge の terminal audit 文書は、すべての protected source の保護開始記録から終端までの来歴を user に提示し、誤分類と裁定された記録を復元、再監査、必要な downstream rework によって回復する経路を示す。 [adr: knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D5, knowledge/adr/2026-07-19-0616-two-box-decision-separation.md#D6] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#In-track 意味変更の裁定権
- knowledge/conventions/adr.md#YAML front-matter
- knowledge/conventions/track-lifecycle.md#ADR baseline lifecycle
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/no-upstream-restatement.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 13  🟡 0  🔴 0

