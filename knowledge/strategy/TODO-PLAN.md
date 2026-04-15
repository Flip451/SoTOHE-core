# SoTOHE-core 全体計画 v6

> **作成日**: 2026-04-13 (採用: 2026-04-15)
> **前版**:
> - v5 draft: `knowledge/strategy/TODO-PLAN-v5-draft.md` (2026-04-07, 未採用、併存継続)
> - v4 draft: `knowledge/strategy/TODO-PLAN-v4-draft.md` (2026-03-23, 未採用、併存継続)
> - v3 正式版: `tmp/archive-2026-04-13/TODO-PLAN-v3.md` (2026-03-22)
> **ビジョン**: [`vision.md`](vision.md) (v6 採用済)
> **ステータス**: 採用版 (2026-04-15 に v3 から昇格)
> **変更理由**:
> 1. **v5 draft の全要素を継承** (sotp スタンドアロン化 + Fowler Taxonomy + Drift Detection + Reviewer Calibration + GitHub Native Harness + Harness Template 展開)
> 2. **TDDD 3 ステップを Phase 2c に統合** (TDDD-01/02/03 = ADR `2026-04-11-0001/0002/0003` の実装)
> 3. **Phase 2d 新設**: TDDD-04 = `spec_source` 必須化 (SoT Chainの 2 番目のリンク)
> 4. **Phase 1.5 拡張**: ハーネス TDDD + typestate-first リファクタリング (HARN-TDDD-01〜N)
> 5. **Phase 3 拡張**: テスト生成の第 4 手法「カタログ駆動テスト」を追加
> 6. **SoTOHE 原点回帰**: vision v6 で「Source of Truth Oriented」を一次名称として宣言 (初版の「Single Source of Truth Oriented」から「Single」を外し、複数 SoT の独立共存を許容。TODO-PLAN への影響は命名文書のみ)

---

## 戦略サマリー v6

**v2**: 基盤を固め → 仕様品質を保証し → テスト生成パイプラインを構築する。
**v3**: 上記に加え、SoTOHE-core 自身のコードとテンプレート出力を区別する。
**v4 draft**: sotp CLI を独立ツールとして物理分離。
**v5 draft**: v4 + ハーネスエンジニアリング業界調査 (Fowler / Anthropic) を踏まえたギャップ補完。
**v6**: v5 + **SoT Chain + TDDD multilayer + ハーネス TDDD + SoTOHE 原点回帰**。**Moat = SoT Chain**。

### Moat の進化

| 版 | Moat | 構成要素 |
|---|---|---|
| v2 | 仕様品質 | spec 信号機 |
| v3 | テスト生成パイプライン | 型 + spec + テストスケルトン |
| v4 draft | sotp スタンドアロン化 | CLI 物理分離 |
| v5 draft | Behaviour Harness (Fowler) | 2 信号機 + Phase 3 テスト生成 |
| **v6** | **SoT Chain** | **4 層 SoT + 一方向参照 + TDDD + Stage 1/2 信号機** を 1 つの名前に統合 |

v5 までの Moat は個別の機能・概念の集合だったが、v6 で **統合された一つの名前 = SoT Chain** が生まれた。これが v6 の決定的な進化である。

### SoT Chain を実装する Phase 群

v5 では Phase 3 (テスト生成パイプライン) 単独が Moat だったが、v6 では以下の Phase 群が **1 つの Moat = SoT Chain** を構成する:

| Phase | SoT Chain における役割 |
|---|---|
| Phase 1.5 (HARN-TDDD-00〜05) | ハーネス自身を SoT Chain 規律下に置く |
| Phase 2 (Stage 1 信号機) | SoT Chain の 1 番目のリンク (spec → ADR) |
| Phase 2c (TDDD-01/02/03) | SoT Chain の 3 番目のリンク (型カタログ → 実装) |
| Phase 2d (TDDD-04) | SoT Chain の 2 番目のリンク (型カタログ → spec) |
| Phase 3 (Behaviour Harness) | SoT Chain 上でテスト生成 4 手法を提供 |

Phase 3 は v6 で「SoT Chain をテストに変換する層」として再位置付けされる。

v6 の 4 つの戦略柱:

1. **SoT Chainによる SoT 統一** (v6 新規):
   ADR ← spec ← 型カタログ ← 実装 の一方向参照チェーンを CI で強制し、仕様と実装のドリフトを構造的に防止する。
2. **TDDD multilayer + シグネチャ検証** (v6 新規・v5 の Phase 2c を拡張):
   任意層に `<layer>-types.json` + シグネチャ検証 + Baseline + action を適用。primitive obsession を機械的に検出。
3. **ハーネス TDDD + typestate-first** (v6 新方針):
   v3/v5 の「ハーネスは typestate 不要」方針を反転。新規コードは first、既存は段階リファクタリング。
4. **v5 の全要素を継承** (sotp 独立化 / Fowler 対応 / Drift Detection / Reviewer Calibration / GitHub Native Harness / Harness Template 展開):
   v5 は採用されなかったが、内容は v6 で全面的に継承する。

### Fowler Taxonomy との対応 (v5 継承 + v6 拡張)

```
Fowler: Guides (Feedforward)        → .claude/rules/, conventions, architecture-rules.json
Fowler: Sensors (Feedback)          → CI gates (computational) + Codex reviewer (inferential)
Fowler: Maintainability Harness     → clippy, fmt, deny, check-layers, usecase-purity, domain-purity ← 成熟
Fowler: Architecture Fitness        → architecture-rules.json + check-layers + TDDD multilayer ← 成熟
Fowler: Behaviour Harness           → 4 層 SoT + 一方向参照 + TDDD + Phase 3 テスト生成 ← v6 で発展
Fowler: Harnessability              → 04-coding-principles.md + TDDD + typestate-first ← v6 で統合
Fowler: Harness Template            → Phase 6 で展開予定 (トポロジー別)
Fowler: Continuous Monitoring       → Phase 4 の DRIFT-01/02 (arch-drift / staleness)
Anthropic: Plan/Generate/Evaluate   → planner (Claude Opus) / implementer / reviewer (Codex) 分離
Anthropic: Evaluation Calibration   → Phase 5 の CALIB-01
```

