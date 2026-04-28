<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 30, yellow: 0, red: 0 }
---

# external_guides 撤去と関連 Python helper の連鎖削除

## Goal

- [GO-01] agent-router 削除後に主要 caller を失った external guide registry 機能 (`scripts/external_guides.py` およびその直接依存 Python helper) を撤去し、workspace の依存連鎖を clean state に戻す。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除]
- [GO-02] Roadmap ADR (`2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md`) が Phase 3 として計画していた「external_guides の Rust 化」を「機能撤去」へ転換し、Phase 3 の方向性変更を両 ADR の narrative に反映する。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D2: Roadmap ADR Phase 3 の supersede]

## Scope

### In Scope
- [IN-01] 機能本体: `scripts/external_guides.py` と `scripts/test_external_guides.py` を削除する。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T001]
- [IN-02] registry SSoT: `knowledge/external/POLICY.md`、`knowledge/external/guides.json`、`knowledge/external/` ディレクトリ自体を削除する。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T001]
- [IN-03] 連鎖削除 Python helper: `scripts/atomic_write.py` と `scripts/test_atomic_write.py` を削除する (`external_guides.py` が現存する唯一の Python caller)。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T001]
- [IN-04] 連鎖削除 Python helper: `scripts/track_resolution.py` の `latest_legacy_track_dir()` 関数を削除する。`track_resolution.py` / `track_schema.py` ファイル全体の扱いは impl 時に再評価し、test-only に成り下がる場合は同 track 内で完全削除する。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T002]
- [IN-05] Makefile.toml から guides 系タスクを削除する: `[tasks.guides-list]` / `[tasks.guides-fetch]` / `[tasks.guides-usage]` / `[tasks.guides-setup]` / `[tasks.guides-clean]` / `[tasks.guides-add]` (`:27-55`) / `[tasks.guides-selftest]` / `[tasks.guides-selftest-local]` (`:97-107`)。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T002]
- [IN-06] Makefile.toml の `[tasks.scripts-selftest-local]` args (`:109-123`) から `scripts/test_atomic_write.py` および `scripts/test_external_guides.py` を除去する。`scripts/test_track_resolution.py` は `track_resolution.py` 全体削除時に除去する。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T002]
- [IN-07] slash command / skill 削除: `.claude/commands/guide/add.md` を削除する (`.claude/skills/` 側に対応定義はなく、追加削除対象なし)。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T003]
- [IN-08] doc 参照の除去: `CLAUDE.md` の `knowledge/external/POLICY.md` / `knowledge/external/guides.json` 参照行、`.claude/rules/09-maintainer-checklist.md` の `scripts/external_guides.py` 参照、および grep で発見される他の全参照箇所を削除する。具体的な発見候補: `LOCAL_DEVELOPMENT.md`、`DEVELOPER_AI_WORKFLOW.md`、`.claude/settings.json`、`.claude/commands/track/catchup.md`。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T003]
- [IN-09] Roadmap ADR の back-reference 追記: `knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` の Phase 3 セクションに、本 ADR (`2026-04-28-1258-remove-external-guides.md`) によって Phase 3 の方向性が「Rust 化」から「機能撤去」へ転換された旨の back-reference note を追記する (adr-editor 経由で実施)。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D2: Roadmap ADR Phase 3 の supersede] [tasks: T004]

### Out of Scope
- [OS-01] Rust 側 `infrastructure::track::atomic_write_file` および `bin/sotp file write-atomic` CLI は別物であり、本撤去対象外。これらは継続利用される。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除]
- [OS-02] external guide 機能の Rust 化 (HTTP fetch / `sotp guide` subcommand 新規実装など): Roadmap ADR Phase 3 の元案として評価・却下済み。本 track では実施しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#Rejected Alternatives]
- [OS-03] Roadmap ADR (`2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md`) の YAML front-matter 変更 (`status: superseded` / `superseded_by` 追記など): post-merge ADR に許容される編集範囲を超えるため実施しない。front-matter は変更しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D2: Roadmap ADR Phase 3 の supersede]
- [OS-04] `.cache/external-guides/` ディレクトリの自動削除: 本撤去により orphan 状態になるが、利用者が手動削除する Negative consequence として許容される。CI / cargo make からは参照されないため、自動削除は本 track のスコープ外。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#Negative]
- [OS-05] Roadmap ADR Phase 1/2 (Phase 1 は実施済み / Phase 2 は `atomic_write.py` + `architecture_rules.py` 部分移行) および Phase 3 の残存スコープ (`scripts/convention_docs.py` / `scripts/track_schema.py` の Rust 化): 本 track のスコープ外。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除]
- [OS-06] scripts/ ディレクトリ自体の完全削除および tools コンテナからの python3 / pytest / ruff 除去: `scripts/convention_docs.py` / `scripts/track_schema.py` / `scripts/track_resolution.py` (残存部分) 等への依存が続くため、本 track では実施しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#Neutral]

