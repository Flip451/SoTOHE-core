<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 26, yellow: 2, red: 0 }
---

# RV2-17: Python hooks 全廃止 (Phase 1)

## Goal

.claude/hooks/ 配下に残存する Python hook (advisory 9 + library 2 + tests 5 = 16 ファイル) を全削除し、Claude Code hook システムから Python 依存を排除する。
Python hook が提供していた advisory 機能は既に rules / skills / commands に存在するため、機能損失をほぼ伴わずに削除できる。
後続 ADR 2026-04-09-2235 (agent-profiles redesign / Phase 2) と 2026-04-09-2047 (planning review separation / Phase 3) の prerequisite として位置付け、Python 側の参照更新を不要化することで後続トラックのスコープを縮小する。

## Scope

### In Scope
- .claude/hooks/*.py 全 16 ファイルの削除と .claude/hooks/ ディレクトリ自体の削除 [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/1] [tasks: T02]
- .claude/settings.json から Python hook 起動エントリ (PreToolUse 2 + PostToolUse 7 = 9 entries) と Bash(cargo make hooks-selftest) permission の削除 [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/2] [tasks: T03]
- libs/infrastructure/src/verify/orchestra.rs の EXPECTED_HOOK_PATHS から削除対象 9 hook entries の除去と関連 unit test の更新 [source: libs/infrastructure/src/verify/orchestra.rs §EXPECTED_HOOK_PATHS] [tasks: T01]
- Makefile.toml から hooks-selftest / hooks-selftest-local task の削除 [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/4] [tasks: T04]
- Makefile.toml の python-lint-local / python-lint ruff 対象から .claude/hooks/ を除外し scripts/ のみに絞り込む (task 自体は維持) [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/4, inference — scripts/ Python が残存するため task 完全削除は不可] [tasks: T05]
- ドキュメント (CLAUDE.md, .claude/rules/09-maintainer-checklist.md, DEVELOPER_AI_WORKFLOW.md, knowledge/WORKFLOW.md, LOCAL_DEVELOPMENT.md, START_HERE_HUMAN.md, knowledge/DESIGN.md, track/workflow.md) からの Python hook 言及削除/更新 [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/6] [tasks: T06]
- Phase 1-3 設計 ADR 3 本 (2026-04-09-2047, 2026-04-09-2235, 2026-04-09-2323) と knowledge/strategy/TODO.md 変更を本トラックの計画 commit に取り込む [source: feedback — handoff (tmp/track-commit/handoff.md) で main 直接コミット不可のため Phase 1 トラックでまとめてコミットする方針] [tasks: T07]
- cargo make ci 全チェック通過 (verify-orchestra で hook path 整合性確認) [source: convention — .claude/rules/07-dev-environment.md] [tasks: T08]

### Out of Scope
- Dockerfile からの python3 / python3-yaml / python3-pytest 削除 [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/5, inference — scripts/ 配下に Python 25 ファイルが残存するため Dockerfile python3 削除は不可]
- docker compose 設定から Python 関連ボリュームマウント / 環境変数の整理 (ADR §5 言及項目) — compose.yml / compose.dev.yml に Python 関連エントリが存在しないため変更不要 [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/5, inference — compose.yml / compose.dev.yml を実際に確認した結果、python 関連エントリなし]
- requirements-python.txt の削除 [source: inference — scripts/ Python 用の ruff と PyYAML の SSoT として scripts/ Python 残存中は維持必要 (pytest は Dockerfile の python3-pytest apt パッケージ由来であり requirements-python.txt には含まれない)]
- scripts/ 配下の Python ファイル (architecture_rules.py, atomic_write.py, convention_docs.py, external_guides.py, track_*.py, test_*.py 等 25 ファイル) の削除 [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/5 (段階的除去方針)]
- .claude/rules/02-codex-delegation.md の debugger capability 言及削除 [source: knowledge/adr/2026-04-09-2235-agent-profiles-redesign.md §Decision/2 (debugger capability 廃止は Phase 2 のスコープ)]
- agent-profiles redesign の実装 (.harness/config/ への移行、capability 中心スキーマ、resolve_execution API) [source: knowledge/adr/2026-04-09-2235-agent-profiles-redesign.md (Phase 2 トラックで実装)]
- planning review phase separation の実装 (sotp review plan / sotp commit plan / plan-review.json / planning-artifacts.json) [source: knowledge/adr/2026-04-09-2047-planning-review-phase-separation.md (Phase 3 トラックで実装)]

## Constraints
- Rust hook (sotp hook dispatch skill-compliance / block-direct-git-ops / block-test-file-deletion) は維持する — 削除対象は Python hook のみ [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Context] [tasks: T01, T02, T03]
- 後方互換性は提供しない (legacy ファイル / graceful skip 機構を残さず、git revert のみで rollback) [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/7] [tasks: T02, T03]
- Hexagonal Architecture を遵守する (orchestra.rs は infrastructure 層、domain への侵食禁止) [source: convention — knowledge/conventions/hexagonal-architecture.md] [tasks: T01]
- TDD ワークフロー遵守 (Red → Green → Refactor) — orchestra.rs のテスト更新は先にテストを赤くしてから実装変更 [source: convention — .claude/rules/05-testing.md] [tasks: T01]
- 実装フェーズの commit は計画 commit (T07 含む) と分離する — atomic 境界を明確にする [source: convention — .claude/rules/10-guardrails.md (Small task commits)] [tasks: T07, T01, T02, T03, T04, T05, T06]

## Acceptance Criteria
- [ ] .claude/hooks/ ディレクトリが存在しない (rm -rf 後の状態確認) [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/1] [tasks: T02]
- [ ] .claude/settings.json の hooks セクションに python3 起動コマンドが 1 つも存在しない [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/2] [tasks: T03]
- [ ] Rust hook (skill-compliance / block-direct-git-ops / block-test-file-deletion) は settings.json に維持されている [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/2] [tasks: T03]
- [ ] .claude/settings.json の permissions.allow から Bash(cargo make hooks-selftest) エントリが削除されている [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/2] [tasks: T03]
- [ ] T06 対象の全ドキュメント (CLAUDE.md, .claude/rules/09-maintainer-checklist.md, DEVELOPER_AI_WORKFLOW.md, knowledge/WORKFLOW.md, LOCAL_DEVELOPMENT.md, START_HERE_HUMAN.md, knowledge/DESIGN.md, track/workflow.md) から Python hook の言及が削除または更新されている [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/6] [tasks: T06]
- [ ] libs/infrastructure/src/verify/orchestra.rs の EXPECTED_HOOK_PATHS に削除した 9 hook が含まれない [source: libs/infrastructure/src/verify/orchestra.rs §EXPECTED_HOOK_PATHS] [tasks: T01]
- [ ] cargo make hooks-selftest コマンドが存在しない (Makefile.toml に task が定義されていない) [source: knowledge/adr/2026-04-09-2323-python-hooks-removal.md §Decision/4] [tasks: T04]
- [ ] cargo make python-lint が scripts/ のみを対象に成功する (.claude/hooks/ への参照なし) [source: inference — scripts/ Python は本トラック scope 外のため task は維持] [tasks: T05]
- [ ] knowledge/adr/2026-04-09-2047-*.md, 2026-04-09-2235-*.md, 2026-04-09-2323-*.md の 3 ADR がトラックの計画 commit に含まれる [source: feedback — handoff (tmp/track-commit/handoff.md)] [tasks: T07]
- [ ] knowledge/strategy/TODO.md の RV2-16 / RV2-17 エントリ更新がトラックの計画 commit に含まれる [source: feedback — handoff (tmp/track-commit/handoff.md)] [tasks: T07]
- [ ] cargo make ci が全チェック通過する (fmt-check / clippy / test / deny / check-layers / verify-arch-docs / verify-orchestra) [source: convention — .claude/rules/07-dev-environment.md] [tasks: T08]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 26  🟡 2  🔴 0

