---
status: draft
version: "1.1"
signals: { blue: 15, yellow: 16, red: 0 }
---

# Spec Signal Evaluation — Stage 1 (TSUMIKI-01)

## Goal

spec.md の各要件に信号機評価（🔵🟡🔴）を導入し、仕様の信頼度を可視化する。
Stage 1 として source attribution tag に基づく要件出所の信頼度評価を実装する。
集計結果は spec.md frontmatter の `signals:` フィールドに格納する。

## Scope

### In Scope

- `ConfidenceSignal` enum（Blue/Yellow/Red）の domain 型定義（Stage 1/2 共通） [source: tmp/TODO-PLAN-2026-03-22.md §Phase 2 2-1]
- `SignalBasis` enum（Document/Feedback/Convention/Discussion/Inference/MissingSource）の domain 型定義（Stage 1 専用） [source: discussion]
- source tag → signal level マッピング純粋関数 [source: convention — project-docs/conventions/source-attribution.md]
- spec.md 本文の Scope / Constraints / Acceptance Criteria セクションから `[source: ...]` タグをパースし信号レベルを評価するエンジン [source: convention — project-docs/conventions/source-attribution.md]
- `sotp track signals` コマンド（spec.md 評価 → spec.md frontmatter `signals:` 更新 → 表示） [source: inference — 垂直スライス原則 SPEC-08]
- `sotp verify spec-signals` 検証ゲート（spec.md frontmatter と実際の source tag 評価の整合性、`red == 0` ポリシー） [source: inference — 垂直スライス原則 SPEC-08]
- `sotp verify spec-states` 最小版（spec.md に `## Domain States` セクションが存在し、テーブルにデータ行が最低 1 行あるかの CI チェック。空テーブル・ヘッダーのみは reject。signal tag パースは Track B） [source: tmp/domain-modeling-guarantee-2026-03-23.md §指示, tmp/domain-modeling-guarantee-2026-03-23.md §懸念1]

### Out of Scope

- `TrackMetadata` への `Option<SignalCounts>` 追加 [source: inference — Stage 2 の workflow aggregate 関心事]
- metadata.json への `domain_state_signals` フィールド追加 [source: inference — Stage 2 で Domain States パース後に追加]
- Domain States テーブルのパース・信号集計 [source: inference — Stage 2 track で実装]
- plan.md rendering への信号サマリー追加 [source: inference — Stage 2 集計が前提]
- per-item の `SignalBasis` 永続化 [source: inference — Phase 3 CC-SDD-01 トレーサビリティと連動]
- `Contradicted` basis の自動検出 [source: inference — Phase 3 SPEC-01 降格ループの前提]
- 信号機降格ループ [source: tmp/TODO-PLAN-2026-03-22.md §Phase 3 3-9]
- 信号機昇格の CI 客観証拠限定 [source: tmp/TODO-PLAN-2026-03-22.md §Phase 3 3-8]

## Constraints

- domain 層は純粋（I/O なし、panic なし）[source: convention — project-docs/conventions/hexagonal-architecture.md]
- 新規ロジックは Rust で実装（Python 禁止）[source: feedback — Rust-first policy]
- TDD 必須（Red → Green → Refactor）[source: convention — .claude/rules/05-testing.md]
- 既存の `SignalCounts` 型を拡張・活用する [source: inference — 既存 domain 型の再利用]
- `ConfidenceSignal` に `#[non_exhaustive]` を付与する（将来の variant 追加を非破壊的変更にするための設計。拡張自体は scope 外） [source: discussion]
- 信号の集計対象は spec.md の Scope / Constraints / Acceptance Criteria セクション内の項目のみ。Goal セクション・コード例・テーブル内のリテラル例は対象外 [source: convention — project-docs/conventions/source-attribution.md §Rules]

## Signal Assignment Rules

| Source Tag Pattern | SignalBasis | ConfidenceSignal |
|---|---|---|
| `[source: <doc> §<section>]` | Document | Blue |
| `[source: <doc>]` (§ なしの文書参照) | Document | Blue |
| `[source: feedback — ...]` | Feedback | Blue |
| `[source: convention — ...]` | Convention | Blue |
| `[source: discussion]` | Discussion | Yellow |
| `[source: inference — ...]` | Inference | Yellow |
| source tag なし | MissingSource | Red |

### Multi-source Tags

`[source: PRD §3.2, discussion]` のようにカンマ区切りで複数 source を持つタグは、
各 source を個別に評価し、**最も高い信頼度**（Blue > Yellow > Red）を採用する。
例: `Document(Blue) + Discussion(Yellow)` → Blue

## Two-Stage Signal Architecture

```
Stage 1 (this track):
  spec.md [source: ...] tags → ConfidenceSignal → spec.md frontmatter signals:
  Gate: red == 0

Stage 2 (follow-up track):
  spec.md ## Domain States → per-state signal → metadata.json domain_state_signals
  Gate: red == 0, depends on Stage 1 passing
```

## Acceptance Criteria

- [ ] `ConfidenceSignal` enum が domain 層に存在し、`#[non_exhaustive]` 付き [source: discussion]
- [ ] `SignalBasis` enum が domain 層に存在し、source tag パターンとの対応が明確 [source: discussion]
- [ ] spec.md の Scope / Constraints / Acceptance Criteria セクションから `[source: ...]` タグを抽出し、Signal Assignment Rules に基づき信号レベルを評価できる [source: convention — project-docs/conventions/source-attribution.md]
- [ ] `sotp track signals` が spec.md を評価し spec.md frontmatter `signals:` を更新する [source: inference — 垂直スライス原則]
- [ ] `sotp verify spec-signals` が frontmatter と実評価の不整合を検出する [source: inference — 垂直スライス原則]
- [ ] `sotp verify spec-signals` が `red > 0` の場合にエラーを返す [source: inference — ゲートポリシー]
- [ ] `sotp verify spec-states` が `## Domain States` セクション未存在時にエラーを返す [source: tmp/domain-modeling-guarantee-2026-03-23.md §指示]
- [ ] `sotp verify spec-states` が `## Domain States` テーブルにデータ行がない場合（空テーブル・ヘッダーのみ）にエラーを返す [source: tmp/domain-modeling-guarantee-2026-03-23.md §懸念1]
- [ ] `cargo make ci` が通る [source: convention — .claude/rules/07-dev-environment.md]
- [ ] 新規コードのテストカバレッジ 80% 以上 [source: convention — .claude/rules/05-testing.md]

## Related Conventions (Required Reading)

- project-docs/conventions/source-attribution.md
- project-docs/conventions/hexagonal-architecture.md
