<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
status: draft
version: "5.0"
signals: { blue: 31, yellow: 0, red: 0 }
---

# TDDD: 逆方向チェック信号機統合 + designer capability + /track:design

## Goal

TDDD (Type-Definition-Driven Development) ワークフローを確立する。
Blue (定義+実装+一致) / Yellow (定義+未実装 WIP) / Red (未定義+実装済み TDDD違反) の 3 値で信号機を統一する。
verify spec-states ゲート: Red なし → pass (途中コミット許容)、merge 時は Yellow もブロック (全 Blue 必須)。
designer capability + /track:design コマンドを新設し、domain-types.json の初回作成と更新を体系化する。

## Scope

### In Scope
- undeclared types/traits を Red の DomainTypeSignal (kind_tag: undeclared_type / undeclared_trait) に変換する関数を domain 層に追加 [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.1] [tasks: T01]
- 定義済みだが未実装の型に Yellow シグナルを返すよう evaluate_domain_type_signals を拡張 [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.3] [tasks: T02]
- domain-type-signals コマンドで逆方向チェックを実行し、undeclared Red + 未実装 Yellow を domain-types.json に保存。不在時はエラー終了し /track:design を促す [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.2] [tasks: T03]
- verify spec-states ゲート: domain-types.json の全シグナル (undeclared Red を含む) を読み取り Red なし → pass (Yellow 許容)。merge 時は全 Blue 必須 (Yellow もブロック)。CI フローは domain-type-signals → verify spec-states の順で実行 [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.4] [tasks: T04]
- agent-profiles.json の全 profile に designer capability を追加 (既定 provider: claude) [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.5] [tasks: T05]
- /track:design コマンドを作成: 対象トラックの plan.md + 既存 domain-types.json を入力に designer capability を呼び出し domain-types.json を生成・更新 [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.5] [tasks: T06]
- /track:plan 完了メッセージ・registry.md・DEVELOPER_AI_WORKFLOW.md・knowledge/WORKFLOW.md に /track:design を次ステップとして案内する導線を追加 [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.6] [tasks: T07]
- ADR 2026-04-08-1800 を最終化し、ADR README 索引のタイトルを実ファイルと一致させる [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md] [tasks: T08]

### Out of Scope
- 未宣言型の自動追加 (auto_add_undeclared) — TDDD に反するため採用しない [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Rejected Alternatives]
- TypeGraph のグラフアルゴリズム (到達可能性、サイクル検出) [source: knowledge/adr/2026-04-08-0045-spec-code-consistency-check-design.md §Reassess When]

## Constraints
- domain 層に I/O を含めない (hexagonal architecture) [source: convention — knowledge/conventions/hexagonal-architecture.md]
- TDD ワークフローに従う (Red → Green → Refactor) [source: convention — .claude/rules/05-testing.md]
- 未宣言型は自動追加せず Red シグナルで報告する — TDDD の原則として型定義は designer が先に書く [source: feedback — TDDD: undeclared types = Red, not auto-add Yellow]
- domain-types.json の初回作成は /track:design の責務。domain-type-signals は既存ファイルの評価のみ行う [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.2]

## Acceptance Criteria
- [ ] undeclared types/traits が kind_tag: undeclared_type / undeclared_trait の Red DomainTypeSignal として返されること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.1] [tasks: T01]
- [ ] 定義済みだが TypeGraph に見つからない型が Yellow シグナルを返すこと [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.3] [tasks: T02]
- [ ] 定義済みかつ実装済みかつ構造一致の型が従来通り Blue シグナルを返すこと (回帰なし) [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.3] [tasks: T02]
- [ ] domain-type-signals コマンドが逆方向チェックを実行し undeclared Red + 未実装 Yellow を domain-types.json に保存すること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.2] [tasks: T03]
- [ ] domain-type-signals コマンドが domain-types.json 不在時にエラー終了し /track:design を促すメッセージを表示すること (ファイルを作成しない) [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.2] [tasks: T03]
- [ ] サマリ出力が blue=N yellow=M red=K (undeclared=U) の形式で domain-types.md がレンダリングされること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.2] [tasks: T03]
- [ ] verify spec-states が Red シグナルで fail を返し、エラーメッセージに /track:design 案内を含むこと [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.4] [tasks: T04]
- [ ] verify spec-states が Yellow のみの場合に pass を返すこと (途中コミット許容) [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.4] [tasks: T04]
- [ ] merge 時 (track 完了判定) に Yellow が残っている場合はブロックされること (全 Blue 必須) [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.4] [tasks: T04]
- [ ] agent-profiles.json の全 profile (default / claude-heavy / codex-heavy) に designer capability が追加されていること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.5] [tasks: T05]
- [ ] /track:design コマンドが対象トラックの plan.md を入力として designer capability を呼び出し domain-types.json を生成できること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.5] [tasks: T06]
- [ ] /track:design コマンドが既存 domain-types.json を読み込み増分更新できること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.5] [tasks: T06]
- [ ] /track:plan 完了時に /track:design が次ステップとして案内されること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.6] [tasks: T07]
- [ ] registry.md (cargo make track-sync-views で自動生成) の Next 列に /track:design が表示されること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.6] [tasks: T07]
- [ ] DEVELOPER_AI_WORKFLOW.md と knowledge/WORKFLOW.md に TDDD フロー (plan → design → implement) が追記されていること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md §Decision.6] [tasks: T07]
- [ ] ADR 2026-04-08-1800 が Accepted で最終化されており、ADR README 索引のタイトルが実ファイルと一致していること [source: knowledge/adr/2026-04-08-1800-reverse-signal-integration.md] [tasks: T08]
- [ ] cargo make ci が通ること [source: convention — .claude/rules/07-dev-environment.md] [tasks: T01, T02, T03, T04, T05, T06, T07, T08]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md
- knowledge/conventions/source-attribution.md

## Signal Summary

### Stage 1: Spec Signals
🔵 31  🟡 0  🔴 0

