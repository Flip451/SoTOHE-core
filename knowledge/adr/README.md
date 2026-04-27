# Architecture Decision Records (ADR)

このディレクトリは設計判断の記録を管理する。

## 運用ルール

- **フォーマット**: Nygard 式 + Rejected Alternatives + Reassess When
- **言語**: 日本語
- **採番**: `YYYY-MM-DD-HHMM-slug.md`（例: `2026-03-11-1430-track-status-derived.md`）
- **Status**: `Proposed` / `Accepted` / `Superseded` / `Deprecated`
  - `Proposed`: ADR is authored and under review / pending activation of the associated track
  - `Accepted`: Decision is accepted and implementation may proceed
  - `Superseded`: Replaced by a newer ADR (reference the superseding ADR)
  - `Deprecated`: Decision is withdrawn without replacement
- **Superseded の場合**: 新 ADR を作成し、旧 ADR の Status を `Superseded by YYYY-MM-DD-HHMM-slug.md` に変更

## ADR テンプレート

```markdown
# {タイトル}

## Status

Proposed / Accepted / Superseded / Deprecated

## Context

{なぜこの判断が必要だったか}

## Decision

{何を選んだか}

## Rejected Alternatives

- {選択肢B}: {却下理由}
- {選択肢C}: {却下理由}

## Consequences

- Good: {良い影響}
- Bad: {悪い影響・トレードオフ}

## Reassess When

- {前提が変わる条件}
```

## ADR と Convention の関係

| | ADR | Convention |
|---|---|---|
| 問い | 「なぜこうした？」 | 「これからどうする？」 |
| 時制 | 過去形（あの時点で判断した） | 現在形（今後はこうせよ） |
| 寿命 | 永続（superseded でも残る） | 現行ルールのみ有効 |
| 例 | 「conch-parser を選んだ。理由は...」 | 「shell パースは conch-parser を使え」 |

Convention に `## Decision Reference` セクションを追加し ADR にリンクする。

## 索引

### プロジェクト戦略

| ADR | Status | Date |
|-----|--------|------|
| [Phase 1.5 を good enough 宣言](2026-03-23-2100-phase-1.5-good-enough.md) | Accepted | 2026-03-23 |
| [sotp CLI 外部ツール化は Moat 後に再評価](2026-03-23-2110-sotp-extraction-deferred.md) | Accepted | 2026-03-23 |
| [scripts/ Python ヘルパーの段階的 Rust 移行ロードマップ](2026-04-13-1200-scripts-python-helpers-rust-migration-roadmap.md) | Proposed | 2026-04-13 |

### 信号機アーキテクチャ

