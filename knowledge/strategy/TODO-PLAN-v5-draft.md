# SoTOHE-core 全体計画 v5 ドラフト

> **作成日**: 2026-04-07
> **前版**: v3 (正式版) + v4 ドラフト (2026-03-23, sotp スタンドアロン化)
> **ビジョン**: [`knowledge/strategy/vision.md`](vision.md) ← v5 採用時に改訂
> **変更理由**:
>   1. v4 ドラフトの sotp スタンドアロン化方針を継承（2026-03-23）
>   2. ハーネスエンジニアリング業界調査を踏まえたギャップ補完（2026-04-07）
> **業界調査**: [`knowledge/research/2026-04-07-1234-harness-engineering-landscape.md`](../research/2026-04-07-1234-harness-engineering-landscape.md)
> **分析レポート**: [`tmp/template-overfitting-analysis-2026-03-23.md`](./template-overfitting-analysis-2026-03-23.md)
> **リファクタリング詳細**: [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md)
> **進捗管理**: [`knowledge/strategy/progress-tracker.md`](progress-tracker.md) ← v5 採用時に改訂
> **TODO 詳細**: [`knowledge/strategy/TODO.md`](TODO.md)

---

## 戦略サマリー v5

**v2**: 基盤を固め → 仕様品質を保証し → テスト生成パイプラインを構築する。

**v3**: 上記に加え、SoTOHE-core 自身のコードとテンプレート出力を区別する。

**v4 ドラフト**: sotp CLI を独立ツールとして物理分離。テンプレート利用者は sotp の正規ワークフローを使う前提。

**v5**: v4 ドラフトの sotp スタンドアロン化に加え、業界のハーネスエンジニアリング体系化を踏まえてギャップを埋める。

1. **sotp スタンドアロン化** (v4 由来): sotp CLI を独立ツールとして物理分離。
   テンプレート利用者は sotp の正規ワークフロー（track, hooks, review cycle）を使う前提。
2. **業界ギャップ補完** (v5 新規): Fowler Taxonomy / Anthropic Three-Agent Harness 等の
   業界体系化を踏まえ、SoTOHE-core が先取りしていた設計を明示的に位置づけるとともに、
   残ギャップ（Drift Detection / Reviewer Calibration / Harness Template 展開）を計画に統合する。

### Fowler Taxonomy との対応

SoTOHE-core は Fowler が体系化した harness engineering の主要概念をほぼ先取りしている。

```
Fowler: Guides (Feedforward)        → .claude/rules/, conventions, architecture-rules.json
Fowler: Sensors (Feedback)          → CI gates (computational) + Codex reviewer (inferential)
Fowler: Maintainability Harness     → clippy, fmt, deny, check-layers, usecase-purity ← 成熟
Fowler: Architecture Fitness        → architecture-rules.json + check-layers ← 成熟
Fowler: Behaviour Harness           → 2つの信号機(仕様書+型設計書) + Phase 3 テスト生成で構築中
Fowler: Harnessability              → 04-coding-principles.md (enum-first, typestate, newtype)
Fowler: Harness Template            → Phase 6 で展開予定
Anthropic: Plan/Generate/Evaluate   → planner (Claude Opus) / implementer / reviewer (Codex) 分離
```

### Phase 概要

```
Phase 0 (✅)   基盤: shell wrapper Rust 化
Phase 1 (✅)   クイックウィン: 事故予防 + spec テンプレート基盤
Phase 1.5 (▶)  sotp CLI 品質改善 + 論理分離
Phase 2 (✅)   仕様品質: 信号機 + トレーサビリティ（テスト生成の入力品質保証）
Phase 2b (✅)  ヒアリング UX 改善: 構造化質問 + モード選択 + プロセス記録
Phase 2c (▶)   domain-types.json 分離: 型宣言の独立ライフサイクル + 5 カテゴリ型信号
Phase 3        Behaviour Harness: sotp テスト生成サブコマンド ← Moat（BRIDGE-01 完了済み）
Phase 4        インフラ + sotp 配布 + Drift Detection
Phase 5        ワークフロー最適化 + Reviewer Calibration + GitHub 外部観測面
Phase 6        Harness Template 展開: テンプレート外枠 + 複数トポロジー対応
```

### v4 → v5 の変更点（業界調査由来）

| 項目 | v4 ドラフト | v5 |
|---|---|---|
| Phase 3 の位置づけ | sotp テスト生成サブコマンド | **Behaviour Harness**（業界最大ギャップへの回答として明確化） |
| Phase 3 項目 | 3-1〜3-11 | **+ 3-12 spec↔code 整合性 + 3-13 シグナル伝播**（v3 から復元） |
| Phase 4 | インフラ + sotp 配布 | **+ Drift Detection**（DRIFT-01/02） |
| Phase 5 | ワークフロー最適化 | **+ Reviewer Calibration**（CALIB-01） |
| Phase 6 | テンプレート外枠のみ | **+ Harness Template 展開**（TMPL-01: 複数トポロジー対応） |
| 業界対応表 | なし | Fowler Taxonomy / Anthropic Three-Agent 対応を明記 |

