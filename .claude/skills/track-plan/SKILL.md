---
name: track-plan
description: |
  Start a new Rust feature with multi-agent collaboration.
  Phase 1: Codebase understanding via the active researcher capability.
  Phase 2: Parallel research & design (Agent Teams: researcher + planner).
  Phase 3: Plan synthesis, user approval, and track update.
  Phase 4: Create track artifacts (metadata.json, plan.md, spec.json, spec.md, verification.md, registry.md).
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
  → spec.json (SSoT) を作成 — 構造化仕様データ
  → spec.md を spec.json から render_spec() で生成（read-only view）
  → verification.md を初期化
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

結果は `knowledge/research/version-baseline-YYYY-MM-DD.md` に保存し、以下へ反映する：
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
    Save analysis to knowledge/research/{feature}-codebase.md
    Return concise summary (7-10 key findings).
```

### Step 3: Read Feature Spec + Differential Context Classification (差分ヒアリング)

track トラックの仕様書を確認する：

```bash
ls track/items/
# spec.json / spec.md が存在する場合に読み込む
```

**差分ヒアリング（TSUMIKI-03）**: 既存の `spec.json` が見つかった場合、全項目を再ヒアリングせず、
信号機評価（ConfidenceSignal）と source tags を活用して情報の充足度を分類する。

#### 3a. 既存 spec.json がある場合

`spec.json` を読み込み、`bin/sotp track signals <track-id>` で最新の信号評価を取得する。
コマンドが失敗した場合（IO エラー等）は `signals: null` のまま続行する。
フォールバック: spec.json の各項目の `sources` 配列を直接読み、下記の分類テーブルと同じルールで分類する
（ツール障害と missing source を混同しないため、全項目 Red 扱いはしない）。

各要件項目（scope.in_scope, scope.out_of_scope, constraints, acceptance_criteria）を以下の 4 カテゴリに分類する。
複数の source tag を持つ項目は**最高信頼度のソースで判定**する（`evaluate_requirement_signal` と同じポリシー）。
既存の `domain_states` エントリも確認し、`description` や `transitions_to` がコードベースの現状と乖離していないかをヒアリング対象に含める：

| カテゴリ | 判定基準 | ヒアリング |
|---------|---------|-----------|
| 🔵 確定済み | 最高信頼度の source が document / feedback / convention → Blue signal | 不要（スキップ） |
| 🟡 要確認 | 最高信頼度の source が inference / discussion → Yellow signal | 確認を推奨 |
| 🔴 要議論 | source tag が空 or 根拠なし → Red signal (MissingSource) | 必須 |
| ❌ 欠落 | spec.json に記載されていないが、機能に必要な情報 | 必須 |

❌ 欠落の検出ヒューリスティクス：
- spec.json の `domain_states` に記載されたステートマシン状態が、実際にはより多くの状態遷移を持つ可能性がある場合
  （注: コード分析で発見された全ての型・トレイトが domain_states 候補になるわけではない。
  domain_states はステートマシンエントリのみを対象とし、ports/adapters/value objects は含まない）
- tech-stack.md の決定事項が constraints に未反映
- 関連 convention の要件が acceptance_criteria に未記載

分類結果を内部メモとして保持し、Step 4 のヒアリングに使う。

#### 3b. 既存 spec.json がない場合

新規 track。分類をスキップし、Step 4 の従来フローにフォールバックする。

### Step 4: Requirements Gathering（差分ヒアリング対応）

#### 4a. 差分ヒアリングモード（既存 spec.json あり）

Step 3a の分類結果に基づき、🟡🔴❌ の項目のみをユーザーに質問する：

```markdown
## 差分ヒアリング: {feature}

### 確定済み項目（Blue — 変更なければスキップ）
- {Blue 項目のリスト — source tag 付き}
> 上記は前回の仕様から変更がなければそのまま引き継ぎます。変更がある場合はお知らせください。

