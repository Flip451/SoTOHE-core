---
status: draft
version: "1.0"
signals: { blue: 15, yellow: 0, red: 0 }
---

# spec.json SSoT 化 — spec.md を rendered view に降格

## Goal

metadata.json → plan.md と同じ SSoT パターンを spec にも適用する。
spec.json を仕様の Single Source of Truth とし、spec.md を read-only rendered view に降格する。
Markdown パースの edge case を根本解決し、構造化データからの信号評価に移行する。

## Scope

### In Scope
- spec.json スキーマ v1 の定義と domain 型の実装 [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- spec.json codec (serde read/write/validate) [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- render_spec(): spec.json → spec.md レンダリングエンジン [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- sync_rendered_views への spec.md 統合 [source: convention — metadata.json → plan.md SSoT pattern]
- 構造化データからの信号評価 (evaluate_from_spec) [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- Multi-source タグ対応 (JSON 配列) [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- sotp track signals の spec.json 対応 [source: convention — hexagonal-architecture.md]
- 全 spec verifier の spec.json 対応 (legacy fallback 付き) [source: convention — hexagonal-architecture.md]
- /track:plan スキルの spec.json 生成対応 [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]

### Out of Scope
- 旧 track の spec.md → spec.json 自動マイグレーションコマンド [source: inference — 旧 track は archived/done で verifier 対象外]
- Stage 2 Domain States 信号評価 (SPEC-05/2-2) [source: TODO-PLAN-2026-03-22 §Phase2-2]
- metadata.json への TrackMetadata.spec_signals 型化 [source: inference — Stage 2 scope]
- spec.json のリアルタイム編集 UI [source: inference — CLI ベースプロジェクトでは不要]

## Constraints
- domain 層は I/O を含まない (hexagonal purity) [source: convention — hexagonal-architecture.md]
- 旧 track (spec.json なし) は spec.md 直接読み込みで後方互換 [source: convention — backward compatibility]
- spec.json schema_version 1 で開始、将来のスキーマ拡張に備える [source: convention — metadata.json schema versioning pattern]
- Multi-source ポリシー: 各要件の信号 = sources 中の最高信頼度 (Blue > Yellow > Red) [source: discussion]
- sources 空配列 = Red (MissingSource) — 現行のタグなしアイテムと同等 [source: convention — source-attribution.md]
- rendered spec.md のフォーマットは現行 spec.md と互換 [source: inference — 既存ツール・レビュー習慣との連続性]
- metadata.json extra.spec_signals への書き込みは廃止 [source: discussion]

## Domain States

| State | Description |
|-------|-------------|
| SpecDocument | spec.json の aggregate root。title, status, version, goal, scope, constraints, domain_states, acceptance_criteria, additional_sections, related_conventions, signals を保持 |
| SpecRequirement | text + sources (Vec<String>) の value object。Scope/Constraints/Acceptance Criteria の各アイテム |
| DomainStateEntry | name + description の value object。Domain States テーブルの各行 |
| SpecScope | in_scope + out_of_scope (Vec<SpecRequirement>) の value object |
| SpecSection | title + content (Vec<String>) の value object。additional_sections 用 |
| SignalCounts | blue + yellow + red (u32) — 既存型。evaluate_from_spec で生成 |

## Acceptance Criteria
- [ ] spec.json から spec.md を正確にレンダリングできる [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- [ ] 構造化データから信号評価が正しく動作する (Blue/Yellow/Red) [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- [ ] Multi-source 要件で最高信頼度の信号が選択される [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]
- [ ] sources 空配列の要件が Red と評価される [source: convention — source-attribution.md]
- [ ] sotp track signals が spec.json を読み書きする [source: convention — hexagonal-architecture.md]
- [ ] sotp verify spec-signals が spec.json から評価し red==0 ゲートを適用する (red>0 で拒否) [source: convention — hexagonal-architecture.md]
- [ ] sotp verify spec-states が spec.json の domain_states を検証する [source: convention — hexagonal-architecture.md]
- [ ] sotp verify spec-attribution が spec.json の sources を検証する [source: convention — source-attribution.md]
- [ ] sotp verify spec-frontmatter が spec-schema に移行し spec.json を検証する [source: convention — hexagonal-architecture.md]
- [ ] sotp verify latest-track が spec.json の存在をチェックする [source: convention — hexagonal-architecture.md]
- [ ] spec.json なし旧 track で spec-signals, spec-states, spec-attribution, spec-schema, latest-track の各 verifier が fallback 動作する [source: convention — backward compatibility]
- [ ] sync_rendered_views が spec.md を plan.md と並行して生成する [source: convention — metadata.json → plan.md SSoT pattern]
- [ ] cargo make ci が全テスト通過する [source: convention — hexagonal-architecture.md]
- [ ] /track:plan スキルが spec.json を生成する [source: TODO-PLAN-2026-03-22 §Phase2 deferred items]

## Related Conventions (Required Reading)
- project-docs/conventions/hexagonal-architecture.md
- project-docs/conventions/source-attribution.md
