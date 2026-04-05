# Implementation Delegation Architecture Guard

## Purpose

Sub-agent（実装者）への委任時にアーキテクチャ設計逸脱を構造的に防止する。
3 層の防御（委任時注入 → 実装後検証 → レビュー時チェック）で設計 → 実装の一貫性を保証する。

## Background

review-system-v2 track で発生した設計逸脱:
- ADR が `CodexReviewer` を infrastructure 層に配置すると明記
- 実装者は `NullReviewer` で bypass し、CLI 層に全ロジックを直接実装
- オーケストレーター・レビューワーともに逸脱を検知できなかった
- 結果: 7+ レビューラウンドのループ、根本原因は設計不適合

## Layer 1: Delegation Injection (委任時)

Sub-agent に実装を委任する際、以下を**必須**セクションとしてプロンプトに含める:

### `## Architecture Constraints (from ADR/plan)`

ADR または plan.md の Canonical Blocks から、以下を抽出して指示に含める:

| 項目 | 例 |
|------|-----|
| 新規 trait / struct の配置層 | `CodexReviewer` → infrastructure |
| trait impl の配置層 | `impl Reviewer for CodexReviewer` → infrastructure |
| 呼び出しフロー | CLI → `ReviewCycle::review()` → `CodexReviewer::review()` |
| CLI の責務制限 | 「CLI は composition root + 結果表示のみ」 |

### テンプレート

```markdown
## Architecture Constraints (from ADR)

以下の型配置は ADR で確定済み。逸脱は許可されない:

| 型/trait | 層 | 根拠 |
|----------|-----|------|
| {Type} | {layer} | ADR §{section} |

CLI 層の責務:
- args パース
- adapter 構築 (composition root)
- usecase 呼び出し
- 結果表示 + ExitCode

CLI 層で usecase ロジック（hash 計算、scope 判定、verdict 変換等）を
再実装してはならない。
```

## Layer 2: Post-Implementation Verification (実装後)

実装完了後、**レビュー起動前に**以下を検証する:

### 2a. 新規型の層チェック

ADR で指定された型が正しい層に存在するか確認:

```
Grep: "pub struct {Type}" → 配置パスが ADR 指定の層か
Grep: "impl {Trait} for {Type}" → 配置パスが infrastructure か
```

### 2b. CLI 肥大化チェック

CLI 層に以下が増えていないか確認:
- usecase port trait の re-implementation
- hash 計算、scope 判定などの domain/usecase ロジック
- `NullReviewer` や `NullXxx` による usecase bypass（status/check-approved 用途を除く）

### 2c. `cargo make check-layers` 実行

レイヤー依存関係の CI チェックを実行。

## Layer 3: Review Briefing Architecture Section (レビュー時)

レビュー briefing に以下を**必須**セクションとして含める:

### `## Architecture Verification Checklist`

```markdown
## Architecture Verification Checklist

- ADR で指定された型が正しい層に配置されているか
- CLI が composition root パターンに従っているか（usecase 呼び出しのみ）
- usecase ロジックが CLI に漏れていないか
- NullXxx による usecase bypass がないか（status/check-approved 用途を除く）
```

## Enforcement

| 検証手段 | タイミング | 自動化 |
|----------|-----------|--------|
| プロンプトテンプレート | 委任時 | オーケストレーター手動 |
| Grep 型配置チェック | 実装後 | オーケストレーター手動 |
| `cargo make check-layers` | 実装後 + CI | 自動 |
| Review briefing template | レビュー時 | オーケストレーター手動 |

TODO: `/track:implement` と `/track:review` skill に Architecture Constraints / Verification Checklist セクションを自動注入する仕組みを追加（RV2-05）。

## Anti-patterns

| パターン | 問題 | 対処 |
|----------|------|------|
| `NullXxx` + CLI 直接実装 | usecase bypass | infra に adapter を実装 |
| CLI で hash 計算 | usecase ロジック漏洩 | `ReviewCycle` に委任 |
| CLI で scope 判定 | domain ロジック漏洩 | `ReviewCycle::get_review_targets()` を使用 |
| CLI で verdict 変換 | usecase ロジック漏洩 | adapter 内で変換 |

## Related Documents

- `hexagonal-architecture.md`: Layer boundaries and port placement
- `architecture-rules.json`: Machine-readable layer dependencies
- `.claude/rules/08-orchestration.md`: Delegation rules
- `.claude/rules/04-coding-principles.md`: Trait-Based Abstraction
