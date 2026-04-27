<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 27, yellow: 0, red: 0 }
---

# Tier 0 SoT 内の削除済みファイルへの broken reference を解消する

## Goal

- [GO-01] ADR D2 が削除を確定した `TRACK_TRACEABILITY.md` および `knowledge/WORKFLOW.md` への broken reference が Tier 0 SoT ファイル (`.claude/rules/` / `.claude/commands/` / `.claude/agents/`) に残存している状態を解消する。各 broken ref を削除または存続する代替ファイルへの参照に書き換えることで、Tier 0 SoT ファイルが削除済みパスを指し示さない状態にする [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D5: 再発防止運用ルール (5 条) を確立する, knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [GO-02] 前 2 track (`doc-decluttering-deletes-2026-04-27` / `track-traceability-merge-2026-04-27`) で Tier 1/2 ファイルの broken ref は cleanup 済みだが、Tier 0 SoT ファイルは変更されなかった。本 track はその retroactive cleanup を完了させ、Tier 0 SoT の action ファイル群 (`.claude/rules/` / `.claude/commands/` / `.claude/agents/` / `.claude/skills/` / `knowledge/conventions/` / `.harness/config/` / `architecture-rules.json` / `Makefile.toml`) において削除済みファイルへの参照がゼロになる状態を達成する。`knowledge/adr/` は削除対象ファイル名を D2 で意図的に記録しているため本 goal の対象から除外する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D5: 再発防止運用ルール (5 条) を確立する, knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]

## Scope

### In Scope
- [IN-01] `.claude/rules/08-orchestration.md` の 2 箇所の `TRACK_TRACEABILITY.md` 参照を修正する。35 行目の Source Of Truth リストから当該エントリを削除し、48 行目の Operational split 節の説明行を削除する。`TRACK_TRACEABILITY.md` の内容は `track/workflow.md` に merge 済みであり (ADR D2 の §5 merge)、削除が正しい対応である [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [IN-02] `.claude/rules/09-maintainer-checklist.md` の 2 箇所の参照を修正する。13 行目の `knowledge/WORKFLOW.md` 参照を `DEVELOPER_AI_WORKFLOW.md` に書き換え、18 行目の `TRACK_TRACEABILITY.md` 参照を `track/workflow.md` に書き換える。それぞれ ADR D2 が確定した代替 SSoT が存在するため、削除より書き換えが情報を保持できる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [IN-03] `.claude/rules/10-guardrails.md` の 144 行目の `knowledge/WORKFLOW.md` 参照を修正する。Operational details リストのこのエントリを `DEVELOPER_AI_WORKFLOW.md` に書き換える。`DEVELOPER_AI_WORKFLOW.md` が workflow narrative SSoT として ADR D2 で確定しているため [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [IN-04] `.claude/commands/track/setup.md` の 13 行目の `knowledge/WORKFLOW.md` 参照を修正する。`Confirm required top-level docs exist` リストから `knowledge/WORKFLOW.md` を削除する。このファイルはリポジトリに存在しないため、existence check リストからエントリを除去するのが正しい対応である [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [IN-05] `.claude/agents/adr-editor.md` の 61 行目の `TRACK_TRACEABILITY.md` 参照を修正する。この参照は「track artifacts に属するもの (ADR に書いてはいけないもの) の例示リスト」中に現れており、`TRACK_TRACEABILITY.md` を削除してリストを短縮する。`TRACK_TRACEABILITY.md` は ADR に書くべきでない成果物の典型例として列挙されていたが、このファイルは存在しないため例示から除去する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [IN-06] 修正前後に Tier 0 SoT の action ファイル群 (`.claude/rules/` / `.claude/commands/` / `.claude/agents/` / `.claude/skills/` / `knowledge/conventions/` / `.harness/config/` / `architecture-rules.json` / `Makefile.toml`) を grep して `TRACK_TRACEABILITY.md` および `knowledge/WORKFLOW.md` への参照が他にないことを確認する。`knowledge/adr/` は削除対象ファイル名を D2 で意図的に記録しているため grep 対象から除外する。上記 5 ファイルは briefing の観測事実に基づく primary scope だが、実装時に追加の残存参照が発見された場合は同一 track 内で修正する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D5: 再発防止運用ルール (5 条) を確立する]

### Out of Scope
- [OS-01] Tier 1/2 ファイル (`CLAUDE.md` / `DEVELOPER_AI_WORKFLOW.md` / `track/workflow.md` / `knowledge/DESIGN.md` 等) の broken ref は前 2 track で cleanup 済みであり、本 track では変更しない。ADR D1 が Tier 0/1/2 の責務を明確に分離しており、Tier 1/2 の cleanup は別 track に帰属する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する]
- [OS-02] Tier 0 SoT ファイルへの broken ref のうち `tmp/**` / `track/items/<archived>/**` / `knowledge/research/` 等 Tier 3 Knowledge Base 配下の参照は本 track のスコープ外。ADR D1 が Tier 3 には「サイズ制約なし。陳腐化したら本人が delete する文化」と定め、Tier 0 の immutable SoT とは異なる管理規則を適用しているため [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する]
- [OS-03] Tier 0 SoT ファイルの内容変更 (broken ref の修正以外の編集) は本 track のスコープ外。本 track は broken reference の修正のみを行い、Tier 0 ファイルの本文の構造・内容には最小限の変更しか加えない。例: `.claude/rules/08-orchestration.md` の Source Of Truth リストから broken ref を除去することは IN-01/AC-03/T001 で行うが、それ以外の Source Of Truth リストの構成変更 (項目の追加・並べ替え・セクション再編等) は行わない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する]
- [OS-04] broken ref の enforcement mechanism 追加 (CI gate での broken link 検出等) は本 track のスコープ外。ADR D6 #6 `doc-rules-enforcement-2026-04-XX` (任意) が扱う領域であり、本 track は retroactive manual cleanup のみを実施する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]
- [OS-05] `orphan-stragglers-2026-04-XX` ADR D6 #5 の残項目 (`metadata.json schema_version` 参照削除 / `cargo make spec-approve` 参照削除 / `domain-types.json` 単数形置換 / `.claude/docs/` 参照削除) は本 track のスコープ外。本 track は `TRACK_TRACEABILITY.md` / `knowledge/WORKFLOW.md` への broken ref に特化した subset track である [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]
- [OS-06] Rust ソースコードの変更は本 track のスコープ外。`libs/` / `apps/` / `crates/` 配下には一切変更を加えない。本 track の primary 修正対象は `.claude/` 配下の markdown / text ファイルであるが、T003 の grep sweep で `.claude/` 外の Tier 0 action ファイル (`knowledge/conventions/` / `.harness/config/` / `architecture-rules.json` / `Makefile.toml`) に残存 broken ref が見つかった場合はその修正も本 track のスコープ内とする [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Neutral]
- [OS-07] parent ADR (`knowledge/adr/2026-04-27-0554-doc-reorganization.md`) を含む既存 ADR の本文編集は本 track のスコープ外。post-merge ADR は `knowledge/conventions/adr.md` Lifecycle ルールにより immutable (typo 修正 / broken cross-reference 修正 / newer ADR への back-reference 追加 を除く semantic 変更は禁止)。本 track は broken reference cleanup に特化しており ADR への entry 追加は対象外 [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]

