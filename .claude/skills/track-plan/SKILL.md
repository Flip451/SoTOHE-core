---
name: track-plan
description: |
  Start a new Rust feature with multi-agent collaboration.
  Phase 1: Codebase understanding via the active researcher capability.
  Phase 2: Parallel research & design (Agent Teams: researcher + planner).
  Phase 3: Plan synthesis, user approval, and track update.
  Phase 4: Create track artifacts (metadata.json, plan.md, spec.md, verification.md, registry.md).
metadata:
  short-description: Rust feature kickoff with Agent Teams (Plan phase)
---

# `/track:plan` Backing Skill

**specialist capability と Agent Teams を活用した Rust 機能計画スキル。**

実際にどの provider を使うかは `.claude/agent-profiles.json` を正本とする。
以下の CLI 例は既定 profile を前提にしており、`researcher` / `multimodal_reader` = Gemini、`planner` = Codex を想定している。

## Overview

```
/track:plan <feature>      ← このスキル（計画 + トラック作成）
    または
/track:plan-only <feature> ← plan/<id> ブランチで計画を作成し PR 経由で main に合流
    ↓ ユーザー承認後にトラック成果物を作成
/track:activate <track-id> ← planning-only track を materialize
    ↓
/track:full-cycle <task>   ← 自動実装・レビュー
    または
/track:implement           ← 並列実装
    ↓
/track:review              ← 並列レビュー
```

Track Workflow との連携：

```
計画承認後：
  → track/items/<id>/ ディレクトリを作成
  → metadata.json (SSoT) を作成
  → plan.md を metadata.json から render_plan() で生成
  → spec.md, verification.md を初期化
  → track/registry.md を更新
```

---

## Phase 1: UNDERSTAND（researcher capability + Claude Lead）

### Step 1: Version Baseline Research with active `researcher` capability (必須)

プロジェクト開始時に最新版を調査し、基準を確定する：

```bash
gemini -p "Research latest stable versions as of today:
- Rust stable toolchain (version + release date)
- cargo-make, cargo-nextest, cargo-deny, cargo-machete
- crates used in Cargo.toml (name, current constraint, latest stable)
Return markdown table with: item | current | latest | recommendation.
Include source links." 2>/dev/null
```

結果は `.claude/docs/research/version-baseline-YYYY-MM-DD.md` に保存し、以下へ反映する：
- `Cargo.toml` (`rust-version`)
- `Dockerfile` (`RUST_VERSION`, ツールバージョンARG)
- `track/tech-stack.md` の MSRV 例と変更履歴

### Step 2: Analyze Rust Codebase with active `researcher` capability

```
Task tool:
  subagent_type: "general-purpose"
  prompt: |
    Run the following Gemini CLI command and return the output:

    gemini -p "Analyze this Rust codebase:
    - Cargo workspace structure and crate organization
    - Key traits (domain ports) and their implementations (adapters)
    - Architecture patterns (Hexagonal, CQRS, DDD if present)
    - Async patterns: Tokio usage, async-trait patterns
    - Error handling strategy (thiserror, anyhow, custom types)
    - Test structure: unit tests, integration tests, mocks
    - Existing conventions for naming, module layout
    - Key dependencies in Cargo.toml
    - track/tech-stack.md content if present" 2>/dev/null

    Also check if track/items/ has a spec.md for this feature.
    Save analysis to .claude/docs/research/{feature}-codebase.md
    Return concise summary (7-10 key findings).
```

### Step 3: Read Feature Spec (if exists)

track トラックの仕様書を確認する：

```bash
ls track/items/
# spec.md が存在する場合に読み込む
```

### Step 4: Requirements Gathering

ユーザーに確認（日本語）：
1. この機能の目的と成功基準は？
2. どのクレートを使う予定か？（または調査が必要か？）
3. ドメイン層の変更が必要か、インフラ層のみか？
4. テスト戦略（ユニット・統合・E2E）は？
5. パフォーマンス要件は？

### Step 5: Interactive Tech Stack Setup (必須)

`track/tech-stack.md` を開き、以下をユーザーと対話して更新する：
1. Rust Edition（2024固定）と MSRV
2. Web フレームワーク
3. DB ライブラリ / DB / マイグレーション
4. メトリクス基盤
5. 認証方式
6. 設定管理方式

`TODO:` が 1つでも残っている場合、Phase 2 に進まない。

---

## Phase 2: RESEARCH & DESIGN（Agent Teams — Parallel）

Claude Code Agent Teams を使って並列実行する：

