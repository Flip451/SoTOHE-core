# Architecture Decision Records (ADR)

このディレクトリは設計判断の記録を管理する。

## 運用ルール

- **フォーマット**: Nygard 式 + Rejected Alternatives + Reassess When
- **言語**: 日本語
- **採番**: `YYYY-MM-DD-HHMM-slug.md`（例: `2026-03-11-1430-track-status-derived.md`）
- **Status**: `Accepted` / `Superseded` / `Deprecated`
- **Superseded の場合**: 新 ADR を作成し、旧 ADR の Status を `Superseded by YYYY-MM-DD-HHMM-slug.md` に変更

## ADR テンプレート

```markdown
# {タイトル}

## Status

Accepted / Superseded / Deprecated

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

### 信号機アーキテクチャ

| ADR | Status | Date |
|-----|--------|------|
| [2 段階信号機アーキテクチャ](2026-03-23-2120-two-stage-signal-architecture.md) | Accepted | 2026-03-23 |
| [spec ↔ code 整合性チェックは Phase 3 に送る](2026-03-23-2130-spec-code-consistency-deferred.md) | Accepted | 2026-03-23 |
| [Coverage は信号機ではなく CI ゲートとする](2026-03-24-0900-coverage-not-a-signal.md) | Accepted | 2026-03-24 |
| [Stage 2 信号機にコンパイル通過を条件に入れない](2026-03-24-0910-stage2-no-compile-check.md) | Accepted | 2026-03-24 |
| [3-12 spec ↔ code 整合性チェック — TypeGraph + 既知課題の解決](2026-04-08-0045-spec-code-consistency-check-design.md) | Accepted | 2026-04-08 |
| [TDDD: 逆方向チェック信号機統合 + designer capability](2026-04-08-1800-reverse-signal-integration.md) | Accepted | 2026-04-08 |

### ADR 運用

| ADR | Status | Date |
|-----|--------|------|
| [ADR 自動導出: SSoT → ADR 候補検出の設計](2026-03-24-0930-adr-auto-derivation-design.md) | Accepted (設計のみ) | 2026-03-24 |

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

### オーケストレーション・エージェント管理

| ADR | Status | Date |
|-----|--------|------|
| [agent-router フックを skill 遵守フックに置換](2026-04-08-1200-remove-agent-router-hook.md) | Accepted | 2026-04-08 |