### v3 → v4 の変更点（sotp スタンドアロン化）

| 項目 | v3 | v4 |
|---|---|---|
| sotp の位置づけ | テンプレートに埋め込み | **スタンドアロン CLI ツール** |
| テンプレートの Cargo workspace | sotp ソース込み | **ユーザーのコード専用（空スケルトン）** |
| BRIDGE-01 | 生成プロジェクト向けツール | **sotp のサブコマンド** |
| Phase 1.5 | コード品質改善のみ | **+ 論理分離（SPLIT-01/02）** |
| Phase 4 | インフラのみ | **+ sotp 配布（SPLIT-03/04/05）** |
| Phase 6 | なし | **新設: テンプレート外枠（scaffold）** |

### v2 → v3 の変更点（参考）

| 項目 | v2 | v3 |
|---|---|---|
| typestate パターン | SoTOHE 自身に適用検討 | **生成プロジェクト向けのみ** |
| `impl Fn` 統一 | SoTOHE の usecase を移行 | **生成プロジェクト向け推奨。SoTOHE 自身は trait 維持** |
| BRIDGE-01 | SoTOHE の domain から抽出 | **生成プロジェクトの domain から抽出するツール** |
| ファイル分割規則 | SoTOHE 内部の規約 | **テンプレートが scaffold するディレクトリ構造** |

---

## Phase 0: ✅ 完了

