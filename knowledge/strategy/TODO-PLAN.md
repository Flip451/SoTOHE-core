# SoTOHE-core 全体計画 v3

> **作成日**: 2026-03-22
> **前版**: `tmp/archive-2026-03-20/TODO-PLAN-2026-03-20.md` (v2)
> **ビジョン**: [`knowledge/strategy/vision.md`](vision.md)
> **変更理由**: ハーネス自身 vs テンプレート出力の区別を明確化。BRIDGE-01 を「生成プロジェクト向けツール」に再定義。
> **リファクタリング詳細**: `tmp/refactoring-plan-2026-03-19.md`（Phase 1.5 のトラック分割は引き続き有効）
> **進捗管理**: [`knowledge/strategy/progress-tracker.md`](progress-tracker.md)
> **TODO 詳細**: [`knowledge/strategy/TODO.md`](TODO.md)

---

## 戦略サマリー v3

**v2**: 基盤を固め → 仕様品質を保証し → テスト生成パイプラインを構築する。

**v3**: 上記に加え、SoTOHE-core 自身のコードとテンプレート出力を区別する。
ハーネス自身のコードは実用的品質で維持し、テンプレートが生成するプロジェクトの出力品質に投資する。

```
Phase 0 (✅)   基盤: shell wrapper Rust 化
Phase 1 (✅)   クイックウィン: 事故予防 + spec テンプレート基盤
Phase 1.5 (▶)  ハーネス自身のコード品質改善（CLI 肥大化解消 + domain 型化）
Phase 2        仕様品質: 信号機 + トレーサビリティ（テスト生成の入力品質保証）
Phase 3        テンプレートツール: spec → テスト生成パイプライン + BRIDGE-01 ← Moat
Phase 4        インフラ: 必要最小限
Phase 5        ワークフロー最適化
```

### v2 → v3 の変更点

| 項目 | v2 | v3 |
|---|---|---|
| typestate パターン | SoTOHE 自身に適用検討 | **生成プロジェクト向けのみ** |
| `impl Fn` 統一 | SoTOHE の usecase を移行 | **生成プロジェクト向け推奨。SoTOHE 自身は trait 維持** |
| BRIDGE-01 | SoTOHE の domain から抽出 | **生成プロジェクトの domain から抽出するツール** |
| ファイル分割規則 | SoTOHE 内部の規約 | **テンプレートが scaffold するディレクトリ構造** |
| Phase 1.5 | 変更なし | 変更なし |

---

## Phase 0: ✅ 完了

