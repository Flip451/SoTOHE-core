<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 23, yellow: 0, red: 0 }
---

# ドキュメント断捨離 — 重複 narrative の即時削除 (knowledge/WORKFLOW.md / knowledge/architecture.md)

## Goal

- [GO-01] ADR D2 が「即時削除対象」と確定した `knowledge/WORKFLOW.md` および `knowledge/architecture.md` を削除し、同一トピックに narrative が 2 つ以上存在する状態を解消する。これにより `DEVELOPER_AI_WORKFLOW.md` が workflow narrative の SSoT として一本化され、`knowledge/DESIGN.md` が architecture narrative の唯一の参照点になる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [GO-02] 削除対象ファイルを参照していた Tier 1/2 derived 文書のリンクを更新し、削除後に Tier 1/2 ファイル中に broken link が残らないようにする。Tier 0 SoT ファイル (ADR / commands / rules / conventions) への参照は CN-02 により本 track では変更しないため、Tier 0 内の残存参照は許容される [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Consequences]
- [GO-03] `.gitignore` 済みの worktree scratch ファイル (`repomix-output.*`) を削除し、worktree を clean にする [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]

## Scope

### In Scope
- [IN-01] `knowledge/WORKFLOW.md` を削除する。`DEVELOPER_AI_WORKFLOW.md` が全章で重複しており、ADR D2 が削除を確定している。削除後に derived 文書からこのファイルへのリンクが残らないことを確認する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [IN-02] `knowledge/architecture.md` を削除する。自称「`knowledge/DESIGN.md` の slim 版」だが両方とも古く、slim 版を残すと「最新ではない」というメタ情報を 2 重に持つだけになると ADR D2 が判断している。削除後に derived 文書からのリンクが残らないことを確認する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [IN-03] worktree に残存する `repomix-output.*` ファイル群を削除する。これらは `.gitignore` 済み scratch ファイルであり、ADR D2 が副次対応として削除を指定している [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [IN-04] 削除した 2 ファイルへのリンクを持つ derived 文書 (`CLAUDE.md` の priority references 等) を更新し、リンク切れを解消する。リンクを削除するか、代替 SoT (`DEVELOPER_AI_WORKFLOW.md` / `knowledge/DESIGN.md`) への差し替えを行う [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]

### Out of Scope
- [OS-01] `TRACK_TRACEABILITY.md` の削除は本 track のスコープ外。ADR D2 はこのファイルを即時削除対象の 1 つとして確定しているが、削除前に `§5 (registry.md Update Rules)` を `track/workflow.md` に merge する必要があるため、ADR D6 が別 track (`track-traceability-merge-...`) に分離した。本 track は D6 の分割決定に従っており D2 との矛盾ではない (D2 は削除方針の確定、D6 が実装 track 分割を決定した) [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]
- [OS-02] `knowledge/DESIGN.md` の heavy shrink (ADR D3.1) は本 track のスコープ外。ADR D6 が `design-md-shrink-...` track として分離しており、content 編集を伴うため別 track で blast radius を制御する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]
- [OS-03] `README.md` / `START_HERE_HUMAN.md` / `LOCAL_DEVELOPMENT.md` の縮約 (ADR D3.2-4) は本 track のスコープ外。ADR D6 が `readme-and-entry-points-shrink-...` track として分離しており、content 編集を伴うため別 track で扱う [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]
- [OS-04] `DEVELOPER_AI_WORKFLOW.md` への content 追加・編集は本 track のスコープ外。ADR D3 補足で現状サイズ維持かつ細部修正のみとされており、workflow SSoT の編集は本 track の blast radius に含めない。細部修正の track 帰属は ADR D6 では明示されていない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D3: Heavy shrink 対象 (4 ファイル) のスコープを定める]
- [OS-05] D5 の再発防止運用ルール (Tier 1 size limit の CI gate 化等) は本 track のスコープ外。ADR D6 が `doc-rules-enforcement-...` track (任意) として分離している [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]
- [OS-06] Rust コードの変更は本 track のスコープ外。削除対象はドキュメントファイルと scratch ファイルのみであり、ソースコード (`libs/` / `apps/` / `crates/`) には一切変更を加えない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Neutral]