### Phase 概要

```
Phase 0 (✅)   基盤: shell wrapper Rust 化
Phase 1 (✅)   クイックウィン: 事故予防 + spec テンプレート基盤
Phase 1.5 (▶)  sotp CLI 品質改善 + 論理分離 + ハーネス TDDD リファクタリング
Phase 2 (✅)   仕様品質: Stage 1 信号機 + トレーサビリティ + spec.json SSoT
Phase 2b (✅)  ヒアリング UX 改善: 構造化質問 + モード選択 + プロセス記録
Phase 2c (▶)   型契約信号機: Stage 2 + TDDD multilayer + シグネチャ検証 + baseline + action
Phase 2d        SoT Chain: TDDD-04 = spec_source 必須化 ← v6 新規
Phase 3        Behaviour Harness: sotp テスト生成サブコマンド 4 手法 ← Moat (BRIDGE-01 完了済み)
Phase 4        インフラ + sotp 配布 + Drift Detection
Phase 5        ワークフロー最適化 + Reviewer Calibration + GitHub 外部観測面
Phase 6        Harness Template 展開: テンプレート外枠 + 複数トポロジー対応 + 多言語
```

### v5 → v6 の変更点

| 項目 | v5 draft | v6 |
|---|---|---|
| ビジョン名称 | 言及なし | **Source of Truth Oriented に原点回帰** (初版の「Single」を外し複数 SoT 許容) |
| 信号機 | 2 つ (仕様書 + 型設計書) | **SoT Chain** に発展 |
| Phase 2c 内容 | domain-types.json 分離 (5 カテゴリ) | **+ TDDD-01/02/03/04 (multilayer + シグネチャ検証 + baseline + action) + tddd-02 usecase 取り込み + taxonomy 拡張 (5 → 12 variants)** |
| Phase 2d | なし | **新設: TDDD-04 = `spec_source` 必須化** |
| Phase 1.5 | SPLIT-01/02 + コード品質改善 | **+ HARN-TDDD-01〜N (ハーネス TDDD リファクタリング)** |
| Phase 3 | テスト生成 3 手法 | **+ 第 4 手法「カタログ駆動テスト」** |
| Stage 2 信号 | Blue/Red 2 値 (Yellow 廃止) | **Blue/Yellow/Red 3 値** (Yellow 復活: WIP 許容) |
| 型カタログファイル | `domain-types.json` (単一層) | `<layer>-types.json` (多層) |
| ハーネス自身の typestate 適用 | 「不要」 (v3 踏襲) | **新規 first + 既存段階リファクタリング** |

---

## Phase 0: ✅ 完了