### 確認が必要な項目（Yellow — 推定に基づく）
{Yellow 項目ごとに、推定の根拠と確認質問を提示}

### 議論が必要な項目（Red — 根拠なし）
{Red 項目ごとに、具体的な質問を提示}

### 不足している可能性がある項目
{コード分析・convention から検出された欠落候補}
```

ユーザーの回答後、以下の手順で `spec.json` を更新する：

**既存項目（🟡🔴）の更新** — ユーザーの回答に基づいて `text` と `sources` を更新する：
- ユーザーが内容を修正した場合 → `text` を修正内容に書き換え、`sources` に `feedback — {内容}` を追加（→ Blue に昇格）
- ユーザーが現状を確認した場合 → `text` はそのまま、`sources` に `feedback — ユーザー確認` を追加（→ Blue に昇格）
- ユーザーが「推定で良い」と承認した場合 → `text` はそのまま、`sources` に `discussion` を追加（→ Yellow に昇格）
（既存の source は保持し、上書きしない。signal 評価は最高信頼度のソースを使うため、追加で十分）

**確定済み項目（🔵）の修正** — ユーザーが Blue 項目の変更を申告した場合：
1. 要件の `text`（または `description`）をユーザーの修正内容に書き換える
2. 既存の `sources` を古い根拠として削除し、`feedback — {修正内容}` で置き換える
これにより内容と根拠の両方が更新され、次回の差分ヒアリングで適切に分類される。

**欠落項目（❌）の新規追加** — ユーザーが確認した欠落項目は `spec.json` の該当セクションに新規エントリとして追加する：
- 要件項目 → `scope.in_scope`, `scope.out_of_scope`, `constraints`, `acceptance_criteria` のいずれか。
  `sources` には `feedback — {ユーザー回答の要約}` を設定する。
  `task_refs`: 既存 track（差分ヒアリング）の場合は `metadata.json` に定義済みの task ID を紐付ける。
  新規 track（全体ヒアリング）の場合は Phase 3 Step 4 で tasks 作成後に紐付ける（作成時は空配列で可）。
- ドメイン状態 → `domain_states` に `name`, `description`, 必要に応じて `transitions_to` を追加する。
  （注: `domain_states` エントリは `sources`/`task_refs` を持たない。信号は Stage 2 の AST スキャンで自動評価される。）

**信号再評価** — source 更新・新規追加後、以下を順に実行して `spec.json` と rendered views を最新化する：
1. `bin/sotp track signals <track-id>` — spec signals（Stage 1）を再計算
2. `bin/sotp track domain-state-signals <track-id>` — domain_states がある場合、domain state signals（Stage 2）を再計算
3. `cargo make track-sync-views` — `plan.md` + `registry.md` + `spec.md`（spec.json がある場合）を再生成

#### 4b. 全体ヒアリングモード（既存 spec.json なし — フォールバック）

従来の固定質問リストで全体をヒアリングする：

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

## Phase 1.5: DESIGN REVIEW（planner capability — 必須）

**難易度にかかわらず、すべての機能で planner capability による設計レビューを実施する。**
「S 難易度」「プロンプト変更のみ」であっても、実装の前に planner に以下を確認させる：

1. 変更が影響する**全てのデータフロー**（読込→処理→永続化→再評価）を列挙
2. 関連するスキーマ制約（どのフィールドがどの型に存在するか）を確認
3. エッジケース（コマンド失敗、空データ、既存データの修正）の洗い出し

planner の出力に漏れがあれば、実装後のレビューで繰り返し指摘される。
事前の 1 回の設計レビューで、レビューラウンドの大幅な削減が期待できる。

```bash
# briefing file に Phase 1 の分析結果 + 要件を書き、planner に渡す
cargo make track-local-plan -- --model {model} --briefing-file tmp/planner-briefing.md
```

planner の出力は `knowledge/research/{YYYY-MM-DD-HHMM}-planner-{feature}.md` に保存する
（日時プレフィックスは `date -u +"%Y-%m-%d-%H%M"` で取得）。

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
  - Save to knowledge/research/{feature}-crates.md

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
  `knowledge/research/{capability}-{feature}.md` and reference it

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

差分ヒアリングを実施した場合は、確定済み項目と新規確認項目を明確に区別して提示する。

#### 3a. 差分ヒアリング実施時の提示形式

```markdown
## プロジェクト計画: {feature}