| ADR | Status | Date |
|-----|--------|------|
| [2 段階信号機アーキテクチャ](2026-03-23-2120-two-stage-signal-architecture.md) | Accepted | 2026-03-23 |
| [spec ↔ code 整合性チェックは Phase 3 に送る](2026-03-23-2130-spec-code-consistency-deferred.md) | Accepted | 2026-03-23 |
| [Coverage は信号機ではなく CI ゲートとする](2026-03-24-0900-coverage-not-a-signal.md) | Accepted | 2026-03-24 |
| [Stage 2 信号機にコンパイル通過を条件に入れない](2026-03-24-0910-stage2-no-compile-check.md) | Accepted | 2026-03-24 |
| [3-12 spec ↔ code 整合性チェック — TypeGraph + 既知課題の解決](2026-04-08-0045-spec-code-consistency-check-design.md) | Accepted | 2026-04-08 |
| [TDDD: 逆方向チェック信号機統合 + designer capability](2026-04-08-1800-reverse-signal-integration.md) | Accepted | 2026-04-08 |
| [TDDD-02: Baseline-Aware Reverse Signal Detection](2026-04-11-0001-baseline-reverse-signals.md) | Proposed | 2026-04-11 |
| [TDDD-01: Multilayer Extension — 型カタログ多層化 + シグネチャ検証](2026-04-11-0002-tddd-multilayer-extension.md) | Proposed | 2026-04-11 |
| [TDDD-03: 型アクション宣言 — add / modify / delete](2026-04-11-0003-type-action-declarations.md) | Accepted | 2026-04-11 |
| [TDDD 型カタログ Taxonomy 拡張 — アプリケーション層パターンの幅を広げる](2026-04-13-1813-tddd-taxonomy-expansion.md) | Accepted | 2026-04-13 |
| [Finding 型 Taxonomy クリーンアップ — 同名衝突の解消と hexagonal 分離の維持](2026-04-14-0625-finding-taxonomy-cleanup.md) | Accepted | 2026-04-14 |
| [Domain serde 依存除去 — hexagonal 純粋性回復 + infrastructure 層 TDDD partial dogfood](2026-04-14-1531-domain-serde-ripout.md) | Accepted | 2026-04-14 |
| [Catalogue active-track guard + renderer source-file-name + sync_rendered_views multilayer](2026-04-15-1012-catalogue-active-guard-fix.md) | Accepted | 2026-04-15 |
| [TDDD-05: Secondary Adapter variant の追加 — infrastructure 層における hexagonal port 実装の検証](2026-04-15-1636-tddd-05-secondary-adapter.md) | Accepted | 2026-04-15 |
| [TDDD Type Graph View — TypeGraph から mermaid 図をレンダーして型間関係を可視化する](2026-04-16-2200-tddd-type-graph-view.md) | Accepted | 2026-04-16 |
| [TDDD Contract Map — 全層カタログを入力とする統合 mermaid view](2026-04-17-1528-tddd-contract-map.md) | Accepted | 2026-04-17 |
| [型カタログ → 仕様書 signal 評価の有効化 (SoT Chain ②)](2026-04-23-0344-catalogue-spec-signal-activation.md) | — | 2026-04-23 |
| [type-designer Phase 2 reconnaissance step — 設計開始前に baseline + type-graph で既存型インベントリを把握する](2026-04-25-0353-type-designer-reconnaissance-step.md) | — | 2026-04-25 |
| [type-designer reconnaissance のレンダリングオプション既定値 — depth=1+2 + edges=all](2026-04-25-0530-type-designer-recon-options-defaults.md) | — | 2026-04-25 |

### トラック・ワークフロー

| ADR | Status | Date |
|-----|--------|------|
| [計画成果物ワークフローの再構築 — SoT Chain に沿ったフェーズ分離](2026-04-19-1242-plan-artifact-workflow-restructure.md) | — | 2026-04-19 |
| [Phase command 共通構造 + subagent 内部 pipeline 決定](2026-04-22-0829-plan-command-structural-refinements.md) | — | 2026-04-22 |
| [sotp track branch create: main 上の activation commit regression 修正](2026-04-22-1432-branch-create-commit-ordering.md) | — | 2026-04-22 |
| [verification.md を observations.md に改名 — 役割を手動観測ログに限定](2026-04-24-2356-verification-md-rename-observations-md.md) | — | 2026-04-24 |
| [verify チェーンを file 存在ベースの phase 責務分離に揃える](2026-04-27-0324-phase-aware-verify-gates.md) | — | 2026-04-27 |

### ADR 運用

| ADR | Status | Date |
|-----|--------|------|
| [ADR 自動導出: SSoT → ADR 候補検出の設計](2026-03-24-0930-adr-auto-derivation-design.md) | Accepted (設計のみ) | 2026-03-24 |

### ドキュメント運用

| ADR | Status | Date |
|-----|--------|------|
| [運用ドキュメント断捨離方針 — SoT 一本化と narrative 重複の解消](2026-04-27-0554-doc-reorganization.md) | — | 2026-04-27 |

### ドメインモデル・型設計 (DESIGN.md 由来)

