<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 19, yellow: 0, red: 0 }
---

# TRACK_TRACEABILITY.md §5 を track/workflow.md に merge してから削除する

## Goal

- [GO-01] ADR D2 が削除対象と確定した `TRACK_TRACEABILITY.md` のうち、他文書に未記載の運用ルールを持つ §5 (registry.md Update Rules) の内容を `track/workflow.md` に merge し、情報損失なく `TRACK_TRACEABILITY.md` を削除できる状態にする [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [GO-02] §5 merge が完了した後、`TRACK_TRACEABILITY.md` ファイル自体を削除し、`track/workflow.md` が track operational SoT として一本化される状態を達成する。削除は content merge と同一 track で行う。ADR D2 は削除を確定しており、merge 完了後に削除を別 track に持ち越すと中間状態が長期残存するリスクがある [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [GO-03] `TRACK_TRACEABILITY.md` を参照していた derived 文書 (Tier 1/2) のリンクを更新し、削除後に broken link が残らないようにする。Tier 0 SoT ファイルへの残存参照は CN-02 により本 track では変更しない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Consequences]

## Scope

### In Scope
- [IN-01] `TRACK_TRACEABILITY.md §5 (registry.md Update Rules)` のテーブル (trigger → required updates) を `track/workflow.md` に追加する。§5 には `track/workflow.md` 等の他文書に未記載の `registry.md` 自動再生成と derived view 同期の運用ルールが含まれており、これを `track/workflow.md` の適切な位置に merge する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [IN-02] `TRACK_TRACEABILITY.md` ファイル全体を削除する。§5 merge 完了を確認してから削除する 2 段階実装順序を採る。§1/§2/§3/§4/§6 は `track/workflow.md` と重複しており固有情報がなく (ADR D2 の根拠: 内容の 80% が重複)、§7 は廃止済み機能 (`spec-approve` / `approved` 状態) の記述であり、削除で情報損失はない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T002]
- [IN-03] `TRACK_TRACEABILITY.md` を参照している derived 文書 (Tier 1/2) を更新し、リンク切れを解消する。具体的には `track/workflow.md` の「`TRACK_TRACEABILITY.md` を参照する」旨の記述を削除または自己参照に変更する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T002]

### Out of Scope
- [OS-01] `TRACK_TRACEABILITY.md` の §1/§2/§3/§4/§6/§7/§8 は本 track では merge 対象としない。§1/§2/§3/§4/§6/§8 は `track/workflow.md` と重複しており unique 情報がなく (ADR D2 根拠)、§7 は廃止済み機能の記述である。これらのセクションに固有の情報はなく、content merge の対象から除外する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する]
- [OS-02] `knowledge/DESIGN.md` heavy shrink (ADR D3.1)、`README.md` / `START_HERE_HUMAN.md` / `LOCAL_DEVELOPMENT.md` 縮約 (ADR D3.2-4) は本 track のスコープ外。ADR D6 がそれぞれ別 track に分離している [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]
- [OS-03] Tier 0 SoT ファイル (ADR 本文 / `.claude/commands/` / `.claude/rules/` / `.claude/skills/` / `.harness/config/agent-profiles.json` / `architecture-rules.json` / `Makefile.toml` / `knowledge/conventions/` / track artifacts) は本 track では変更しない。CN-02 の制約による [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する]
- [OS-04] Rust ソースコードの変更は本 track のスコープ外。操作対象はドキュメントファイルのみであり、`libs/` / `apps/` / `crates/` 配下には一切変更を加えない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Neutral]
- [OS-05] D5 再発防止運用ルール (Tier 1 size limit の CI gate 化等) は本 track のスコープ外。ADR D6 が `doc-rules-enforcement-...` track (任意) として分離している [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D6: 実装ロードマップは別 track 群で段階化する (本 ADR は方針のみ)]

## Constraints
- [CN-01] §5 の content を `track/workflow.md` に merge してから `TRACK_TRACEABILITY.md` を削除する。この順序を逆にしてはならない。merge 完了前の削除は情報損失になる [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001, T002]
- [CN-02] 既存の Tier 0 SoT ファイル (ADR 本文 / `.claude/commands/` / `.claude/rules/` / `.claude/skills/` / `.harness/config/agent-profiles.json` / `architecture-rules.json` / `Makefile.toml` / `knowledge/conventions/` / track artifacts) は本 track では変更 (modify / delete) しない。変更対象は `TRACK_TRACEABILITY.md` (削除対象 Tier 2 narrative) と `track/workflow.md` (Tier 2 operational narrative、merge 先) および Tier 1/2 ファイルの broken link 修正に限定する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D1: 文書 4-tier 構造を確立し、それぞれの責務と制約を明文化する] [tasks: T001, T002]
- [CN-03] `track/workflow.md` への §5 追加内容は既存の section 構造と一貫したスタイルで記述する。`TRACK_TRACEABILITY.md §5` の表・記述を逐語的にコピーするのではなく、`track/workflow.md` の文体・フォーマットに合わせて統合する。重複するルール (既に `track/workflow.md` に記載済みのもの) は追加しない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D4: SSoT 単一化マッピングを表で定義する] [tasks: T001]
- [CN-04] 削除後に migration shim (削除ファイルと同名の redirect stub / alias ファイル) を作らない。ADR D2 が削除を確定しており、`no-backward-compat` convention に従い旧パスへの compatibility layer は導入しない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] `track/workflow.md` に `registry.md` 更新タイミング (trigger → required updates) の運用ルールが追加されており、`TRACK_TRACEABILITY.md §5` のテーブルが持っていた情報 (どのコマンド実行時に registry.md を更新するか、`/track:plan` / `/track:commit` / `/track:archive` それぞれのトリガー内容) が `track/workflow.md` で参照可能になっている [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T001]
- [ ] [AC-02] `TRACK_TRACEABILITY.md` がワークツリーに存在しない。AC-01 の merge 完了を確認した後に削除する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D2: 即時削除対象 (3 ファイル) を確定する] [tasks: T002]
- [ ] [AC-03] `TRACK_TRACEABILITY.md` への参照が derived 文書 (Tier 1/2 の非 Tier 0 ファイル) に残っていない。具体的には `track/workflow.md` 中の「`TRACK_TRACEABILITY.md` を参照する」旨の記述が削除または自己参照に置き換えられている。Tier 0 SoT ファイル (`.claude/rules/` / `.claude/commands/` 等) に残る参照は CN-02 の制約により本 track では修正しない [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#D4: SSoT 単一化マッピングを表で定義する] [tasks: T002]
- [ ] [AC-04] `cargo make ci` が pass する。Rust コードへの変更はないため fmt-check / clippy / test / deny / check-layers はそのまま通過する。`TRACK_TRACEABILITY.md` 削除による verify-* への影響がないことも確認する [adr: knowledge/adr/2026-04-27-0554-doc-reorganization.md#Neutral] [tasks: T003]

## Related Conventions (Required Reading)
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/adr.md#Lifecycle: pre-merge draft vs post-merge record

## Signal Summary

### Stage 1: Spec Signals
🔵 19  🟡 0  🔴 0