## Constraints
- [CN-01] Roadmap ADR の back-reference 追記は adr-editor 経由で実施する。main orchestrator が `knowledge/adr/*.md` を直接編集することは pre-track-adr-authoring.md の 1 ファイル 1 writer 原則に違反するため禁止。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D2: Roadmap ADR Phase 3 の supersede] [conv: knowledge/conventions/pre-track-adr-authoring.md#main による直接編集の禁止] [tasks: T004]
- [CN-02] Roadmap ADR の YAML front-matter は変更しない。同 ADR の `decisions[]` は single grandfathered entry で構成されており、Phase 3 専用独立 entry が存在しないため、front-matter の semantic 変更 (`status: superseded` / `superseded_by` 追記) は post-merge ADR への許容範囲外となる。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D2: Roadmap ADR Phase 3 の supersede] [conv: knowledge/conventions/adr.md#Lifecycle: pre-merge draft vs post-merge record] [tasks: T004]
- [CN-03] 削除作業の完了後、`cargo make ci` が pass すること。削除漏れのある参照がビルドエラー / CI ゲートを通じて検出されることを確認する。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#Positive] [tasks: T005]

## Acceptance Criteria
- [ ] [AC-01] `scripts/external_guides.py`、`scripts/test_external_guides.py` がファイルシステムに存在しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T001]
- [ ] [AC-02] `knowledge/external/` ディレクトリ (および配下の `POLICY.md`、`guides.json`) がファイルシステムに存在しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T001]
- [ ] [AC-03] `scripts/atomic_write.py`、`scripts/test_atomic_write.py` がファイルシステムに存在しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T001]
- [ ] [AC-04] `scripts/track_resolution.py` 内に `latest_legacy_track_dir` 関数が存在しない。または `scripts/track_resolution.py` 全体がファイルシステムに存在しない (impl 時の再評価結果に依存)。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T002]
- [ ] [AC-05] `Makefile.toml` に `guides-list` / `guides-fetch` / `guides-usage` / `guides-setup` / `guides-clean` / `guides-add` / `guides-selftest` / `guides-selftest-local` のタスク定義が存在しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T002]
- [ ] [AC-06] `Makefile.toml` の `scripts-selftest-local` タスクの args から `scripts/test_atomic_write.py` および `scripts/test_external_guides.py` が除去されている。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T002]
- [ ] [AC-07] `.claude/commands/guide/add.md` がファイルシステムに存在しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T003]
- [ ] [AC-08] `CLAUDE.md`、`.claude/rules/09-maintainer-checklist.md`、`DEVELOPER_AI_WORKFLOW.md`、`LOCAL_DEVELOPMENT.md`、`.claude/settings.json`、`.claude/commands/track/catchup.md` 等に `knowledge/external/POLICY.md`、`knowledge/external/guides.json`、`scripts/external_guides.py`、`guides-fetch` / `guides-list` 等の削除済み成果物への参照が残存しない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D1: external_guides 機能の撤去と関連 Python helper の連鎖削除] [tasks: T003]
- [ ] [AC-09] `knowledge/adr/2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md` の Phase 3 セクションに、本 ADR (`2026-04-28-1258-remove-external-guides.md`) による方向性転換を示す back-reference note が追記されている。Roadmap ADR の YAML front-matter は変更されていない。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#D2: Roadmap ADR Phase 3 の supersede] [tasks: T004]
- [ ] [AC-10] `cargo make ci` が pass する。Rust ソースコードへの変更はないため fmt-check / clippy / nextest / deny / check-layers はそのまま通過する。`scripts/` 変更に起因する `cargo make scripts-selftest` / `cargo make hooks-selftest` / `sotp verify *` への影響がないことも確認する。 [adr: knowledge/adr/2026-04-28-1258-remove-external-guides.md#Positive] [tasks: T005]

## Related Conventions (Required Reading)
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/adr.md#Lifecycle: pre-merge draft vs post-merge record
- knowledge/conventions/source-attribution.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code

## Signal Summary

### Stage 1: Spec Signals
🔵 30  🟡 0  🔴 0