| ADR | Status | Date |
|-----|--------|------|
| [TrackStatus を tasks から導出](2026-03-11-0000-track-status-derived.md) | Accepted | 2026-03-11 |
| [TaskStatus::Done が CommitHash を所有](2026-03-11-0010-done-owns-commit-hash.md) | Accepted | 2026-03-11 |
| [TaskTransition を明示的 enum コマンドに](2026-03-11-0020-task-transition-enum.md) | Accepted | 2026-03-11 |
| [StatusOverride の自動クリア](2026-03-11-0030-status-override-auto-clear.md) | Accepted | 2026-03-11 |
| [Plan-task 参照整合性を構築時に検証](2026-03-11-0040-plan-task-integrity.md) | Accepted | 2026-03-11 |
| [Fail-closed フック エラーハンドリング](2026-03-11-0050-fail-closed-hooks.md) | Accepted | 2026-03-11 |
| [Shell guard を domain 層に配置 (no trait)](2026-03-11-0060-shell-guard-in-domain.md) | Superseded | 2026-03-11 |
| [INF-20: ShellParser port + ConchShellParser adapter](2026-03-23-1000-shell-parser-port.md) | Accepted | 2026-03-23 |
| [conch-parser for shell AST (vendored, patched)](2026-03-11-0070-conch-parser-selection.md) | Accepted | 2026-03-11 |
| [Guard policy: ban edge-case-producing patterns](2026-03-11-0080-guard-policy-ban-patterns.md) | Accepted | 2026-03-11 |
| [Reviewer model_profiles in agent-profiles.json](2026-03-17-0000-reviewer-model-profiles.md) | Accepted | 2026-03-17 |
| [3-level signals with SignalBasis](2026-03-23-1010-three-level-signals.md) | Accepted | 2026-03-23 |
| [Two-stage signal architecture](2026-03-23-1020-two-stage-signals.md) | Accepted | 2026-03-23 |
| [DiffScope と scope filtering は usecase 層に配置](2026-03-25-0000-diff-scope-in-usecase.md) | Accepted | 2026-03-25 |
| [パス正規化: exact match + fail-closed](2026-03-25-0010-path-normalization-exact-match.md) | Accepted | 2026-03-25 |
| [Review state trust model と metadata.json 自己参照問題](2026-03-24-1200-review-state-trust-model.md) | Superseded | 2026-03-24 |
| [FsTrackStore + review.json: 関心事の分離](2026-03-25-2125-review-json-separation-of-concerns.md) | Superseded | 2026-03-25 |
| [Review Hash スコープ再設計](2026-03-26-0000-review-hash-scope-redesign.md) | Superseded | 2026-03-26 |
| [review.json 分離 + グループ独立レビュー状態](2026-03-29-0947-review-json-per-group-review-state.md) | Superseded | 2026-03-29 |
| [Review System v2: frozen scope 廃止とスコープ独立型レビュー](2026-04-04-1456-review-system-v2-redesign.md) | Accepted | 2026-04-04 |
| [Review System V1 完全撤去 — metadata.json review + V1 review.json codec + escalation + index_tree_hash_normalizing](2026-04-12-1800-reviewstate-v1-decommission.md) | Accepted | 2026-04-12 |
| [review-scope.json に scope 別 briefing 注入機構を追加する — plan-artifacts scope の新設](2026-04-18-1354-review-scope-prompt-injection.md) | Proposed | 2026-04-18 |

### オーケストレーション・エージェント管理

| ADR | Status | Date |
|-----|--------|------|
| [agent-router フックを skill 遵守フックに置換](2026-04-08-1200-remove-agent-router-hook.md) | Accepted | 2026-04-08 |

### テスト・CI ツーリング

| ADR | Status | Date |
|-----|--------|------|
| [`cargo make llvm-cov` を nextest 経路に統一する](2026-04-27-0124-llvm-cov-nextest-harness-alignment.md) | — | 2026-04-27 |