| # | 項目 | 状態 |
|---|---|---|
| 0-1 | **STRAT-09** shell wrapper の Rust CLI 集約 | ✅ done (PR #30) |

---

## Phase 1: ✅ 完了（10/10）

詳細は `tmp/archive-2026-03-20/TODO-PLAN-2026-03-17.md` を参照。

---

## Phase 1.5: sotp CLI 品質改善 + 論理分離（▶ 進行中）

> **詳細計画**: [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md)
> **Review 改善計画**: [`knowledge/strategy/rvw-remediation-plan.md`](../../knowledge/strategy/rvw-remediation-plan.md)（Phase A-J、58 件、~13-16 日）

**目標**: CLI 肥大化解消 + domain 型化 + sotp とテンプレートの論理的境界を確立。

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
| 1.5-8 | ERR-09b: track/activate.rs 分割 (1890行) | M | 未着手 |
| 1.5-9 | RVW-01: frontmatter パーサー抽出 | S | 未着手 |
| 1.5-10 | RVW-02: conch-parser AST 走査 | M | 未着手 |
| 1.5-11 | ~~GAP-01 タイムスタンプ型化~~ | M | ✅ done (PR #42) |
| 1.5-12 | WF-44/46 codec バリデーション | S | 未着手 |
| 1.5-13 | WF-48 domain API hardening | S | 未着手 |
| 1.5-14 | ~~WF-45 + WF-51~~ | — | ✅ DM-01 で自然消滅 |
| 1.5-15 | WF-52 CLI review 統合テスト | S | 未着手 (CLI-02 後) |
| 1.5-16 | 構造的ロック (pub(crate) + CI) | M | 未着手 |
| 1.5-17 | ~~WF-55 Phase 1: view-freshness CI~~ | S | ✅ done (PR #46) |
| 1.5-18 | **capability 追加**: domain_modeler, spec_reviewer, acceptance_reviewer | S | 未着手 |
| 1.5-19 | ~~INF-15: `sotp verify usecase-purity` — usecase 層の I/O 混入検知 CI~~ | S | ✅ done |
| 1.5-20 | ~~INF-16: `pr_review.rs` hexagonal リファクタリング~~ | S | ✅ done (PR #51) |
| 1.5-21 | ~~INF-17: `usecase-purity` warning → error 昇格~~ | S | ✅ done (PR #52) |
| 1.5-22 | INF-18: verify ルール定義の外部設定化 | S | 未着手 |
| 1.5-23 | ~~INF-19: `sotp verify domain-purity` — domain 層 I/O purity CI~~ | S | ✅ done (PR #53) |
| 1.5-24 | ~~INF-20: `conch-parser` を domain → infrastructure に移動~~ | M | ✅ done (PR #54) |
| 1.5-25 | ~~**RVW-03**: review.json 分離~~ | M | ✅ done |
| 1.5-26 | ~~**RVW-10/11**: verdict auto-record + diff scope filtering~~ | M | ✅ done (PR #63) |
| 1.5-27 | ~~**RVW-13/15/17**: review infra quality hardening~~ | M | ✅ done (PR #64) |
| 1.5-28 | ~~**WF-59**: review-scope manifest hash~~ | M | ✅ done |
| 1.5-29 | **WF-43**: verdict 改ざん防止 | L | archived (review system v2 で superseded) |
| **1.5-30** | **SPLIT-01: sotp / テンプレートの論理分離** | **M** | **NEW (v4)** |
| **1.5-31** | **SPLIT-02: bin/sotp パス抽象化** | **S** | **NEW (v4)** |

### SPLIT-01: sotp / テンプレートの論理分離（v4 由来）

**目標**: 同一リポ内で sotp CLI のコードとテンプレートスケルトンの境界を明確にする。

**内容**:
- README / CLAUDE.md に「sotp = スタンドアロン CLI ツール」「テンプレート = sotp を使うプロジェクト基盤」の区別を明記
- Cargo workspace 内で sotp グループ（libs/domain, usecase, infrastructure, apps/cli）とテンプレートグループ（apps/server）を文書化
- `architecture-rules.json` に sotp/template 境界を反映（将来の物理分割の準備）
- vision v5 を作成し `knowledge/strategy/vision.md` に保存

**やらないこと（Phase 4 に送る）**:
- 物理的なリポ分割
- sotp のバイナリ配布
- Dockerfile の sotp インストール化

### SPLIT-02: bin/sotp パス抽象化（v4 由来）

**目標**: `bin/sotp` のハードコード参照を抽象化し、将来の PATH ベースインストールに備える。

**内容**:
- Makefile.toml の `SOTP_BIN` 変数を導入: デフォルト `bin/sotp`、環境変数 `SOTP_BIN` でオーバーライド可能
- hooks / scripts 内の `bin/sotp` 参照を `SOTP_BIN` 変数経由に変更
- `sotp` が PATH 上にある場合は `bin/sotp` より優先するロジック

---

## Phase 2: ✅ 完了（+ 2c 進行中）

**目標**: 信号機 + トレーサビリティを最小セットで導入。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 2-1 | **TSUMIKI-01** Spec 信号機評価 (Stage 1) + Domain States 存在チェック | M | ✅ done |
| 2-1b | **spec.json SSoT 化** — spec.md を rendered view に降格 | M | ✅ done (PR #57) |
| 2-2 | **SPEC-05** Domain States 信号機 (Stage 2) + 遷移関数検証 | M | ✅ done (PR #58) |
| 2-3 | ~~**CC-SDD-01** 要件-タスク双方向トレーサビリティ~~ | M | ✅ done (PR #60) |
| 2-4 | ~~**CC-SDD-02** 明示的承認ゲート~~ | S | ✅ done (PR #62) |
| 2-5 | ~~**TSUMIKI-03** 差分ヒアリング~~ | S | ✅ done |
| ~~2-6~~ | ~~**SSoT-07** 二重書き込み解消~~ | — | スキップ（spec.json SSoT で解決済み） |
| ~~2-7~~ | ~~spec.md Domain States 必須化~~ | — | 2-1 に統合、2-2 で完全実装 |

### Phase 2c: 型設計書の信号機 — domain-types.json 分離（▶ 進行中）

> **トラック**: `spec-domain-types-v2-2026-04-07`（現在着手中）
> **ADR**: `knowledge/adr/2026-04-07-0045-domain-types-separation.md`
>
> Phase 2 の Stage 2（Domain States 信号機）を **型設計書の信号機** として再設計する。
> 「仕様書の信号機」（Stage 1）とは独立したライフサイクルを持つ。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 2c-1 | DomainTypeKind enum + DomainTypeEntry + DomainTypeSignal + DomainTypesDocument 型定義 | M | 未着手（T001-T003） |
| 2c-2 | evaluate_domain_type_signals() + SpecDocument から domain_states 削除 | M | 未着手（T003-T004） |
| 2c-3 | domain-types.json codec + domain-types.md renderer + verify 切替 | M | 未着手（T005-T009） |
| 2c-4 | CLI: domain-type-signals コマンド + views sync + マイグレーション | M | 未着手（T010-T012） |
| 2c-5 | DESIGN.md + ADR 更新 | S | 未着手（T013） |

### 2つの信号機アーキテクチャ

SoTOHE-core には **2つの独立した信号機** がある。それぞれ異なるドキュメントの品質を測定し、
独立したゲートとして動作する。

> ADR: `2026-03-23-2120-two-stage-signal-architecture.md`, `2026-03-23-1020-two-stage-signals.md`
> ADR: `2026-04-07-0045-domain-types-separation.md`（Phase 2c で改訂）

```
┌─────────────────────────────────────────────────────────────────┐
│  仕様書の信号機 (Stage 1)                                       │
│  ファイル: spec.json / spec.md                                   │
│  評価対象: 要件の出典 (source tags)                              │
│  信号: Blue / Yellow / Red (3値)                                 │
│  ゲート: red == 0 で通過                                         │
│  SSoT: spec.json → spec.md (rendered view)                      │
│  ADR: 2026-03-23-1010-three-level-signals.md                    │
│                                                                  │
│  Blue:   出典が信頼できるドキュメントに紐づく                    │
│  Yellow: 出典があるが確証が弱い（要確認）                        │
│  Red:    出典なし / プレースホルダー（ブロック）                  │
│                                                                  │
│  共有プリミティブ: ConfidenceSignal + SignalCounts               │
│  Stage 1 固有: SignalBasis (出典の理由を追跡)                    │
└─────────────────────────────────────────────────────────────────┘
        ↓ Stage 1 通過が前提条件
┌─────────────────────────────────────────────────────────────────┐
│  型設計書の信号機 (Stage 2)  ← Phase 2c で再設計                │
│  ファイル: domain-types.json / domain-types.md (NEW)             │
│  評価対象: 型宣言と実装の一致度                                  │
│  信号: Blue / Red (2値, Yellow 廃止)                             │
│  ゲート: red == 0 で通過                                         │
│  SSoT: domain-types.json → domain-types.md (rendered view)      │
│  ADR: 2026-04-07-0045-domain-types-separation.md                │
│                                                                  │
│  Blue: spec の型宣言と code の実装が完全一致                     │
│  Red:  型なし / 不一致 / 未宣言（全て）                          │
│                                                                  │
│  入力: CodeScanResult + Optional SchemaExport (BRIDGE-01)        │
│  5カテゴリ: DomainTypeKind enum (enum-first パターン)            │
└─────────────────────────────────────────────────────────────────┘
        ↓ 両方通過が Phase 3 の前提
┌─────────────────────────────────────────────────────────────────┐
│  CI ゲート (信号機ではない)                                      │
│  coverage (spec-coverage): 二値。信号機の 3 段階に馴染まない    │
│  ADR: 2026-03-24-0900-coverage-not-a-signal.md                  │
└─────────────────────────────────────────────────────────────────┘
```

### 分離の動機（ADR 2026-04-07-0045）

**ライフサイクルの分離**:
- 仕様書 (spec.json): 承認後に凍結。要件の出典と確度を管理
- 型設計書 (domain-types.json): 実装の進捗に応じて更新。1 ファイルに混在すると content hash が不必要に無効化

**型カテゴリの導入**:
旧 `DomainStateEntry` は `name + description + transitions_to` のみ。typestate 以外の型（enum, value object, error type, trait port）を区別できなかった。

**DomainTypeKind の 5 カテゴリ** (enum-first パターン, `04-coding-principles.md` 準拠):

| kind | 検証データ | Blue 条件 | Red 条件 |
|------|-----------|-----------|----------|
| `Typestate` | `transitions_to: Vec<String>` | 型存在 + 全遷移関数発見 | 型なし / 遷移不足 |
| `Enum` | `expected_variants: Vec<String>` | 型存在 + variants 完全一致 | 型なし / variant 過不足 |
| `ValueObject` | (なし) | 型存在 | 型なし |
| `ErrorType` | `expected_variants: Vec<String>` | 型存在 + expected_variants 全カバー | 型なし / variant 不足 |
| `TraitPort` | `expected_methods: Vec<String>` | trait 存在 + 全メソッド発見 | trait なし / メソッド不足 |

**Yellow 廃止の理由**: spec に型を宣言したなら完全に書くべき。`transitions_to: None`（未宣言）が Yellow だった v1 の曖昧さを排除。Yellow は Stage 1 でのみ使用。

**approved フィールド（将来用）**: `DomainTypeEntry` に `approved: bool` を事前追加。手動エントリは `true`、将来の AI 自動追加は `false`。Phase 2c では approved による信号分岐は未実装。逆方向チェック（code → spec）で AI が自動登録する際に Yellow 再導入の可能性あり。

### SchemaExport 連携（BRIDGE-01）

`evaluate_domain_type_signals()` は 2 つの入力を受け取る:

1. **CodeScanResult** (syn AST): 型名の存在チェック（必須）
2. **SchemaExport** (rustdoc JSON, BRIDGE-01): variant 名・メソッド名の詳細チェック（optional）

nightly 未インストール環境では SchemaExport なしで部分検証（型存在のみ）を行う。
SchemaExport があれば Enum/ErrorType の variant 一致、TraitPort のメソッド一致まで検証。

### Phase 3 への接続

Phase 2c 完了により、Phase 3 の以下の項目の前提が整う:

| Phase 3 項目 | 依存する Phase 2c 成果 |
|---|---|
| 3-12 spec ↔ code 整合性チェック | domain-types.json + SchemaExport の突合基盤 |
| 3-8 SPEC-03 信号機昇格を CI 証拠に限定 | Blue/Red 2 値の厳格な判定基準 |
| 3-9 SPEC-01 信号機自動降格ループ | domain-types.json の独立ライフサイクル |
| 3-13 TSUMIKI-08 シグナル伝播 | domain-types signals → metadata.json タスクへの伝播 |

### 先送り項目

| 項目 | 先送り先 | 理由 |
|---|---|---|
| per-item `SignalBasis` 永続化 | Phase 3 | CC-SDD-01 トレーサビリティと連動 |
| `Contradicted` basis 自動検出 | Phase 3 | SPEC-01 降格ループの前提 |
| spec ↔ code 双方向整合性チェック CLI | Phase 3 (3-12) | ADR `2026-03-23-2130` で先送り決定。BRIDGE-01 完了により実装可能に |
| Yellow 再導入 (AI 自動追加 + 承認ゲート) | 将来 | `approved` フィールドは事前追加済み。ADR Reassess When 参照 |
| Stage 1 の Blue/Red 2値統一 | 将来 | ADR Reassess When: 現時点で Stage 1 の Yellow には意味がある |

---

## Phase 2b: ✅ 完了（ヒアリング UX 改善）

**目標**: Phase 3 のテスト生成パイプラインに渡す spec の品質を、ヒアリング UX の改善で底上げする。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 2b-1 | ~~**TSUMIKI-05** 構造化ヒアリング UX~~ | S | ✅ done |
| 2b-2 | ~~**TSUMIKI-06** ヒアリング作業規模選定~~ | S | ✅ done |
| 2b-3 | ~~**TSUMIKI-07** ヒアリング記録~~ | S | ✅ done |

---

## Phase 3: Behaviour Harness — sotp テスト生成サブコマンド（Moat）

> **v5 での位置づけ**: v4 ドラフトの「sotp テスト生成サブコマンド」を、業界の Fowler Taxonomy
> における **Behaviour Harness**（機能的正確性の検証層）として明確に位置づける。
>
> Fowler は「AI生成テストへの過度な信頼は不十分」と警告しており、SoTOHE-core の
> spec.json SSoT + 信号機 + Given/When/Then → テスト変換は、
> **入力品質から保証する Behaviour Harness** として業界最大のギャップを埋める。
>
> sotp が独立ツールであることを前提に、`sotp domain export-schema` 等は
> テンプレート利用者が自分のプロジェクトで `sotp` コマンドとして実行する。

**目標**: sotp のサブコマンドとして、生成プロジェクトの spec → テスト自動生成パイプラインを提供する。

### テスト生成の 3 手法

| 手法 | 対象 | 入力 |
|---|---|---|
| spec 例 → テスト変換 | domain impl | spec の Given/When/Then |
| proptest + typestate | domain impl | export-schema のシグネチャ（関数の存在 = 有効遷移） |
| usecase モック自動生成 | usecase | `impl Fn` のクロージャモック |

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 3-1 | ~~**BRIDGE-01** `sotp domain export-schema` (rustdoc JSON)~~ | M | ✅ done (`bridge01-export-schema-2026-04-06`)。rustdoc JSON ベース。syn → rustdoc JSON に方針転換（コンパイラ再発明を回避） |
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
| 3-12 | **spec ↔ code 整合性チェック** | M | domain-types.json と SchemaExport (BRIDGE-01) を突合。BRIDGE-01 完了 + domain-types.json 分離で実装可能に (ADR `2026-03-23-2130`) |
| 3-13 | **TSUMIKI-08** シグナル伝播 | M | spec.json 信号 → metadata.json タスクへ worst-case 伝播 |

### テンプレートが推奨する生成プロジェクトの設計

| 設計要素 | 推奨パターン | SoTOHE 自身に適用？ |
|---|---|---|
| 状態遷移 | typestate（関数の存在 = 有効遷移） | いいえ |
| usecase DI | `impl Fn`（モック = クロージャ） | いいえ（trait 維持） |
| ファイル分割 | DDD 概念 + pub 可視性フィルタ | はい（CLI-02 で実施済み） |
| 永続化 | domain: typestate 維持、infra: serde enum DTO に変換 | いいえ |

---

## Phase 4: インフラ + sotp 配布 + Drift Detection

> **v4 由来**: sotp 配布関連 3 項目（SPLIT-03/04/05）
> **v5 追加**: Drift Detection 2 項目（業界調査 — Fowler の「Continuous Monitoring」層に対応）

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 4-1 | ~~CON-07~~ | M | ✅ done |
| 4-2 | ~~SEC-09~~ | S | ✅ done |
| 4-3 | CON-08 scratch file 競合 | M | 未着手 |
| 4-4 | SEC-11 git 部分文字列過剰ブロック | M | 未着手 |
| 4-5 | STRAT-08 外部非同期 state 永続化 | M | 未着手 |
| 4-6 | ~~ERR-08 pr-review 中断耐性~~ | M | ✅ done |
| 4-7 | INF-12 hook cold build timeout | S | 未着手 |
| 4-8 | SPEC-04 エフェメラル worktree 分離 | L | 未着手 |
| **4-9** | **SPLIT-03: sotp crate 公開準備** | **M** | **NEW (v4)** |
| **4-10** | **SPLIT-04: sotp バイナリ配布 (GitHub Releases)** | **M** | **NEW (v4)** |
| **4-11** | **SPLIT-05: Dockerfile sotp インストール化** | **S** | **NEW (v4)** |
| **4-12** | **DRIFT-01: アーキテクチャドリフト定期スキャン** | **M** | **NEW (v5)** |
| **4-13** | **DRIFT-02: 依存性 staleness + デッドコード自動検出** | **S** | **NEW (v5)** |

### SPLIT-03: sotp crate 公開準備（v4 由来）

**内容**:
- sotp の Cargo.toml を publish 可能な状態に整備（license, description, repository 等）
- `cargo install sotp` でインストールできるよう crates.io 公開準備
- バージョニング戦略の決定（semver、テンプレートとの互換性マトリクス）

### SPLIT-04: sotp バイナリ配布（v4 由来）

**内容**:
- GitHub Actions で sotp バイナリをクロスコンパイル + リリース
- `cargo-binstall` 対応メタデータ
- インストール手順のドキュメント（README, DEVELOPER_AI_WORKFLOW.md）

### SPLIT-05: Dockerfile sotp インストール化（v4 由来）

**内容**:
- Dockerfile の tools ステージに sotp バイナリインストールを追加
- `bin/sotp` ビルドステップの代替として sotp がコンテナ内で直接使えるように
- テンプレートの bootstrap フロー更新

### DRIFT-01: アーキテクチャドリフト定期スキャン（v5 新規 — 業界調査由来）

> Fowler Taxonomy: Continuous Monitoring 層。Pre-integration は強いが post-integration が手薄。

**内容**:
- `sotp verify arch-drift` サブコマンド: `architecture-rules.json` と実際の依存グラフの乖離を検出
- `check-layers` の拡張版: 新規追加された crate が正しいレイヤーに配置されているか、
  既存 crate 間に新たな不正依存が生まれていないかを検出
- CI の定期実行（weekly cron）または merge 後フックでの自動実行

### DRIFT-02: 依存性 staleness + デッドコード自動検出（v5 新規 — 業界調査由来）

**内容**:
- `cargo make machete` の定期自動実行 + レポート生成
- `cargo outdated` 相当の staleness チェック（major/minor/patch 別に分類）
- 結果を `knowledge/research/drift-report-{date}.md` に出力

---

## Phase 5: ワークフロー最適化 + Reviewer Calibration + GitHub 外部観測面

> **v5 追加**:
> - Reviewer Calibration（業界調査 — Anthropic の「Evaluation Calibration」対応）
> - GitHub Native Harness（2026-04-05 調査 — 内部オーケストレーションの外部観測面）

| # | 項目 | 難易度 |
|---|---|---|
| 5-1 | SURVEY-06 clarify フェーズ | M |
| 5-2 | SURVEY-08 checklist | M |
| 5-3 | Session/Bootstrap/Briefing 統合 | S |
| 5-4 | SURVEY-09 Hook profile | M |
| 5-5 | HARNESS-01 PostToolUse 構造化フィードバック | S |
| 5-6 | GAP-02 PR State Machine | M |
| 5-7 | GAP-11 tracing 導入 | M |
| **5-8** | **CALIB-01: Reviewer Calibration 基準** | **M** |
| **5-9** | **GH-01: GitHub Native Harness — Issue Intake + State Projection** | **M** |
| **5-10** | **GH-02: Label-driven Handoff + Harness Scorecard** | **M** |

### CALIB-01: Reviewer Calibration 基準（v5 新規 — 業界調査由来）

> Anthropic の three-agent harness では evaluator に few-shot examples + 明示的スコアリング基準を
> 渡すことで判定品質を安定させている。SoTOHE-core の reviewer capability にも適用可能。

**目標**: reviewer に渡すキャリブレーション基準を決定論的に定義し、severity 判定のブレを減らす。

**内容**:
- P1/P2/P3 の判定基準を `knowledge/conventions/review-severity-criteria.md` に明文化
- reviewer briefing に自動注入する few-shot examples セット（good findings / false positive examples）
- severity 判定の一貫性を測定する回帰テスト（同一コードに対する複数回レビューの variance 測定）

**Fowler の分類**: Inferential Sensor のキャリブレーション。
Computational Sensor（linter, test）は決定論的だが、Inferential Sensor（reviewer）は
確率的であり、キャリブレーションなしでは judgment drift が発生する。

### GH-01/02: GitHub Native Harness（v5 新規 — 2026-04-05 調査由来）

> 調査: `knowledge/research/2026-04-05-harness-engineering-startup-analysis.md`
> 設計メモ: `knowledge/research/2026-04-05-github-native-harness-design.md`

**課題**: SoTOHE-core は「内部オーケストレーションは強いが、外部観測面が薄い」。
`metadata.json` の状態が GitHub Issue / Label / Project に同期されていない。

**GH-01: Issue Intake + State Projection**（Phase A + B）

- Issue 作成 → `track/items/<id>/` artifact を自動生成
- `metadata.json` の phase / review state を GitHub label に投影
- `github.json` で operational state を分離（`review.json` パターン踏襲）
- SSoT は repo 内 artifact のまま。GitHub は projection のみ

**GH-02: Label-driven Handoff + Harness Scorecard**（Phase C + D）

- `phase:*` / `control:*` label による次工程トリガー
- Harness Scorecard: workflow success rate, review rounds per track, human rescue rate 等の定点観測
- 着手前提: `STRAT-08`, `GAP-02 PR State Machine` の完了後

---

## Phase 6: Harness Template 展開（v4 + v5 統合）

> **前提**: Phase 4 の SPLIT-03/04/05 完了後（sotp が独立配布可能な状態）
>
> **v4 由来**: テンプレート外枠（sotp init / scaffold）
> **v5 追加**: Fowler「Harness Template パターン」（3-5 トポロジーで 80% カバー）

**目標**: テンプレート利用者向けのプロジェクト生成・カスタマイズ機能を提供し、
複数のサービストポロジーに対応する Harness Template セットを構築する。

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 6-1 | **sotp init**: 新規プロジェクト生成 | M | fork/clone → ジェネレータモデルへ (v4) |
| 6-2 | **sotp scaffold**: レイヤー/モジュール追加 | M | `/architecture-customizer` の CLI 化 (v4) |
| 6-3 | **テンプレートリポ分割** | M | sotp ソースをテンプレートリポから完全除去 (v4) |
| 6-4 | **sotp upgrade**: テンプレートの sotp バージョン更新 | S | バージョン互換性管理 (v4) |
| 6-5 | **TMPL-01: トポロジー別 Harness Template セット** | L | Fowler 推奨: 3-5 トポロジーで 80% カバー (v5) |
| 6-6 | **STRAT-11**: 多言語プロジェクト対応 | L | Rust 以外への展開 (v4) |

### TMPL-01: トポロジー別 Harness Template セット（v5 新規 — 業界調査由来）

> Fowler: 「組織の 80% をカバーする 3-5 個のサービストポロジーごとに
> ハーネステンプレートを用意する」

**候補トポロジー**（要検討）:

| # | トポロジー | 特徴 | guides / sensors の差分 |
|---|---|---|---|
| 1 | **CLI ツール** | 現在の SoTOHE-core 自体 | 既存ハーネスがそのまま使える |
| 2 | **Web API (REST/gRPC)** | async runtime, DB, HTTP | API contract test, schema drift 検出 |
| 3 | **Event-driven service** | message queue, eventual consistency | saga テスト, idempotency 検証 |
| 4 | **Library crate** | pub API stability, semver | public API diff, breaking change 検出 |

各トポロジーに対して `sotp init --topology <name>` で適切な guides + sensors セットが
scaffold される。

### sotp init の設計方向

```
$ sotp init my-project
? 言語: [Rust] / TypeScript / Go
? トポロジー: [CLI] / Web API / Event-driven / Library
? アーキテクチャ: [Hexagonal] / Layered / Clean
? レイヤー名: [libs/domain, libs/usecase, libs/infrastructure, apps/cli]
? 非同期ランタイム: [なし] / Tokio / async-std
? CI プロバイダ: [GitHub Actions] / GitLab CI
→ my-project/ にテンプレート生成
→ sotp バージョン互換性チェック
→ track/tech-stack.md の TODO: を自動充填
→ トポロジー別 guides/sensors を自動配置
```

---

## Phase 間の依存関係

```
Phase 0 (✅) + Phase 1 (✅)
    ↓
Phase 1.5 (▶ sotp 品質改善 + 論理分離)
    ↓ domain 型化完了、CLI が薄くなる、sotp/template 境界が明確
Phase 2 (✅ 仕様品質 — テスト生成の入力品質)
    ↓ 信号機 + Domain States + トレーサビリティが整う
Phase 2b (✅ ヒアリング UX — Phase 3 の spec 入力品質向上)
    ↓ 構造化質問 + モード選択で spec の精度向上
Phase 2c (▶ domain-types.json 分離 — 型カテゴリ別信号 + BRIDGE-01 連携)
    ↓ 5 カテゴリ型信号 + SchemaExport 連携が整う
Phase 3 (Behaviour Harness = sotp テスト生成) ← Moat（BRIDGE-01 完了済み）
    ↓ sotp が「spec → テスト → 実装」を提供
Phase 4 (インフラ + sotp 配布 + Drift Detection)
    ↓ sotp が独立配布可能に + 継続的品質監視
Phase 5 (ワークフロー + Calibration + GitHub 観測面)  ← Phase 4 と並行可能
Phase 6 (Harness Template 展開)  ← Phase 4 完了後
```

### v3 → v5 の依存関係変更

```
v3: Phase 1.5 → Phase 2 → Phase 3 → Phase 4 || Phase 5
v5: Phase 1.5 → Phase 2 → Phase 3 → Phase 4(+配布+drift) → Phase 6(template展開)
                                        ↕ 並行
                                      Phase 5(+calibration)
```

**Phase 6 が新設**されたが、Phase 3（Moat）までの流れは変わらない。
sotp の物理分割は Phase 4 に入り、Harness Template 展開は Phase 6。
つまり **Moat 到達までのクリティカルパスに影響しない**。

**Phase 1.5 はハーネスの保守、Phase 3 が Behaviour Harness の価値。両者は別の目的。**

---

## 見積もり

| Phase | 項目 | 残 | 推定日数 | v3 差分 |
|---|---|---|---|---|
| 1.5 | 31 | 13 (+1) | 4 日 | +0.5 日（SPLIT-01/02） |
| 2 | 7 | 0 | — | — |
| 2b | 3 | 0 | — | — |
| 2c | 5 (NEW) | 5 | 2 日 | +2 日（domain-types.json 分離） |
| 3 | 13 | 12 | 5 日 | -0.5 日（BRIDGE-01 完了） |
| 4 | 13 (+5) | 10 (+5) | 4 日 | +1.5 日（SPLIT + DRIFT） |
| 5 | 10 (+3) | 10 (+3) | 5 日 | +2 日（CALIB-01 + GH-01/02） |
| 6 | 6 (NEW) | 6 | 5 日 | +5 日（新 Phase） |
| **合計** | **88** | **56** | **~25 日** | **v3 比 +10.5 日** |

### クリティカルパス（Moat 到達まで）

| | v3 | v5 |
|---|---|---|
| Phase 1.5 → 3 完了 | ~12 日 | ~14 日（+0.5 SPLIT + 2 domain-types - 0.5 BRIDGE-01 完了） |
| 全体完了 | ~14.5 日 | ~25 日（+10.5 日: Phase 2c/6 新設 + Phase 4/5 拡張） |

**Moat 到達への影響は +2 日**（Phase 2c の domain-types.json 分離分）。
ただし Phase 2c は Phase 3 の型コンテキスト品質を大幅に向上させるため、ROI は高い。

---

## v5 採用時に更新が必要なファイル

| ファイル | 変更内容 |
|---------|---------|
| `knowledge/strategy/vision.md` | sotp 独立ツール化 + Harness Template 展開の方針を反映 |
| `knowledge/strategy/TODO-PLAN.md` | このドラフトを正式版に |
| `knowledge/strategy/progress-tracker.md` | SPLIT-01/02 を Phase 1.5 に追加、Phase 6 追加 |
| `knowledge/strategy/TODO.md` | SPLIT-01〜05, DRIFT-01/02, CALIB-01, TMPL-01 を追加 |
| `README.md` | sotp の位置づけ説明 |
| `CLAUDE.md` | sotp/template 分離の概要参照 |
| `DEVELOPER_AI_WORKFLOW.md` | sotp インストール手順（Phase 4 以降） |

> 更新: 2026-04-07。v4 ドラ���ト（2026-03-23, sotp スタンドアロン化）と
> ハーネスエンジニアリング業界調査（2026-04-07）を統��して v5 として新規作成。
>
> 最近の完了トラック反映:
> - **review-system-v2** (2026-04-05): frozen scope 廃止、スコープ独立型レビュー。review.json v2 codec。
> - **rv2-docs-skill-update** (2026-04-06): review-fix-lead エージェント定義、v2 運用文書整備。
> - **bridge01-export-schema** (2026-04-06): BRIDGE-01 完了。rustdoc JSON ベース（syn 方式から転換）。Phase 3 の起点。
> - **planner-claude-migration** (2026-04-07): planner capability を Codex → Claude Opus に移行（Phase 1）。
> - **spec-domain-types-v2** (2026-04-07, ▶ 進行中): domain-types.json 分離。Phase 2c として新設。
>
> 統合した調査資料:
> - `knowledge/research/2026-04-07-1234-harness-engineering-landscape.md` (Fowler Taxonomy, Anthropic Three-Agent)
> - `knowledge/research/2026-04-05-harness-engineering-startup-analysis.md` (外部観測面の課題分析)
> - `knowledge/research/2026-04-05-github-native-harness-design.md` (GitHub Native Harness 設計メモ)
> - `knowledge/adr/2026-04-07-0045-domain-types-separation.md` (2つの信号機 + 型カテゴリ)
> - `knowledge/adr/2026-03-23-2120-two-stage-signal-architecture.md` (2段階ゲート)
> - `knowledge/adr/2026-03-23-1010-three-level-signals.md` (Blue/Yellow/Red 3値)
> - `knowledge/adr/2026-03-24-0900-coverage-not-a-signal.md` (coverage は CI ゲート)