| # | 項目 | 状態 |
|---|---|---|
| 0-1 | **STRAT-09** shell wrapper の Rust CLI 集約 | ✅ done (PR #30) |

---

## Phase 1: ✅ 完了 (10/10)

詳細は `tmp/archive-2026-03-20/TODO-PLAN-2026-03-17.md` を参照。

---

## Phase 1.5: sotp CLI 品質改善 + 論理分離 + ハーネス TDDD リファクタリング (▶ 進行中)

> **詳細計画**: [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md)
> **Review 改善計画**: [`knowledge/strategy/rvw-remediation-plan.md`](../../knowledge/strategy/rvw-remediation-plan.md)

**目標**: CLI 肥大化解消 + domain 型化 + sotp/テンプレート論理境界 + **ハーネス自身の TDDD + typestate-first リファクタリング**。

**v6 の方針変更**: v3/v5 の「Phase 1.5 では typestate / TDDD 不要」方針を反転。v6 では HARN-TDDD-01〜N track で段階的にハーネス自身を TDDD + typestate-first 化する。

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
| 1.5-17 | ~~WF-55 view-freshness CI~~ | S | ✅ done (PR #46) |
| 1.5-18 | capability 追加: domain_modeler, spec_reviewer, acceptance_reviewer | S | 未着手 |
| 1.5-19 | ~~INF-15: usecase-purity CI~~ | S | ✅ done |
| 1.5-20 | ~~INF-16: pr_review.rs hexagonal~~ | S | ✅ done (PR #51) |
| 1.5-21 | ~~INF-17: usecase-purity error 昇格~~ | S | ✅ done (PR #52) |
| 1.5-22 | INF-18: verify ルール定義の外部設定化 | S | 未着手 |
| 1.5-23 | ~~INF-19: domain-purity CI~~ | S | ✅ done (PR #53) |
| 1.5-24 | ~~INF-20: conch-parser を infrastructure に移動~~ | M | ✅ done (PR #54) |
| 1.5-25 | ~~RVW-03: review.json 分離~~ | M | ✅ done |
| 1.5-26 | ~~RVW-10/11: verdict auto-record + diff scope filtering~~ | M | ✅ done (PR #63) |
| 1.5-27 | ~~RVW-13/15/17: review infra quality hardening~~ | M | ✅ done (PR #64) |
| 1.5-28 | ~~WF-59: review-scope manifest hash~~ | M | ✅ done |
| 1.5-29 | **WF-43**: verdict 改ざん防止 | L | archived (review system v2 で superseded) |
| 1.5-30 | **SPLIT-01: sotp / テンプレートの論理分離** | M | NEW (v4 継承) |
| 1.5-31 | **SPLIT-02: bin/sotp パス抽象化** | S | NEW (v4 継承) |
| **1.5-32** | **HARN-TDDD-00: ハーネス TDDD リファクタリング方針策定 track** | **S** | **NEW (v6)** |
| **1.5-33** | **HARN-TDDD-01: review モジュール TDDD + typestate-first リファクタリング** | **L** | **NEW (v6)** |
| **1.5-34** | **HARN-TDDD-02: pr モジュール TDDD + typestate-first リファクタリング** | **L** | **NEW (v6)** |
| **1.5-35** | **HARN-TDDD-03: track モジュール TDDD + typestate-first リファクタリング** | **L** | **NEW (v6)** |
| **1.5-36** | **HARN-TDDD-04: verdict 系モジュール TDDD + typestate-first リファクタリング** | **M** | **NEW (v6)** |
| **1.5-37** | **HARN-TDDD-05: value object 系モジュール TDDD リファクタリング** (id 型 / hash 型 / timestamp 型) | **M** | **NEW (v6)** |

### SPLIT-01: sotp / テンプレートの論理分離 (v4/v5 継承)

**目標**: 同一リポ内で sotp CLI のコードとテンプレートスケルトンの境界を明確にする。

**内容**:
- README / CLAUDE.md に「sotp = スタンドアロン CLI ツール」「テンプレート = sotp を使うプロジェクト基盤」の区別を明記
- Cargo workspace 内で sotp グループと テンプレートグループの論理境界を文書化
- `architecture-rules.json` に sotp/template 境界を反映 (将来の物理分割の準備)
- vision v6 / README v6 を作成し昇格

**やらないこと (Phase 4 に送る)**:
- 物理的なリポ分割
- sotp のバイナリ配布
- Dockerfile の sotp インストール化

### SPLIT-02: bin/sotp パス抽象化 (v4/v5 継承)

**目標**: `bin/sotp` のハードコード参照を抽象化し、将来の PATH ベースインストールに備える。

**内容**:
- `Makefile.toml` の `SOTP_BIN` 変数を導入: デフォルト `bin/sotp`、環境変数 `SOTP_BIN` でオーバーライド可能
- hooks / scripts 内の `bin/sotp` 参照を `SOTP_BIN` 変数経由に変更
- `sotp` が PATH 上にある場合は `bin/sotp` より優先するロジック

### HARN-TDDD-00: ハーネス TDDD リファクタリング方針策定 track (v6 新規)

**目標**: ハーネス自身に TDDD + typestate-first を適用する際の原則・優先度・判断基準を文書化する。

**内容**:
- `knowledge/conventions/harness-tddd-refactoring.md` を新設
- 優先度判定の基準 (状態遷移明確度 / value object 密度 / 純粋データ型の割合)
- 段階移行のパターン集 (新規コード / 既存モジュール / 部分リファクタリング)
- リファクタリング対象外の判断基準 (テンプレート向けサンプル / 一時的なプロトタイプ / archive 対象)
- 成功判定基準 (対象モジュールの `<layer>-types.json` が Blue のみ + 既存テストの green 維持)

**成果物**:
- 新しい conventions ドキュメント
- HARN-TDDD-01〜05 の優先度付きリスト
- vision v6 §6 の参照先 (既存 trait ベース DI 維持方針との両立を明示)

### HARN-TDDD-01〜05: 各モジュールの TDDD リファクタリング (v6 新規)

**共通の進め方** (HARN-TDDD-00 で確定):

1. `/track:plan` でモジュールの現状分析 (既存型の洗い出し)
2. `/track:design` で `<layer>-types.json` に TDDD カタログを宣言 (spec_source 付き)
3. `baseline-capture` で既存型のスナップショット
4. typestate 化 + enum-first 化のリファクタリング
5. `<layer>-types.json` が all-Blue になるまでループ
6. 既存テストの green 維持を確認
7. `/track:review` + `/track:ci` + `/track:commit`

**対象モジュール** (HARN-TDDD-00 の優先度付け結果を反映):

- **HARN-TDDD-01 review**: `ReviewGroupState`, `GroupRoundVerdict`, `ReviewConcern`, `EscalationPhase` 等。既に enum-first 化されているが、シグネチャ検証 + typestate 化の余地あり
- **HARN-TDDD-02 pr**: `PrState`, `PrCheckState`, `PrMergeMethod` 等。状態遷移が明確
- **HARN-TDDD-03 track**: `TrackStatus`, `TaskStatus`, track state machine 系。典型的な typestate 対象
- **HARN-TDDD-04 verdict 系**: `Verdict`, `CodeHash`, `ReviewScope` 等。enum-first 適用済み
- **HARN-TDDD-05 value object 系**: `TrackId`, `TaskId`, `CommitHash`, タイムスタンプ型等。Newtype + バリデーション

---

## Phase 2: ✅ 完了 (+ Phase 2c 進行中)

**目標**: Stage 1 信号機 + トレーサビリティの最小セットを導入。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 2-1 | **TSUMIKI-01** Spec 信号機評価 (Stage 1) + Domain States 存在チェック | M | ✅ done |
| 2-1b | **spec.json SSoT 化** — spec.md を rendered view に降格 | M | ✅ done (PR #57) |
| 2-2 | **SPEC-05** Domain States 信号機 (Stage 2 初期) + 遷移関数検証 | M | ✅ done (PR #58) |
| 2-3 | ~~**CC-SDD-01** 要件-タスク双方向トレーサビリティ~~ | M | ✅ done (PR #60) |
| 2-4 | ~~**CC-SDD-02** 明示的承認ゲート~~ | S | ✅ done (PR #62) |
| 2-5 | ~~**TSUMIKI-03** 差分ヒアリング~~ | S | ✅ done |
| ~~2-6~~ | ~~**SSoT-07** 二重書き込み解消~~ | — | スキップ (spec.json SSoT で解決済み) |
| ~~2-7~~ | ~~spec.md Domain States 必須化~~ | — | 2-1 に統合、2-2 で完全実装 |

---

## Phase 2b: ✅ 完了 (ヒアリング UX 改善)

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 2b-1 | ~~**TSUMIKI-05** 構造化ヒアリング UX~~ | S | ✅ done |
| 2b-2 | ~~**TSUMIKI-06** ヒアリング作業規模選定~~ | S | ✅ done |
| 2b-3 | ~~**TSUMIKI-07** ヒアリング記録~~ | S | ✅ done |

---

## Phase 2c: 型契約信号機 — multilayer + シグネチャ検証 + baseline + action (▶ 進行中)

> **v5 → v6 の変更**: v5 では `domain-types.json` 分離 (5 カテゴリ) のみだったが、v6 では TDDD-01/02/03 を統合して multilayer + シグネチャ検証 + baseline + action まで拡張し、さらに **tddd-02 (usecase-wiring, ADR `2026-04-13-1813`)** で taxonomy を **5 → 12 variants** に拡張する (SecondaryPort 旧 TraitPort + ApplicationService / UseCase / Interactor / Dto / Command / Query / Factory を追加)。

**目標**: 任意層に TDDD カタログ + シグネチャ検証 + Baseline 4 グループ評価 + action 宣言を適用。primitive obsession を機械的に検出し、既存型ノイズを排除し、型削除と TDDD を併用可能にする。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| 2c-1 | ~~`DomainTypeKind` enum + `DomainTypeEntry` + `DomainTypeSignal` + `DomainTypesDocument` 型定義~~ | M | ✅ done (`spec-domain-types-v2-2026-04-07`) |
| 2c-2 | ~~`evaluate_domain_type_signals()` + `SpecDocument` から `domain_states` 削除~~ | M | ✅ done |
| 2c-3 | ~~`domain-types.json` codec + `domain-types.md` renderer + verify 切替~~ | M | ✅ done |
| 2c-4 | ~~CLI: `domain-type-signals` コマンド + views sync + マイグレーション~~ | M | ✅ done |
| 2c-5 | ~~`DESIGN.md` + ADR 更新~~ | S | ✅ done |
| 2c-6 | ~~**reverse signal 導入**: `check_consistency` + `DomainTypeSignal` (Red) への変換~~ | M | ✅ done (ADR `2026-04-08-1800`) |
| 2c-7 | ~~**TDDD-02: baseline reverse signals + 4 グループ評価**~~ | M | ✅ done (ADR `2026-04-11-0001`) |
| **2c-8** | **TDDD-01: multilayer + シグネチャ検証 + `MethodDeclaration` + リネーム** | **L** | **▶ 進行中** (`track/tddd-01-multilayer-2026-04-12`, T001-T003 完了, T004-T007 進行中) |
| **2c-9** | **TDDD-03: action 宣言 (add/modify/reference/delete)** | **S** | 未着手 (ADR `2026-04-11-0003`, Proposed) |

### 2 つの信号機アーキテクチャ (v5 継承 + v6 拡張)

v5 の ADR `2026-04-07-0045` + `2026-03-23-2120-two-stage-signal-architecture.md` で確定した 2 信号機をそのまま継承し、v6 で以下を拡張する:

**v6 拡張**:
- Stage 2 で Yellow を復活 (TDDD-02 の 4 グループ評価で WIP 許容が必要)
- ファイル名: `domain-types.json` → `<layer>-types.json` (multilayer 対応)
- 入力: `CodeScanResult + Optional SchemaExport` → `TypeGraph (from rustdoc JSON) + TypeBaseline + spec.json`
- 12 variants の `TypeDefinitionKind` (旧 `DomainTypeKind`, ADR `2026-04-13-1813` で 5 → 12 に拡張: SecondaryPort (旧 TraitPort) + ApplicationService / UseCase / Interactor / Dto / Command / Query / Factory)
- `expected_methods: Vec<String>` → `Vec<MethodDeclaration>` (シグネチャ検証)
- `action` フィールド (add/modify/reference/delete)

```
┌─────────────────────────────────────────────────────────────────┐
│  仕様書の信号機 (Stage 1)                                       │
│  ファイル: spec.json / spec.md                                   │
│  評価対象: 要件の出典 (source tags)                              │
│  信号: Blue / Yellow / Red (3値)                                 │
│  ゲート: red == 0 で通過                                         │
└─────────────────────────────────────────────────────────────────┘
        ↓ Stage 1 通過が前提条件
┌─────────────────────────────────────────────────────────────────┐
│  型契約の信号機 (Stage 2)  ← v6 で multilayer + シグネチャ検証拡張          │
│  ファイル: <layer>-types.json / <layer>-types.md                 │
│  評価対象: 型宣言と実装の一致度 (forward + reverse + シグネチャ検証)         │
│  信号: Blue / Yellow / Red (v6 で Yellow 復活)                   │
│  ゲート: red == 0 で通過 (merge 時は yellow もブロック)          │
│  入力: TypeGraph + TypeBaseline + spec.json                      │
│  12 variants: TypeDefinitionKind (domain + application 層,      │
│               ADR 2026-04-13-1813 で 5 → 12 に拡張, enum-first) │
│  v6 拡張: multilayer / シグネチャ検証 / baseline / action / spec_source      │
└─────────────────────────────────────────────────────────────────┘
```

### TDDD-01: multilayer + シグネチャ検証 (ADR `2026-04-11-0002`)

**内容**:
- `DomainTypeKind` → `TypeDefinitionKind` / `DomainTypeEntry` → `TypeCatalogueEntry` / `DomainTypesDocument` → `TypeCatalogueDocument` のリネーム
- `MethodDeclaration` / `ParamDeclaration` / `MemberDeclaration` 型を domain 層に追加
- `expected_methods: Vec<String>` → `Vec<MethodDeclaration>` に拡張
- `TypeGraph` の拡張 (`TypeNode::methods`, `TraitNode::methods` を `Vec<MethodDeclaration>` 化)
- `architecture-rules.json` に `layers[].tddd` ブロック追加
- `sotp track type-signals --layer <id>` + `baseline-capture --layer <id>` の multilayer 対応
- `verify spec-states` を全層 AND 集約に拡張
- `/track:design` を多層 loop に対応

**現在の状態**: `track/tddd-01-multilayer-2026-04-12` で Phase 1 実装中。T001-T003 (リネーム + 3 分割) 完了、T004-T007 (TypeGraph 拡張 + シグネチャ検証 + multilayer wiring) 進行中。

### TDDD-03: action 宣言 (ADR `2026-04-11-0003`)

**内容**:
- `DomainTypeEntry` (= `TypeCatalogueEntry`) に optional な `action` フィールド追加
- `action`: `"add"` (default) / `"modify"` / `"reference"` / `"delete"`
- forward check に `action` 別ロジック追加
  - `"delete"`: C に **存在しない** → Blue、存在する → Yellow
  - その他: 従来どおり C に存在し宣言と一致 → Blue
- `action` と baseline の矛盾検出 (警告): 例 `"add"` + baseline に既存 → 警告
- `action: "delete"` の baseline 存在検証 (存在しない型への delete 宣言はエラー)
- `/track:design` で `action` を選択できるよう UX 更新

**依存**: TDDD-01 のリネーム完了後 (`TypeCatalogueEntry` に対して `action` を追加)。

---

## Phase 2d: SoT Chain の完成 — TDDD-04 = spec_source 必須化 (v6 新規)

> **v6 新規**: 型カタログと spec の紐付けを必須化し、SoT Chainの 2 番目のリンク (カタログ → spec) を構造的に強制する。

**目標**: `<layer>-types.json` の全エントリに `spec_source` フィールドを必須化し、カタログと spec.json の双方向整合性を CI で検証する。

| # | 項目 | 難易度 | 状態 |
|---|---|---|---|
| **2d-1** | **TDDD-04 設計 ADR 作成**: spec_source の粒度 (section header / spec.json element id) + hash 戦略 (section 単位 / 全体) + tamper 検証ロジック | M | 未着手 |
| **2d-2** | **`SpecSource` 型定義** (domain 層): `TypeCatalogueEntry` に必須フィールド追加 | S | 未着手 |
| **2d-3** | **catalogue_codec 拡張**: JSON schema_version bump + migration エラーメッセージ | S | 未着手 |
| **2d-4** | **双方向検証**: forward (orphan spec detection) + reverse (dangling spec_source detection) + tamper (spec_hash mismatch detection) | M | 未着手 |
| **2d-5** | **`/track:design` UX 拡張**: カタログエントリ作成時に spec セクションを必須選択、hash 自動計算 | M | 未着手 |
| **2d-6** | **既存トラックのマイグレーション方針**: 完了済みトラックは対象外、in-progress トラックは spec_source 手動入力を要求 | S | 未着手 |
| **2d-7** | **Stage 2 信号機統合**: spec_source 関連検証を Red シグナルとして Stage 2 に組み込む | S | 未着手 |

### TDDD-04 設計の論点

**spec_source の粒度候補**:

| 候補 | 粒度 | pros | cons |
|---|---|---|---|
| A | section header のフルパス (例: `## Domain States > UserRepository`) | 人間に読みやすい、markdown レンダリング互換 | section rename で壊れる |
| B | spec.json の element id (例: `domain_states.user_repository`) | JSON SSoT と整合、rename 安全 | 人間に読みにくい |
| C | A + B のハイブリッド | 両方の利点 | 保守コスト 2 倍 |

**hash 戦略候補**:

| 候補 | 粒度 | 意味 |
|---|---|---|
| 1 | spec.json 全体の content hash | 全体の変更を検知、カタログ全エントリが同時 stale になる |
| 2 | spec section 単位の hash | 該当セクションだけ stale にできる、粒度が細かい |
| 3 | section 内の特定要素 (name, description, examples) の hash | 該当要素の変更だけ stale にできる、最小影響 |

**現時点の推奨**: B (element id) + 2 (section 単位 hash)。ADR で最終確定する。

### SoT Chainが完結する時点

TDDD-04 が完了すると、SoT Chainの 4 層全てが構造的に強制される:

| リンク | 実装時期 | 実装方法 |
|---|---|---|
| ADR → spec (source tag) | v5 既存 | Stage 1 信号機 SignalBasis |
| spec → 型カタログ | **v6 Phase 2d で新規** | `spec_source` 必須化 |
| 型カタログ → 実装 | v5/v6 既存 | TDDD forward/reverse check (TDDD-01/02/03) |

v5 の 2 信号機アーキテクチャは v6 で **SoT Chain**に発展する。

---

## Phase 3: Behaviour Harness — sotp テスト生成サブコマンド (SoT Chain 上のテスト生成層)

> **v5 継承 + v6 拡張**: v5 の Behaviour Harness 位置付けを継承し、v6 でテスト生成の第 4 手法「カタログ駆動テスト」を追加する。
> **Moat の位置付け変更**: v5 では Phase 3 単独が Moat だったが、v6 では **SoT Chain 全体 (Phase 1.5/2/2c/2d/3)** が Moat。Phase 3 はその上で「SoT Chain をテストに変換する層」として機能する。
> **Fowler 対応の更新**: Fowler の "Behaviour Harness" を v6 では SoT Chain として実装する (= Behaviour Harness = SoT Chain)。
>
> Fowler は「AI 生成テストへの過度な信頼は不十分」と警告しており、SoTOHE-core の **spec.json SSoT + 4 層 SoT + 一方向参照 + TDDD + Given/When/Then → テスト変換** は、
> **入力品質から保証する Behaviour Harness** として業界最大のギャップを埋める。

**目標**: sotp のサブコマンドとして、生成プロジェクトの spec → テスト自動生成パイプラインを提供する。

### テスト生成の 4 手法 (v5 の 3 手法 + v6 新規 1 手法)

| 手法 | 対象 | 入力 | 版 |
|---|---|---|---|
| spec 例 → テスト変換 | domain impl | spec の Given/When/Then | v5 継承 |
| proptest + typestate | domain impl | export-schema のシグネチャ (関数の存在 = 有効遷移) | v5 継承 |
| usecase モック自動生成 | usecase | `impl Fn` のクロージャモック | v5 継承 |
| **カタログ駆動テスト** | 任意層 | `<layer>-types.json` の `MethodDeclaration` + `spec_source` | **v6 新規** |

### Phase 3 項目

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 3-1 | ~~**BRIDGE-01** `sotp domain export-schema` (rustdoc JSON)~~ | M | ✅ done (`bridge01-export-schema-2026-04-06`) |
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
| 3-12 | **spec ↔ code 整合性チェック** | M | TDDD-01/02/03 + Phase 2d (spec_source) で部分的に実現。残りはカタログ駆動テストで補完 |
| 3-13 | **TSUMIKI-08** シグナル伝播 | M | spec.json 信号 → metadata.json タスクへ worst-case 伝播 |
| **3-14** | **カタログ駆動テスト生成**: `<layer>-types.json` の `MethodDeclaration` + `spec_source` から契約テスト + 遷移網羅テスト + enum exhaustiveness テストを自動生成 | **M** | **v6 新規** — TDDD カタログ自体をテスト生成の入力とする、宣言 = テストの哲学を具現化 |

### カタログ駆動テスト生成 (3-14) の設計方向

**入力**: `<layer>-types.json` の各 TDDD カタログエントリ
**出力**: Rust テストコード (`tests/generated_<layer>_catalogue.rs`)

**生成されるテストの種類**:

| 入力 kind | 生成テスト | 目的 |
|---|---|---|
| `Typestate` + `transitions_to` | proptest! で全遷移パスを網羅 | 不正遷移の検出 (type-level テスト) |
| `Enum` + `expected_variants` | exhaustive match test (全 variant の取り扱い確認) | variant 漏れの検出 |
| `SecondaryPort` / `ApplicationService` + `expected_methods` (シグネチャ検証) | 契約テスト (trait bound 検証) | シグネチャ逸脱の検出 |
| `ValueObject` | newtype 境界テスト (空文字, overflow, invalid UTF-8 等) | バリデーション漏れの検出 |
| `ErrorType` + `expected_variants` | error → match 変換テスト | error variant 漏れの検出 |

**`spec_source` との連携**: 各生成テストの doc comment に `spec_source` の参照を埋め込む。これにより、テスト失敗時に開発者が直接 spec セクションまで辿れる。

```rust
/// Source: spec.json > domain_states > user_repository
/// See ADR: 2026-04-11-0002-tddd-multilayer-extension.md
#[test]
fn test_user_repository_find_by_id_signature() {
    // generated from <layer>-types.json MethodDeclaration
    ...
}
```

### テンプレートが推奨する生成プロジェクトの設計 (v5 継承)

| 設計要素 | 推奨パターン | SoTOHE 自身に適用？ |
|---|---|---|
| 状態遷移 | typestate (関数の存在 = 有効遷移) | **v6 で適用** (新規 first + 既存段階移行) |
| usecase DI | `impl Fn` (モック = クロージャ) | いいえ (trait 維持) |
| ファイル分割 | DDD 概念 + pub 可視性フィルタ + `tddd/` サブモジュール | はい |
| 永続化 | domain: typestate 維持、infra: serde enum DTO に変換 | **v6 で適用** (段階移行) |

---

## Phase 4: インフラ + sotp 配布 + Drift Detection (v5 継承)

> **v5 継承**: sotp 配布関連 3 項目 (SPLIT-03/04/05) + Drift Detection 2 項目 (DRIFT-01/02)

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
| 4-9 | **SPLIT-03: sotp crate 公開準備** | M | NEW (v4/v5 継承) |
| 4-10 | **SPLIT-04: sotp バイナリ配布 (GitHub Releases)** | M | NEW (v4/v5 継承) |
| 4-11 | **SPLIT-05: Dockerfile sotp インストール化** | S | NEW (v4/v5 継承) |
| 4-12 | **DRIFT-01: アーキテクチャドリフト定期スキャン** | M | NEW (v5 継承) |
| 4-13 | **DRIFT-02: 依存性 staleness + デッドコード自動検出** | S | NEW (v5 継承) |

### SPLIT-03/04/05: sotp 配布 (v5 継承)

**SPLIT-03**: Cargo.toml を publish 可能に整備、`cargo install sotp` 対応、semver + 互換性マトリクス決定
**SPLIT-04**: GitHub Actions でクロスコンパイル + Release、`cargo-binstall` 対応、インストール手順の文書化
**SPLIT-05**: Dockerfile tools ステージに sotp バイナリインストールを追加、`bin/sotp` ビルドの代替に

### DRIFT-01: アーキテクチャドリフト定期スキャン (v5 継承)

**内容**:
- `sotp verify arch-drift` サブコマンド: `architecture-rules.json` と実際の依存グラフの乖離を検出
- `check-layers` の拡張版
- CI の定期実行 (weekly cron) または merge 後フック

### DRIFT-02: 依存性 staleness + デッドコード自動検出 (v5 継承)

**内容**:
- `cargo make machete` の定期自動実行 + レポート生成
- `cargo outdated` 相当の staleness チェック (major/minor/patch 別分類)
- 結果を `knowledge/research/drift-report-{date}.md` に出力

---

## Phase 5: ワークフロー最適化 + Reviewer Calibration + GitHub 外部観測面 (v5 継承)

| # | 項目 | 難易度 |
|---|---|---|
| 5-1 | SURVEY-06 clarify フェーズ | M |
| 5-2 | SURVEY-08 checklist | M |
| 5-3 | Session/Bootstrap/Briefing 統合 | S |
| 5-4 | SURVEY-09 Hook profile | M |
| 5-5 | HARNESS-01 PostToolUse 構造化フィードバック | S |
| 5-6 | GAP-02 PR State Machine | M |
| 5-7 | GAP-11 tracing 導入 | M |
| 5-8 | **WF-67** agent-router 廃止 + skill 遵守フック導入 | M |
| 5-9 | **CALIB-01: Reviewer Calibration 基準** | M (v5 継承) |
| 5-10 | **GH-01: GitHub Native Harness — Issue Intake + State Projection** | M (v5 継承) |
| 5-11 | **GH-02: Label-driven Handoff + Harness Scorecard** | M (v5 継承) |

### CALIB-01: Reviewer Calibration 基準 (v5 継承)

**目標**: reviewer に渡すキャリブレーション基準を決定論的に定義し、severity 判定のブレを減らす。

**内容**:
- P1/P2/P3 の判定基準を `knowledge/conventions/review-severity-criteria.md` に明文化
- reviewer briefing に自動注入する few-shot examples セット
- severity 判定の一貫性を測定する回帰テスト

### GH-01/02: GitHub Native Harness (v5 継承)

**GH-01: Issue Intake + State Projection**
- Issue 作成 → `track/items/<id>/` artifact を自動生成
- `metadata.json` の phase / review state を GitHub label に投影
- `github.json` で operational state を分離 (review.json パターン踏襲)
- SSoT は repo 内 artifact のまま、GitHub は projection のみ

**GH-02: Label-driven Handoff + Harness Scorecard**
- `phase:*` / `control:*` label による次工程トリガー
- Harness Scorecard: workflow success rate, review rounds per track, human rescue rate 等の定点観測

---

## Phase 6: Harness Template 展開 — テンプレート外枠 + 複数トポロジー対応 + 多言語 (v5 継承)

> **前提**: Phase 4 の SPLIT-03/04/05 完了後 (sotp が独立配布可能な状態)

**目標**: テンプレート利用者向けのプロジェクト生成・カスタマイズ機能 + 複数サービストポロジー対応。

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 6-1 | **sotp init**: 新規プロジェクト生成 | M | fork/clone → ジェネレータモデル (v4/v5 継承) |
| 6-2 | **sotp scaffold**: レイヤー/モジュール追加 | M | `/architecture-customizer` の CLI 化 (v4/v5 継承) |
| 6-3 | **テンプレートリポ分割** | M | sotp ソースをテンプレートリポから完全除去 (v4/v5 継承) |
| 6-4 | **sotp upgrade**: テンプレートの sotp バージョン更新 | S | バージョン互換性管理 (v4/v5 継承) |
| 6-5 | **TMPL-01: トポロジー別 Harness Template セット** | L | Fowler 推奨: 3-5 トポロジーで 80% カバー (v5 継承) |
| 6-6 | **STRAT-11**: 多言語プロジェクト対応 | L | Rust 以外への展開 (v4/v5 継承) |

### TMPL-01: トポロジー別 Harness Template セット (v5 継承)

| # | トポロジー | 特徴 | guides / sensors の差分 |
|---|---|---|---|
| 1 | **CLI ツール** | 現在の SoTOHE-core 自身 | 既存ハーネスがそのまま使える |
| 2 | **Web API (REST/gRPC)** | async runtime, DB, HTTP | API contract test, schema drift 検出 |
| 3 | **Event-driven service** | message queue, eventual consistency | saga テスト, idempotency 検証 |
| 4 | **Library crate** | pub API stability, semver | public API diff, breaking change 検出 |

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
→ TDDD 初期カタログ (<layer>-types.json) のスケルトンを生成
```

---

## Phase 間の依存関係

```
Phase 0 (✅) + Phase 1 (✅)
    ↓
Phase 1.5 (▶ sotp 品質改善 + 論理分離 + ハーネス TDDD リファクタリング)
    ↓ domain 型化完了、sotp/template 境界が明確、ハーネスが TDDD 対応
Phase 2 (✅ Stage 1 信号機 + トレーサビリティ)
    ↓ 信号機 + spec.json SSoT が整う
Phase 2b (✅ ヒアリング UX)
    ↓ 構造化質問で spec 精度向上
Phase 2c (▶ 型契約信号機 — multilayer + シグネチャ検証 + baseline + action)
    ↓ 任意層に TDDD 適用可能、primitive obsession 検出
Phase 2d (SoT Chain — spec_source 必須化)  ← v6 新規
    ↓ 型カタログ ↔ spec の双方向紐付けが CI で強制される
Phase 3 (Behaviour Harness = sotp テスト生成 4 手法) ← Moat
    ↓ sotp が「spec → カタログ → テスト → 実装」を提供
Phase 4 (インフラ + sotp 配布 + Drift Detection)
    ↓ sotp が独立配布可能に + 継続的品質監視
Phase 5 (ワークフロー + Calibration + GitHub 観測面)  ← Phase 4 と並行可能
Phase 6 (Harness Template 展開 + 多言語)  ← Phase 4 完了後
```

### v5 → v6 の依存関係変更

```
v5: Phase 1.5 → Phase 2c → Phase 3 → Phase 4(+配布+drift) → Phase 6
                                          ↕ 並行
                                        Phase 5(+calibration)
v6: Phase 1.5 (+ HARN-TDDD) → Phase 2c (+ TDDD-01/02/03) → Phase 2d (+ TDDD-04) → Phase 3 (+ カタログ駆動) → ...
```

**v6 の追加**: Phase 1.5 に HARN-TDDD 追加、Phase 2c に TDDD-01/02/03 統合、Phase 2d 新設、Phase 3 に第 4 手法追加、Phase 5 に WF-67 (agent-router 廃止 + skill 遵守フック導入、+0.5 日) 追加。**Phase 4 と Phase 6 は v5 と変わらない**。

**Moat 到達クリティカルパスへの影響**:
- v5 比で + Phase 2d (~ 2 日) + Phase 2c 拡張 (~ 2 日, TDDD-01/02/03) + Phase 3 第 4 手法 (~ 1 日) = 約 +5 日
- HARN-TDDD はクリティカルパスと並行で進行するため Moat には影響しない

---

## 見積もり

| Phase | 項目 | 残 | 推定日数 | v5 比 差分 |
|---|---|---|---|---|
| 1.5 | 37 (+6) | 19 (+6) | 6 日 | +2 日 (HARN-TDDD-00〜05) |
| 2 | 7 | 0 | — | — |
| 2b | 3 | 0 | — | — |
| 2c | 9 (+4) | 2 (+2) | 2.5 日 | +1 日 (TDDD-01/03 統合、TDDD-02 は完了済み) |
| 2d (NEW) | 7 | 7 | 3 日 | +3 日 (新規 Phase) |
| 3 | 14 (+1) | 13 (+1) | 6 日 | +1 日 (カタログ駆動テスト) |
| 4 | 13 | 10 | 4 日 | — |
| 5 | 11 (+1) | 11 (+1) | 5 日 | +0.5 日 (WF-67) |
| 6 | 6 | 6 | 5 日 | — |
| **合計** | **107** | **68** | **~31.5 日** | **v5 比 +7.5 日** |

### クリティカルパス (Moat 到達まで)

| | v3 | v5 | v6 |
|---|---|---|---|
| Phase 1.5 → 3 完了 | ~12 日 | ~14 日 | **~19 日** (+5 日: TDDD-01/03 + TDDD-04 + カタログ駆動) |
| 全体完了 | ~14.5 日 | ~25 日 | **~31.5 日** (+6.5 日: HARN-TDDD + TDDD-04 + WF-67) |

**Moat 到達への影響は +5 日**。ただし TDDD-04 (SoT Chain) による仕様と実装のドリフト防止は長期的に review loop を削減するため、ROI は高い。

HARN-TDDD は Phase 1.5 内で **クリティカルパスと並行** で進行するため、Moat 到達には影響しない。

---

## v6 採用時に更新が必要なファイル (歴史的記録)

> **注**: 本節は v6 草案作成時点 (2026-04-13) の作業計画である。v6 昇格は 2026-04-15 に完了済みで、以下の項目は履歴として保持する。

| ファイル | 変更内容 |
|---------|---------|
| `knowledge/strategy/vision.md` | v6 draft (`tmp/vision-v6-draft.md`) で置き換え |
| `knowledge/strategy/TODO-PLAN.md` | 本ドラフトで置き換え |
| `knowledge/strategy/progress-tracker.md` | Phase 2d 追加、Phase 1.5 に HARN-TDDD-00〜05 追加 |
| `knowledge/strategy/TODO.md` | HARN-TDDD, TDDD-04 の詳細項目を追加 |
| `knowledge/strategy/README.md` | Files 一覧から `TODO-PLAN-v4-draft.md` / `TODO-PLAN-v5-draft.md` を退避候補として記載 |
| `README.md` | v6 draft (`tmp/README-v6-draft.md`) で置き換え (名称を "Source of Truth Oriented Harness Engine" に変更、初版「Single」を外す) |
| `CLAUDE.md` | SoT Chain + TDDD multilayer + ハーネス TDDD リファクタリング方針の概要参照 |
| `DEVELOPER_AI_WORKFLOW.md` | Phase 2d (spec_source) 対応ワークフロー (`/track:design` 拡張) を追記 |
| `.claude/rules/` | 新規 rule として「SoT Chainの遵守」を追加検討 |
| `knowledge/adr/` | TDDD-04 (spec_source) ADR の新規作成 |

**併存を継続するファイル** (v6 採用時点の判断で `knowledge/strategy/` に残置、検討履歴として参照):
- `knowledge/strategy/TODO-PLAN-v4-draft.md` — v4 検討履歴
- `knowledge/strategy/TODO-PLAN-v5-draft.md` — v5 検討履歴

**保存するファイル** (ユーザー指示 3(b)):
- `tmp/vision-v4-2026-04-13.md` — v4 作業 draft (リファレンスとして保存)
- `tmp/README-v4-2026-04-13.md` — README v4 作業 draft (リファレンスとして保存)

---

## レビュー観点

1. **Phase 2c 拡張 (TDDD-01/02/03 統合)**: v5 の Phase 2c に TDDD を統合する構造が妥当か
2. **Phase 2d 新設 (spec_source)**: SoT Chainの 2 番目のリンクを新 Phase として切り出す粒度が正しいか
3. **Phase 1.5 の HARN-TDDD**: ハーネス TDDD リファクタリングを既存 Phase 1.5 に追加する方針が妥当か (別 Phase にしない)
4. **Phase 3 の第 4 手法**: カタログ駆動テストを Phase 3 の既存 3 手法に追加する位置付けが正しいか
5. **Moat 到達クリティカルパス**: v5 比で +5 日の影響は受容可能か (TDDD-04 の ROI との比較)
6. **HARN-TDDD の粒度**: 01-05 のモジュール分割が妥当か (もう少し細分化 / 大雑把化するか)
7. **TDDD-04 設計の論点**: spec_source の粒度 (A/B/C) + hash 戦略 (1/2/3) の候補から、どれを ADR で優先検討するか
8. **既存 Phase 2c 進行中トラック (`tddd-01-multilayer-2026-04-12`) との整合**: v6 昇格時に現行 track に影響を与えないか

---

> 作成: 2026-04-13。v5 ドラフト (2026-04-07, Fowler Taxonomy + sotp スタンドアロン化) と TDDD 展開 (ADR `2026-04-11-0001/0002/0003`) + SoT Chain + ハーネス TDDD + SoTOHE 原点回帰を統合して v6 として新規作成。
>
> 最近の完了トラック反映:
> - **review-system-v2** (2026-04-05): frozen scope 廃止、スコープ独立型レビュー
> - **rv2-docs-skill-update** (2026-04-06): review-fix-lead エージェント定義、v2 運用文書整備
> - **bridge01-export-schema** (2026-04-06): BRIDGE-01 完了。rustdoc JSON ベース (Phase 3 の起点)
> - **planner-claude-migration** (2026-04-07): planner capability を Codex → Claude Opus に移行 (Phase 1)
> - **spec-domain-types-v2** (2026-04-07): domain-types.json 分離。Phase 2c 初期実装 (完了)
> - **先行 track** (2026-04-08): reverse signal 導入 (ADR `2026-04-08-1800`)
> - **先行 track** (2026-04-11): TDDD-02 baseline 4 グループ評価 (ADR `2026-04-11-0001`)
> - **tddd-01-multilayer-2026-04-12** (2026-04-13 時点進行中): TDDD-01 multilayer + シグネチャ検証 (ADR `2026-04-11-0002`)
>
> 統合した調査資料:
> - `knowledge/research/2026-04-07-1234-harness-engineering-landscape.md` (Fowler Taxonomy, Anthropic Three-Agent)
> - `knowledge/research/2026-04-05-harness-engineering-startup-analysis.md` (外部観測面の課題分析)
> - `knowledge/research/2026-04-05-github-native-harness-design.md` (GitHub Native Harness 設計メモ)
> - `knowledge/adr/2026-04-07-0045-domain-types-separation.md` (2 信号機 + 型カテゴリ)
> - `knowledge/adr/2026-04-08-1800-reverse-signal-integration.md` (reverse signal 導入 + TDDD 命名)
> - `knowledge/adr/2026-04-11-0001-baseline-reverse-signals.md` (baseline 4 グループ評価)
> - `knowledge/adr/2026-04-11-0002-tddd-multilayer-extension.md` (multilayer + シグネチャ検証 + `MethodDeclaration`)
> - `knowledge/adr/2026-04-11-0003-type-action-declarations.md` (action 宣言)
> - 初版 README (コミット `3e817d8`): SoTOHE = Single Source of Truth Oriented Harness Engine