## Constraints
- [CN-01] 削除されたファイルへの参照を修正する際、内容が代替ファイルに統合済みの場合は代替ファイルへの参照書き換えを優先する。内容が完全に消失している場合は参照行ごと削除する。`TRACK_TRACEABILITY.md` → `track/workflow.md` / `knowledge/WORKFLOW.md` → `DEVELOPER_AI_WORKFLOW.md` が主な書き換え先となる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [CN-02] 修正対象は broken reference の箇所のみに限定し、Tier 0 SoT ファイルの構造・意味・その他の内容は変更しない。最小差分の原則 (surgical edit) を守る。これにより Tier 0 SoT ファイルの内容が immutable に近い状態を保ちながら broken ref のみを解消できる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する]
- [CN-03] 削除済みファイルと同名の redirect stub / alias ファイルを新規作成しない。これは `no-backward-compat` convention に従い、旧パスへの compatibility layer を排除するものである [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [CN-04] track artifacts (`spec.json` / `<layer>-types.json` / `impl-plan.json` / `task-coverage.json` / `metadata.json`) は本 track では変更しない。`spec.md` / `plan.md` は `bin/sotp` による自動再生成の対象であり、直接編集しない。ADR の新規作成・既存 ADR の編集も行わない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する]

## Acceptance Criteria
- [ ] [AC-01] Tier 0 SoT ファイル群のうち action ファイル (`.claude/rules/` / `.claude/commands/` / `.claude/agents/` / `.claude/skills/` / `knowledge/conventions/` / `.harness/config/` / `architecture-rules.json` / `Makefile.toml`) を対象に `TRACK_TRACEABILITY.md` を grep した結果がゼロ件になる。`knowledge/adr/` は削除対象ファイルの名称を D2 で意図的に記録しているため grep 対象から除外する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D5: 再発防止運用ルール (5 条) を確立する]
- [ ] [AC-02] Tier 0 SoT ファイル群のうち action ファイル (AC-01 と同じ scope、`knowledge/adr/` 除外) を対象に `knowledge/WORKFLOW.md` を grep した結果がゼロ件になる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D5: 再発防止運用ルール (5 条) を確立する]
- [ ] [AC-03] `.claude/rules/08-orchestration.md` が修正されており、Source Of Truth リストおよび Operational split 節から `TRACK_TRACEABILITY.md` の 2 行が除去されている。ファイルの他の内容は変更されていない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [ ] [AC-04] `.claude/rules/09-maintainer-checklist.md` が修正されており、`knowledge/WORKFLOW.md` の参照が `DEVELOPER_AI_WORKFLOW.md` に、`TRACK_TRACEABILITY.md` の参照が `track/workflow.md` に書き換えられている [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [ ] [AC-05] `.claude/rules/10-guardrails.md` が修正されており、Operational details リストの `knowledge/WORKFLOW.md` 参照が `DEVELOPER_AI_WORKFLOW.md` に書き換えられている [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [ ] [AC-06] `.claude/commands/track/setup.md` が修正されており、`Confirm required top-level docs exist` リストから `knowledge/WORKFLOW.md` エントリが除去されている [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [ ] [AC-07] `.claude/agents/adr-editor.md` が修正されており、track artifacts の例示リストから `TRACK_TRACEABILITY.md` エントリが除去されている [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [ ] [AC-08] `cargo make ci` が pass する。Rust コードへの変更はないため fmt-check / clippy / test / deny / check-layers はそのまま通過する。`.claude/` 配下のドキュメント修正による verify-* への影響がないことも確認する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Neutral]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 27  🟡 0  🔴 0