## Constraints
- [CN-01] 削除後に migration shim (削除ファイルと同名の redirect stub / alias ファイル) を作らない。ADR D2 が削除を確定しており、`no-backward-compat` convention に従い旧パスへの compatibility layer は導入しない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [CN-02] 既存の Tier 0 SoT ファイル (ADR 本文 / `.claude/commands/` / `.claude/rules/` / `.claude/skills/` / `.harness/config/agent-profiles.json` / `architecture-rules.json` / `Makefile.toml` / `knowledge/conventions/` / track artifacts) は本 track では変更 (modify / delete) しない。変更対象は Tier 1 entry-point index (derived link 削除) および deleted Tier 2 narrative ファイルに限定する。例外 1: CN-04 による pre-merge ADR 本文 (`knowledge/adr/2026-04-27-0554-doc-reorganization.md`) の新規追加 (staging) は「既存ファイルの変更」ではないため対象外。例外 2: `knowledge/adr/README.md` (ADR index) への新規 ADR エントリ追加は、新規 ADR commit の必須終端処理であり許容される変更 (手動 index としての性質上、新規 ADR 追加のたびに更新が必要なため)。ADR D1 Tier 0 列は `.claude/rules/` と明記しており、全 `.claude/rules/*.md` ファイルを Tier 0 として扱う (`.claude/rules/09-maintainer-checklist.md` / `.claude/rules/10-guardrails.md` 等を含む) [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する] [tasks: T001]
- [CN-03] `TRACK_TRACEABILITY.md` には変更を加えない。このファイルは別 track (`track-traceability-merge-...`) での content merge + 削除が先行するため、本 track での partial 変更は blast radius を増やすだけで価値がない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)] [tasks: T001]
- [CN-04] pre-merge ADR (`knowledge/adr/2026-04-27-0554-doc-reorganization.md`) を本 track の最初の commit で一緒にコミットする。ADR ファイルはまだ working tree のみに存在する pre-merge 状態であり、`pre-track-adr-authoring` convention の終端処理ルールに従い、本 track の実装 commit と同一 commit に含める [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] `knowledge/WORKFLOW.md` がワークツリーに存在しない。`git status` または `ls` で確認できる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [ ] [AC-02] `knowledge/architecture.md` がワークツリーに存在しない。`git status` または `ls` で確認できる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [ ] [AC-03] worktree に `repomix-output.*` ファイルが存在しない。`.gitignore` 済みであるためコミット対象ではないが、clean な worktree が確認できる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [ ] [AC-04] 削除したファイルへの参照が derived 文書 (Tier 1/2 の非 Tier 0 ファイル) に残っていない。具体的には `knowledge/WORKFLOW.md` および `knowledge/architecture.md` へのリンクが Tier 1/2 ファイル中に残存しない (grep 等で確認可能)。Tier 0 SoT ファイル (`.claude/rules/` / `.claude/commands/` 等) に残る参照は CN-02 の制約により本 track では修正しない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Consequences] [tasks: T001]
- [ ] [AC-05] `cargo make ci` が pass する。Rust コードへの変更はないため fmt-check / clippy / test / deny / check-layers はそのまま通過する。削除による verify-* の変化がないことも確認する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Neutral] [tasks: T002]
- [ ] [AC-06] `knowledge/adr/2026-04-27-0554-doc-reorganization.md` がリポジトリにコミット済みである (本 track の最初のコミットに含まれている) [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/adr.md#Lifecycle: pre-merge draft vs post-merge record

## Signal Summary

### Stage 1: Spec Signals
🔵 23  🟡 0  🔴 0