```
Spawn two teammates:

Teammate 1 — Researcher capability（既定 profile では Gemini CLI でクレートと設計パターンを調査）:
  - Research required crates (latest version, idiomatic usage, async support)
  - Find Rust-specific patterns for the feature
  - Check for similar implementations in the Rust ecosystem
  - Command: gemini -p "Research Rust crates needed for: {feature}" 2>/dev/null
  - Save to .claude/docs/research/{feature}-crates.md

Teammate 2 — Planner / Architect capability（既定 profile では Codex CLI で Rust アーキテクチャを設計）:
  - Design trait definitions (ports) and their implementations (adapters)
  - Plan ownership/lifetime structure
  - Design error types hierarchy
  - Create step-by-step implementation plan considering Rust's type system
  - Resolve `{model}` from `profiles.<active_profile>.provider_model_overrides.codex` first, then `providers.codex.default_model`
  - Command: codex exec --model {model} --sandbox read-only --full-auto "
      Design Rust architecture for: {feature}
      Current codebase patterns: {summary from Phase 1}
      Requirements: {requirements from Phase 1}
      Provide: trait definitions, module structure, TDD implementation plan
    " 2>/dev/null
```

---

## Phase 3: PLAN & APPROVE（Claude Lead）

### Step 1: Synthesize Results

Researcher と Architect の結果を統合する。
`track/tech-stack.md` に `TODO:` が残っている場合はここで停止し、ユーザーに確認する。

**Canonical Block preservation rule:**
When copying `planner` capability output into `plan.md` or `DESIGN.md`, copy every block inside
the `## Canonical Blocks` section of the planner's response verbatim. Only that section qualifies;
fenced blocks in `## Rust Code Example`, `## Analysis`, or other sections are illustrative and
may be summarized.

- `plan.md`: surrounding explanation may be summarized in Japanese; `## Canonical Blocks` content must not be altered
- `DESIGN.md`: preserve planner's English text and `## Canonical Blocks` content as-is
- If direct embedding is awkward, save the full planner output to
  `.claude/docs/research/{capability}-{feature}.md` and reference it

### Step 2: Create TDD Implementation Plan

`project-docs/conventions/README.md` を読み、この機能に関連する convention ドキュメントを特定する。
`plan.md` には `## 関連規約（実装前に必読）` セクションを設け、該当 convention の repo-relative path を通常の箇条書きで列挙する。該当なしの場合は「なし」と明記する。`- [ ]` checkbox 形式は task parser と衝突するため使わない。

Rust TDD を前提としたタスク順序：

1. ドメイン型・エラー型の定義
2. トレイト（Port）の定義
3. ユニットテストの作成（Red）
4. ドメインロジックの実装（Green）
5. リファクタリング（Refactor）
6. インフラ実装（Adapter）
7. 統合テスト
8. `cargo make ci` 全チェック

### Step 3: Present to User（日本語）

```markdown
## プロジェクト計画: {feature}

### コードベース分析（researcher capability; 既定 profile では Gemini 1M context）
{Key findings}

### クレート調査（Researcher）
{Crate recommendations, versions, notes}

### Rust アーキテクチャ設計（planner capability; 既定 profile では Codex）
{Trait definitions, module structure}

### タスクリスト（TDDサイクル）
1. [ ] ドメイン型定義: `{TypeName}`
2. [ ] トレイト定義: `{TraitName}`
3. [ ] テスト作成（Red）: `{test names}`
4. [ ] 実装（Green）: `{impl details}`
5. [ ] リファクタリング + clippy
6. [ ] インフラ実装: `{AdapterName}`
7. [ ] 統合テスト
8. [ ] `cargo make ci` 全チェック

### 次のステップ
- この計画で進めてよろしいですか？
- 承認後に standard lane なら `/track:full-cycle <task>` または `/track:implement`、plan-only lane なら `/track:activate <track-id>` を経由して実装を開始
```

### Step 4: Create Track Artifacts（承認後）

ユーザーが計画を承認したら、以下の成果物を作成する：

1. `track/items/<id>/` ディレクトリを作成（safe slug + timestamp/id で衝突回避）
2. `metadata.json` (SSoT) を作成（schema_version 3, tasks, plan sections）
3. `plan.md` を `metadata.json` から `render_plan()` で生成（直接書き込み禁止）
4. `spec.md` を初期化（feature goal, scope, constraints, acceptance criteria）
   - Scope, Constraints, Acceptance Criteria の各項目に `[source: ...]` タグを付与する（ソース帰属）
   - Source タグの種類（5 種）:
     - `[source: <document> §<section>]` — 明示的な文書参照（例: `[source: PRD §3.2]`, `[source: track/tech-stack.md]`）
     - `[source: feedback — <context>]` — ユーザーフィードバック（例: `[source: feedback — Rust-first policy]`）
     - `[source: convention — <file>]` — プロジェクト規約（例: `[source: convention — .claude/rules/05-testing.md]`）
     - `[source: inference — <理由>]` — 推定・慣行ベース（例: `[source: inference — セキュリティ慣行から推定]`）
     - `[source: discussion]` — チーム・ユーザーとの議論ベース
   - 参照: `project-docs/conventions/source-attribution.md`
5. `verification.md` を初期化（scope verified, manual steps, result, verified_at）
6. `track/registry.md` を更新（active track row, Current Focus, Last updated）
