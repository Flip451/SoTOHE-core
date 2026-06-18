<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 30, yellow: 0, red: 0 }
---

# knowledge/strategy ディレクトリの整理方針

## Goal

- [GO-01] knowledge/strategy/ (15 ファイル)、knowledge/designs/ (3 ファイル)、knowledge/schemas/ (2 ファイル)、knowledge/DESIGN.md の計 21 ファイルを working tree から撤去し、knowledge/ 配下の認知負荷を下げる [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D2, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D3, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5]
- [GO-02] 撤去前に各ファイルを精査し、現在の SoTOHE にとって役立つ情報を knowledge/research/ の独立 file に salvage することで、情報損失リスクを低減しながら削除を完遂する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1.1]
- [GO-03] knowledge/DESIGN.md および削除対象ディレクトリ配下の文書 (例: knowledge/strategy/TODO-PLAN.md / TODO.md) を cite している現役 doc (CLAUDE.md / README.md / .claude/rules/ 等) を撤去に追従して更新し、dead link を残さない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5]

## Scope

### In Scope
- [IN-01] knowledge/strategy/ 配下の全 15 ファイルとディレクトリ自体を削除する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1] [tasks: T001]
- [IN-02] knowledge/designs/ 配下の全 3 ファイル (auto-mode-*) とディレクトリ自体を削除する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D2] [tasks: T002]
- [IN-03] knowledge/schemas/ 配下の全 2 ファイル (auto-mode-config-schema.md / auto-state-schema.md) とディレクトリ自体を削除する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D3] [tasks: T002]
- [IN-04] knowledge/DESIGN.md を削除する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T003]
- [IN-05] 各削除対象ファイルを読み込み、現在の SoTOHE にとって役立つ情報を抽出して knowledge/research/YYYY-MM-DD-HHMM-<topic>.md 形式の独立 file に encode してから削除する (salvage フェーズ) [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1.1] [tasks: T001, T002, T003]
- [IN-06] knowledge/DESIGN.md および削除対象ディレクトリ (knowledge/strategy/ / knowledge/designs/ / knowledge/schemas/) 配下の文書を cite している現役 doc (CLAUDE.md / README.md / .claude/rules/ / .claude/commands/ / .claude/settings.json / .codex/instructions.md / knowledge/conventions/ 等) から該当参照を削除または代替 SSoT への参照に更新する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T004]
- [IN-07] knowledge/README.md の Directory Structure 表から strategy/ / designs/ / schemas/ の行と knowledge/DESIGN.md の参照を削除し、実態と一致させる [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T005]
- [IN-08] knowledge/conventions/adr.md の Decision Reference セクションにある knowledge-restructure-design-2026-03-20.md (strategy/ 配下) への参照を削除する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4] [tasks: T006]

### Out of Scope
- [OS-01] main にマージ済みの過去 ADR (2026-03-30-0546-knowledge-directory-consolidation.md 等) に残る knowledge/strategy/ / knowledge/designs/ / knowledge/schemas/ / knowledge/DESIGN.md への参照は撤去後 dead link となるが更新しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4]
- [OS-02] 完了済み track (done 状態) の spec.json / impl-plan.json 等に埋め込まれた knowledge/strategy/ / knowledge/designs/ / knowledge/schemas/ / knowledge/DESIGN.md への参照は更新しない。過去 track の artifact は歴史的記録であり参照整合の検証は現行 track のみが対象 [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4]
- [OS-04] Rust ソースコード (libs/ / apps/) の変更は行わない。本 track は doc 操作のみ [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1]
- [OS-05] knowledge/adr/2026-03-24-0930-adr-auto-derivation-design.md (sotp adr suggest 設計 ADR) の deprecate / supersede 判断は本 track のスコープ外。撤去で前提が変わることは Consequences に記載されているが、対応 ADR の作成は別 track で扱う [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1]

## Constraints
- [CN-01] 既存 SoT (ADR / convention / CLAUDE.md / track/tech-stack.md 等) への inline 追記による salvage は重要文書を汚染するため禁止する。encode 先は knowledge/research/YYYY-MM-DD-HHMM-<topic>.md 形式の独立 file に限定する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1.1] [tasks: T001, T002, T003]
- [CN-02] 撤去後に dead link となる過去 ADR の参照は追従して更新しない。ADR の post-merge 不変原則に従い過去 ADR は当時の文脈の記録として残す [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4] [tasks: T001, T002, T003, T004, T005, T006]
- [CN-03] 現役 doc (knowledge/conventions/ / CLAUDE.md / README.md / .claude/rules/ 等、SoT として運用中の文書) は D4 の対象外であり、撤去に追従して参照を更新する [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4] [tasks: T004, T005, T006]
- [CN-04] salvage 判定 (どの情報を残すか) の判断基準: 現在の SoTOHE の ADR / convention / track artifact にまだ encode されていない固有情報があれば salvage 候補とし、重複または陳腐化した情報は salvage しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1.1] [tasks: T001, T002, T003]

## Acceptance Criteria
- [ ] [AC-01] knowledge/strategy/ ディレクトリが存在しない (git status で削除として記録される) [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1] [tasks: T001]
- [ ] [AC-02] knowledge/designs/ ディレクトリが存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D2] [tasks: T002]
- [ ] [AC-03] knowledge/schemas/ ディレクトリが存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D3] [tasks: T002]
- [ ] [AC-04] knowledge/DESIGN.md が存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T003]
- [ ] [AC-05] 各削除対象ファイルを読み込んで salvage 判定 (役立つ情報あり / なし) を実施した上で削除している。salvage ありと判定したファイルについては削除前に knowledge/research/ 配下の独立 file への encode が完了している [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1.1] [tasks: T001, T002, T003]
- [ ] [AC-06] knowledge/research/ 配下に salvage として encode した各 file の名前が YYYY-MM-DD-HHMM-<topic>.md 形式であり、ADR / convention / CLAUDE.md / track/tech-stack.md 等の既存 SoT ファイルへの inline 追記が行われていない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D1.1] [tasks: T001, T002, T003]
- [ ] [AC-07] CLAUDE.md に knowledge/DESIGN.md および削除対象ディレクトリ (knowledge/strategy/ / knowledge/designs/ / knowledge/schemas/) 配下への参照が存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T004]
- [ ] [AC-08] README.md に knowledge/DESIGN.md および削除対象ディレクトリ (knowledge/strategy/ / knowledge/designs/ / knowledge/schemas/) 配下への参照が存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T004]
- [ ] [AC-09] T004 対象の現役 doc (.claude/rules/ / .claude/commands/ / .claude/settings.json / .codex/instructions.md / .claude/skills/ / knowledge/conventions/ 等) に knowledge/DESIGN.md または削除対象ディレクトリ (knowledge/strategy/ / knowledge/designs/ / knowledge/schemas/) 配下への参照が存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T004]
- [ ] [AC-10] knowledge/README.md の Directory Structure 表に strategy/ / designs/ / schemas/ の行が存在せず、Related Top-Level Files に knowledge/DESIGN.md への参照が存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4, knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D5] [tasks: T005]
- [ ] [AC-11] knowledge/conventions/adr.md の Decision Reference セクションに knowledge-restructure-design-2026-03-20.md (strategy/ 配下) への参照が存在しない [adr: knowledge/adr/2026-06-17-1321-knowledge-strategy-cleanup.md#D4] [tasks: T006]

## Related Conventions (Required Reading)
- knowledge/conventions/adr.md#Lifecycle
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/track-lifecycle.md#Rules

## Signal Summary

### Stage 1: Spec Signals
🔵 30  🟡 0  🔴 0