### 仕様の信頼性サマリー
- 🔵 確定済み: {N} 項目（前回仕様から継続）
- 🟡 今回確認済み: {N} 項目（ヒアリングで確認）
- 🔴→🔵 今回解決: {N} 項目（ヒアリングで根拠が付与された）
- ❌→🔵 今回追加: {N} 項目（欠落として検出・補完された）

### コードベース分析（researcher capability）
{Key findings}

### クレート調査（Researcher）
{Crate recommendations, versions, notes — 必要な場合のみ}

### Rust アーキテクチャ設計（planner capability）
{Trait definitions, module structure — 必要な場合のみ}

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
- 承認後に `cargo make spec-approve <track-dir>` で仕様を明示的に承認できます（`approved_at` + `content_hash` を記録）
- standard lane なら `/track:full-cycle <task>` または `/track:implement`、plan-only lane なら `/track:activate <track-id>` を経由して実装を開始
```

#### 3b. 新規仕様（差分ヒアリングなし）の提示形式

```markdown
## プロジェクト計画: {feature}

### コードベース分析（researcher capability）
{Key findings}

### クレート調査（Researcher）
{Crate recommendations, versions, notes}

### Rust アーキテクチャ設計（planner capability）
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
- 承認後に `cargo make spec-approve <track-dir>` で仕様を明示的に承認できます（`approved_at` + `content_hash` を記録）
- standard lane なら `/track:full-cycle <task>` または `/track:implement`、plan-only lane なら `/track:activate <track-id>` を経由して実装を開始
```

### Step 4: Create Track Artifacts（承認後）

ユーザーが計画を承認したら、以下の成果物を作成する。

**タイムスタンプ取得（必須）**: `created_at` / `updated_at` に使う ISO 8601 タイムスタンプは、
推測や固定値ではなく、以下のコマンドで**現在時刻を取得**して使うこと：

```bash
date -u +"%Y-%m-%dT%H:%M:%SZ"
```

UTC の ISO 8601 形式（例: `2026-03-28T00:12:22Z`）を使用する。
既存の `sotp` CLI（`now_iso8601()`）が UTC を使うため、全タイムスタンプを UTC に統一する。
このコマンドの出力をそのまま `created_at` と `updated_at` に使う。手入力や推定は禁止。

1. `track/items/<id>/` ディレクトリを作成（safe slug + timestamp/id で衝突回避）
2. `metadata.json` (SSoT) を作成（schema_version 3, tasks, plan sections）
   - review state は `review.json` で管理される（metadata.json には含めない）
3. `plan.md` を `metadata.json` から `render_plan()` で生成（直接書き込み禁止）
4. `spec.json` (仕様 SSoT) を作成（schema_version 1, status, version, title, goal, scope, constraints, domain_states, acceptance_criteria, additional_sections, related_conventions）
   - Scope, Constraints, Acceptance Criteria の各要件に `sources` 配列でソース帰属を付与する
   - Source タグの種類（5 種）: document, feedback, convention, inference, discussion
   - 参照: `project-docs/conventions/source-attribution.md`
   - `related_conventions` に関連規約ファイルパスを含める
5. `cargo make track-sync-views` で `spec.md` を `spec.json` から自動生成（直接書き込み禁止）
6. `verification.md` を初期化（scope verified, manual steps, result, verified_at）
7. `track/registry.md` を更新（active track row, Current Focus, Last updated）
