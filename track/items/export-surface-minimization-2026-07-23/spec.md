<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 21, yellow: 0, red: 0 }
---

# 出荷面を最小化し、workflow と出荷物の乖離クラスを閉じる

## Goal

- [GO-01] テンプレート利用者に、その場で意味を持つ最小の規約・skills・rules だけを出荷する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D1, knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D2, knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D3]
- [GO-02] 出荷 workflow と Makefile の既知の乖離クラスを、maintainer CI の template-export-smoke で恒常的に検出できる状態にする。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4]

## Scope

### In Scope
- [IN-01] template boundary manifest の conventions を、各 convention ファイルごとの include または exclude 分類に変更し、未分類の新規 convention を fail-closed で検出する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D1] [tasks: T001]
- [IN-02] export 後の scaffold bootstrap が出荷済み convention 部分集合から convention index を再生成し、export 本体は copy と overlay 上書きだけを続ける。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D1, knowledge/adr/2026-07-06-1717-template-extraction-boundary.md#D4] [tasks: T002]
- [IN-03] 出荷する .claude skills を architecture-customizer だけに限定し、重複した track-plan と diagnose skill のソースを削除し、skill-compliance hook の対応表を更新する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D2] [tasks: T003]
- [IN-04] .claude/rules の番号プレフィックスを除去し、読み順を CLAUDE.md の列挙へ移し、必須 rules を出荷し language rule は中立版 overlay で出荷する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D3] [tasks: T004]
- [IN-05] rules の改名・出荷集合に合わせて、CLAUDE.md、rules 相互参照、workflow の参照を共同更新し、旧番号付き path を残さない。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D3] [tasks: T004]
- [IN-06] maintainer-CI の template-export-smoke に、出荷 workflow が参照する cargo make task の overlay Makefile での欠落と、overlay Makefile への workspace CLI cargo run 流出を検出する二つの恒常チェックを追加し、既存の二件の回帰を修復する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2] [tasks: T005, T006]

### Out of Scope
- [OS-01] convention を汎用用と sotp 開発用の二つの物理ディレクトリへ分割すること。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D1] [tasks: T001]
- [OS-02] track-plan または diagnose skill を export manifest の exclude だけで残し、ソースの論理複製を温存すること。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D2] [tasks: T003]
- [OS-03] rules の番号を別の連番として再導入すること、または maintainer-checklist を常時出荷 rules に含めること。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D3] [tasks: T004]
- [OS-04] 配布 Makefile の採録基準または単発操作の bin/sotp 直呼び原則を再決定すること、あるいは template export 自体を新しい fail-closed 条件で利用者側から停止すること。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2] [tasks: T005, T006]

## Constraints
- [CO-01] 新しい出荷面の整合性チェックは、テンプレート framework 自身の export / workflow / Makefile 整合性だけを対象にし、利用者の provider や agent 設定選択を CI で強制しない。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4] [conv: knowledge/conventions/responsibility-boundary.md#Rules] [tasks: T006]
- [CO-02] 新しい恒常検査は既存の template-export-smoke に内蔵し、新規の独立した verify サブコマンドまたは CI gate 面を増やさない。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4] [conv: knowledge/conventions/workflow-ceremony-minimization.md#Rules, knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T006]

## Acceptance Criteria
- [ ] [AC-01] template boundary manifest が convention ごとの include / exclude を表し、全 convention の出荷可否が一意に分類される。出荷集合には汎用 convention だけが含まれ、sotp 開発固有 convention は exclude として分類される。新規 convention を未分類のまま追加すると export 前の検証が失敗する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D1] [tasks: T001]
- [ ] [AC-02] export 済み scaffold には分類で include された convention だけが存在し、その bootstrap 実行後の convention index は出荷済み集合と一致する。export はファイル内容のプログラム的変換を行わない。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D1, knowledge/adr/2026-07-06-1717-template-extraction-boundary.md#D4] [tasks: T001, T002]
- [ ] [AC-03] export 済み scaffold の .claude/skills には architecture-customizer だけが含まれ、track-plan、diagnose、repomix-snapshot、codex-system、gemini-system は出荷されない。削除対象の重複 skill を参照する skill-compliance hook mapping は残らない。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D2] [tasks: T003]
- [ ] [AC-04] 出荷 rules は番号なしの dev-environment、orchestration、guardrails と中立 language overlay からなり、maintainer-checklist は出荷されない。CLAUDE.md、rules、workflows の参照はこの最終 path 集合と一致する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D3] [tasks: T004]
- [ ] [AC-05] template-export-smoke は export 後の workflow が参照する cargo make task を overlay Makefile が提供しない場合に失敗し、提供される場合に通過する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1] [tasks: T006]
- [ ] [AC-06] template-export-smoke は overlay Makefile に workspace CLI を対象とする cargo run 呼び出しが残る場合に失敗し、配布 scaffold が bin/sotp を利用する場合に通過する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2] [tasks: T006]
- [ ] [AC-07] 既存の workflow の wrapper 参照による task 欠落と、scaffold の ci-track における bin/sotp adr-baseline check-commit 欠落を修復する。maintainer CI の template-export-smoke は、出荷 workflow が参照する cargo make task の欠落と、exported Makefile への workspace CLI cargo run 流出の二種を検出し、修復後に通過する。 [adr: knowledge/adr/2026-07-23-0117-export-surface-minimization.md#D4, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D1, knowledge/adr/2026-07-16-1438-consumer-scaffold-host-first-makefile.md#D2] [tasks: T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/responsibility-boundary.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 21  🟡 0  🔴 0