| # | 項目 | 状態 |
|---|---|---|
| 0-1 | **STRAT-09** shell wrapper の Rust CLI 集約 | ✅ done (PR #30) |

---

## Phase 1: ✅ 完了（10/10）

詳細は `tmp/archive-2026-03-20/TODO-PLAN-2026-03-17.md` を参照。

---

## Phase 1.5: ハーネス自身のコード品質改善（▶ 進行中）

> **詳細計画**: [`tmp/refactoring-plan-2026-03-19.md`](../../tmp/refactoring-plan-2026-03-19.md)

**目標**: CLI 肥大化解消 + domain 型化。テンプレート開発の生産性向上。

**注意**: Phase 1.5 はハーネス自身の品質改善。typestate や impl Fn への移行は不要。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 1.5-0 | ~~ファイルロックシステム削除~~ | M | ✅ done (PR #41) |
| 1.5-1 | ~~DM-01 Verdict enum~~ | S | ✅ done (PR #42) |
| 1.5-2 | ~~DM-02 GhReviewState enum~~ | S | ✅ done (PR #42) |
| 1.5-3 | ~~DM-03 Severity enum~~ | S | ✅ done (PR #42) |
| 1.5-4 | 設計方針の明文化 (conventions) | S | 未着手 |
| 1.5-5 | ~~CI 検知網~~ | S | ✅ done (PR #46) |
| 1.5-6 | ~~CLI-02: review.rs usecase 移動~~ | M | ✅ done (PR #47 + #49) |
| 1.5-7 | CLI-01: pr.rs usecase 移動 | M | 未着手 |
| 1.5-8 | ERR-09b: activate.rs 分割 | M | 未着手 |
| 1.5-9 | RVW-01: frontmatter パーサー抽出 | S | 未着手 |
| 1.5-10 | RVW-02: conch-parser AST 走査 | M | 未着手 |
| 1.5-11 | ~~GAP-01 タイムスタンプ型化~~ | M | ✅ done (PR #42) |
| 1.5-12 | WF-44/46 codec バリデーション | S | 未着手 |
| 1.5-13 | WF-48 domain API hardening | S | 未着手 |
| 1.5-14 | ~~WF-45 + WF-51~~ | — | ✅ DM-01 で自然消滅 |
| 1.5-15 | WF-52 CLI review 統合テスト | S | 未着手 (CLI-02 後) |
| 1.5-16 | 構造的ロック (pub(crate) + CI) | M | 未着手 |
| 1.5-19 | ~~INF-15: `sotp verify usecase-purity` — usecase 層の I/O 混入検知 CI~~ | S | ✅ done (syn AST ベース、std I/O 網羅ブロック) |
| 1.5-20 | ~~INF-16: `pr_review.rs` hexagonal リファクタリング — `std::fs` / `std::io` を CLI 層に移動~~ | S | ✅ done (PR #51) |
| 1.5-21 | ~~INF-17: `usecase-purity` warning → error 昇格 — CI ブロック化~~ | S | ✅ done (PR #52) |
| 1.5-22 | INF-18: verify ルール定義の外部設定化 — `layer-purity-rules.json` 新設 | S | 未着手 (ドメインロジックの infra 流出防止) |
| 1.5-23 | ~~INF-19: `sotp verify domain-purity` — domain 層 I/O purity CI~~ | S | ✅ done (PR #53) |
| 1.5-24 | ~~INF-20: `conch-parser` を domain → infrastructure に移動~~ | M | ✅ done (PR #54) |
| 1.5-17 | ~~WF-55 Phase 1: view-freshness CI~~ | S | ✅ done (PR #46) |
| 1.5-18 | **capability 追加**: domain_modeler, spec_reviewer, acceptance_reviewer | S | 未着手 |
| 1.5-25 | **RVW-03**: review.json 分離 — review state を metadata.json から独立ファイルに外部化。metadata tampering bypass を構造的に排除 ([ADR](../../knowledge/adr/2026-03-24-1200-review-state-trust-model.md)) | M | 未着手 |
| 1.5-26 | ~~**RVW-10/11**: verdict auto-record + diff scope filtering~~ | M | ✅ done (PR #63, `review-verdict-autorecord-2026-03-25`) |
| 1.5-27 | **RVW-13/15/17**: review infra quality hardening (GitDiffScope テスト, codex-reviewer agent 検証, auto-record e2e) | M | ▶ in_progress (`review-infra-quality-2026-03-25`) |

---

## Phase 2: 仕様品質（テスト生成の入力品質保証）

**目標**: 信号機 + トレーサビリティを最小セットで導入。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 2-1 | **TSUMIKI-01** Spec 信号機評価 (Stage 1) + Domain States 存在チェック (旧 2-7 最小版統合) | M | ✅ done |
| 2-1b | **spec.json SSoT 化** — spec.md を rendered view に降格 | M | ✅ done (PR #57) |
| 2-2 | **SPEC-05** Domain States 信号機 (Stage 2) + 遷移関数検証 + spec.json `domain_state_signals` | M | ✅ done (PR #58) |
| 2-3 | ~~**CC-SDD-01** 要件-タスク双方向トレーサビリティ~~ | M | ✅ done (PR #60, `req-task-traceability-2026-03-24`) |
| 2-4 | ~~**CC-SDD-02** 明示的承認ゲート~~ | S | ✅ done (PR #62, `spec-approval-gate-2026-03-24`) |
| 2-5 | **TSUMIKI-03** 差分ヒアリング | S | 未着手 |
| ~~2-6~~ | ~~**SSoT-07** 二重書き込み解消~~ | — | スキップ（spec.json SSoT + ADR + view-freshness CI で解決済み） |
| ~~2-7~~ | ~~spec.md Domain States 必須化~~ | — | 2-1 に最小版統合、2-2 で完全実装 |

### 2段階信号機アーキテクチャ (2026-03-23)

**設計決定**: spec 信号機（Stage 1）と Domain States 信号機（Stage 2）は独立した2つのゲート。

```
Stage 1 (Track A = 2-1):
  spec.md [source: ...] tags → ConfidenceSignal → spec.md frontmatter signals:
  Gate: red == 0
  + Domain States セクション存在チェック (Stage 2 への橋渡し)

Stage 2 (Track B = 2-2):
  spec.md ## Domain States → per-state signal → metadata.json domain_state_signals
  Gate: red == 0, Stage 1 通過が前提条件
  + TrackMetadata に Option<SignalCounts> 追加
  + plan.md 信号サマリー
```

**共有プリミティブ**: `ConfidenceSignal` + `SignalCounts` は Stage 1 で定義し Stage 2 も使う
**Stage 固有**: `SignalBasis` は Stage 1 専用。Stage 2 は Domain States エントリを直接モデル化

**先送り項目** (Stage 1 の scope 外):

| 項目 | 先送り先 | 理由 |
|---|---|---|
| `TrackMetadata` への `Option<SignalCounts>` | 2-2 (Stage 2) | workflow aggregate は Stage 2 の関心事 |
| metadata.json `domain_state_signals` | 2-2 (Stage 2) | Domain States パース後に追加 |
| plan.md 信号サマリー | 2-2 (Stage 2) | Stage 2 集計が前提 |
| per-item `SignalBasis` 永続化 | Phase 3 | CC-SDD-01 トレーサビリティと連動 |
| `Contradicted` basis 自動検出 | Phase 3 | SPEC-01 降格ループの前提 |
| spec ↔ code 整合性チェック | Phase 3 | BRIDGE-01 スコープ拡張と連動 |
| Multi-source (カンマ区切り) tag 対応 | spec.json SSoT track | JSON 配列で表現。markdown カンマ分割の edge case を回避 |
| spec.md frontmatter `signals:` ドリフトチェック | spec.json SSoT track | JSON SSoT 化で frontmatter 操作を廃止 |
| spec.json SSoT 化（spec.md を JSON から rendered view に） | Phase 2 新規 track | metadata.json → plan.md パターンを spec にも適用。markdown パースの edge case を根本解決 |
| 4スペースインデントコードブロックのスキップ | 不要（fenced blocks のみ使用） | プロジェクトで 4-space コードブロック未使用 |

### 2-2 (Stage 2) 計画メモ

> 出典: `tmp/domain-modeling-guarantee-2026-03-23.md` §懸念2
> 更新: 2026-03-23 spec.json SSoT 完了後に計画確定

**🔵 基準（確定）**: syn AST スキャンで domain コードから型名 + 遷移関数を自動検出。主観排除。

| Signal | 基準 |
|--------|------|
| 🔵 Blue | 型が domain コードに存在 AND (`transitions_to: []` 終端 OR 宣言された遷移関数が全て存在) |
| 🟡 Yellow | 型存在だが遷移関数未発見、または `transitions_to` 未宣言 |
| 🔴 Red | 型未存在、またはプレースホルダー |

**追加設計決定**:
- `DomainStateEntry` に `transitions_to` (optional Vec<String>) 追加。省略=未定義、`[]=`終端
- 遷移関数検出: syn で `Result<T, E>` / `Option<T>` 内部型をアンラップして遷移先判定
- `transitions_to` の参照先が `domain_states` に存在しない場合は検証エラー
- Stage 1 (spec signals red==0) を前提条件として `sotp verify spec-states` 内で強制
- 未宣言遷移の検出（code→spec 逆方向）は Phase 3 (spec ↔ code 整合性) に先送り
- モジュールスコープ曖昧性（同名型）は Phase 3 (BRIDGE-01) に先送り、v1 は名前一致

---

## Phase 3: テンプレートツール — テスト生成パイプライン（Moat）

**目標**: テンプレートが生成するプロジェクトに対して、spec → テスト自動生成の仕組みを提供する。

### テスト生成の 3 手法

| 手法 | 対象 | 入力 |
|---|---|---|
| spec 例 → テスト変換 | domain impl | spec の Given/When/Then |
| proptest + typestate | domain impl | export-schema のシグネチャ（関数の存在 = 有効遷移） |
| usecase モック自動生成 | usecase | `impl Fn` のクロージャモック |

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 3-1 | **BRIDGE-01** `sotp domain export-schema` (syn) | M | 生成プロジェクトの domain から pub シグネチャを抽出。テスト生成 AI の唯一の型コンテキスト |
| 3-2 | **spec 例 → テストスケルトン自動生成** | M | spec の Given/When/Then → `#[test]` を `/track:plan` Phase B で生成 |
| 3-3 | **proptest テンプレート** | M | export-schema + typestate から proptest テストを生成 |
| 3-4 | **usecase テストテンプレート** | M | `impl Fn` パターンのクロージャモックテストを自動生成 |
| 3-5 | **HARNESS-03** Stop hook テスト通過ゲート | S | テスト通過をシステム強制 |
| 3-6 | **WF-25** CI カバレッジ目標 | M | 80% ルール |
| 3-7 | **TSUMIKI-04** 要件網羅率 | M | spec → テストのトレーサビリティ |
| 3-8 | **SPEC-03** 信号機昇格を CI 証拠に限定 | M | テスト通過 = 🟡→🔵 |
| 3-9 | **SPEC-01** 信号機自動降格ループ | M | テスト失敗 → 🔴 |
| 3-10 | WF-47/50/53 findings 自動配線 | M | review 改善 |
| 3-11 | WF-49 streak リセット | S | 監査証跡 |
| 3-12 | **spec ↔ code 整合性チェック** | M | spec の Domain States と code の domain 型の一致を検証。BRIDGE-01 の出力と spec.md の Domain States セクションを突合し、未実装・不一致を検出 |

### テンプレートが推奨する生成プロジェクトの設計

| 設計要素 | 推奨パターン | SoTOHE 自身に適用？ |
|---|---|---|
| 状態遷移 | typestate（関数の存在 = 有効遷移） | いいえ |
| usecase DI | `impl Fn`（モック = クロージャ） | いいえ（trait 維持） |
| ファイル分割 | DDD 概念 + pub 可視性フィルタ | はい（CLI-02 で実施済み） |
| 永続化 | typestate enum + serde 変換 | いいえ |

---

## Phase 4: インフラ（必要最小限）

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 4-1 | ~~CON-07~~ | M | ✅ done |
| 4-2 | ~~SEC-09~~ | S | ✅ done |
| 4-3 | CON-08 scratch file 競合 | M | 未着手 |
| 4-4 | SEC-11 git 部分文字列過剰ブロック | M | 未着手 |
| 4-5 | STRAT-08 外部非同期 state 永続化 | M | 未着手 |
| 4-6 | ERR-08 pr-review 中断耐性 | M | 未着手 |
| 4-7 | INF-12 hook cold build timeout | S | 未着手 |
| 4-8 | SPEC-04 エフェメラル worktree 分離 | L | 未着手 |

---

## Phase 5: ワークフロー最適化

| # | 項目 | 難易度 |
|---|---|---|
| 5-1 | SURVEY-06 clarify フェーズ | M |
| 5-2 | SURVEY-08 checklist | M |
| 5-3 | Session/Bootstrap/Briefing 統合 | S |
| 5-4 | SURVEY-09 Hook profile | M |
| 5-5 | HARNESS-01 PostToolUse 構造化フィードバック | S |
| 5-6 | GAP-02 PR State Machine | M |
| 5-7 | GAP-11 tracing 導入 | M |

---

## Phase 間の依存関係

```
Phase 0 (✅) + Phase 1 (✅)
    ↓
Phase 1.5 (▶ ハーネス自身の品質改善)
    ↓ domain 型化完了、CLI が薄くなる
Phase 2 (仕様品質 — テスト生成の入力品質)
    ↓ 信号機 + Domain States + トレーサビリティが整う
Phase 3 (テスト生成パイプライン) ← Moat
    ↓ テンプレートが「spec → テスト → 実装」を提供
Phase 4 (インフラ)  Phase 5 (ワークフロー)  ← 並行可能
```

**Phase 1.5 はハーネスの保守、Phase 3 がテンプレートの価値。両者は別の目的。**

---

## 見積もり

| Phase | 項目 | 残 | 推定日数 |
|---|---|---|---|
| 1.5 | 20 | 9 | 3 日 |
| 2 | 7 | 1 | 0.5 日 |
| 3 | 12 | 12 | 5 日 |
| 4 | 8 | 6 | 3 日 |
| 5 | 7 | 7 | 3 日 |
| **合計** | **54** | **35** | **~14.5 日** |

> 更新: 2026-03-25。Phase 2 は CC-SDD-01/02 完了で残り TSUMIKI-03 のみ。Phase 1.5 に RVW-10/11 (done) + RVW-13/15/17 (in_progress) を追加。
