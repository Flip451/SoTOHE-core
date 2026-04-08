<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "1.0.0"
signals: { blue: 17, yellow: 0, red: 0 }
---

# WF-67: agent-router 廃止 + skill 遵守フック導入（Rust）

## Goal

agent-router.py（intent 検出 + ルーティングヒント注入）を廃止し、/track:* コマンド検出時に SKILL.md フェーズ遵守を強制する Rust フックに置換する。
ルーティング判断は rules + agent-profiles.json に委譲し、フックの責務を skill 遵守 + external guide injection に限定する。

## Scope

### In Scope
- agent-router.py + test_agent_router.py の削除と全参照箇所のクリーンアップ [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md] [tasks: T01, T02, T03, T04]
- sotp hook dispatch skill-compliance サブコマンドの新設（Rust） [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md] [tasks: T05, T06, T09]
- /track:* コマンドパターン検出と SKILL.md フェーズ要件リマインド生成 [source: feedback — /track:plan 実行時に planner capability が省略された事故] [tasks: T05, T06]
- external guide injection の Rust 移植（guides.json keyword match） [source: knowledge/research/2026-04-08-0033-planner-agent-router-removal.md] [tasks: T07, T08]

### Out of Scope
- intent 検出・ルーティングヒント注入（rules + agent-profiles.json に委譲） [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md]
- _agent_profiles.py の変更（他フック・skill から引き続き利用） [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md]
- agent-profiles.json / 08-orchestration.md の変更 [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md]

## Constraints
- 新ロジックは Rust で実装し、sotp hook dispatch サブコマンドとして統合する [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md] [tasks: T09]
- agent-router.py と test_agent_router.py は同時に削除する（pytest collection 失敗防止） [source: knowledge/research/2026-04-08-0033-planner-agent-router-removal.md] [tasks: T02]
- Hexagonal Architecture に従い、domain/infrastructure/CLI の層分離を遵守する [source: convention — knowledge/conventions/hexagonal-architecture.md] [tasks: T05, T06, T07, T08, T09]
- TDD ワークフロー遵守（Red → Green → Refactor） [source: convention — .claude/rules/05-testing.md] [tasks: T10]

## Acceptance Criteria
- [ ] agent-router.py / test_agent_router.py が削除されている [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md] [tasks: T02]
- [ ] settings.json の UserPromptSubmit に sotp hook dispatch skill-compliance が登録されている [source: knowledge/adr/2026-04-08-1200-remove-agent-router-hook.md] [tasks: T01, T11]
- [ ] /track:plan 等のコマンド送信時に SKILL.md フェーズ要件のリマインドが additionalContext として注入される [source: feedback — /track:plan 実行時に planner capability が省略された事故] [tasks: T05, T06, T09]
- [ ] /track:* コマンド + guides.json にマッチするガイドがある場合、ガイドサマリーが additionalContext に含まれる [source: knowledge/research/2026-04-08-0033-planner-agent-router-removal.md] [tasks: T07, T08]
- [ ] cargo make ci が全チェック通過する [source: convention — .claude/rules/07-dev-environment.md] [tasks: T12]
- [ ] cargo make hooks-selftest が通過する（test_agent_router.py 削除後も他テストに影響なし） [source: knowledge/research/2026-04-08-0033-planner-agent-router-removal.md] [tasks: T02, T12]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 17  🟡 0  🔴 0

