# SoTOHE-core 取り込み推奨機能一覧

> **作成日**: 2026-03-17
> **出典レポート**:
> - `tmp/tsumiki-adoption-report-2026-03-17.md`
> - `tmp/cc-sdd-adoption-report-2026-03-17.md`
> - `tmp/sdd-comparison-report-2026-03-17.md`
> - `tmp/agent-harness-survey-2026-03-17.md`

---

## 凡例

- **優先度**: HIGH / MEDIUM / LOW
- **難易度**: S (小) / M (中) / L (大)
- **影響範囲**: spec.md / plan.md / metadata.json / sotp CLI / hooks / skills / workflow

---

## HIGH（強く推奨）

| # | 機能 | 出典 | 概要 | 難易度 | 影響範囲 |
| --- | --- | --- | --- | --- | --- |
| 1 | **信号機評価 🔵🟡🔴** | Tsumiki | 全仕様項目に信頼度シグナル (確実/推定/根拠なし) を付与。🔴 残存時は implementing 遷移をブロック | M | spec.md, metadata.json, sotp CLI |
| 2 | **要件-タスク双方向トレーサビリティ** | CC-SDD | plan.md タスクに `_Requirements: S-01_` タグ。spec.md に Coverage Summary。実装漏れを機械的に検出 | M | spec.md, plan.md, sotp CLI |
| 3 | **cross-artifact 整合性分析** | Spec Kit `analyze` | spec ↔ plan ↔ verification の 6 カテゴリ自動検出 (Duplication, Ambiguity, Underspecification, Constitution, Coverage, Inconsistency)。読み取り専用レポート | L | sotp CLI (新サブコマンド) |
| 4 | **多段階バリデーション** | SpecPulse | spec → plan → task → impl の各段階で `sotp verify` サブコマンド。フェーズごとの品質ゲート | M | sotp CLI |
| 5 | **テストファイル削除ブロック** | Anthropic 公式 | PreToolUse hook でテストファイル (`*_test.rs`, `tests/`) の削除・大幅削減をブロック (exit 2) | S | hooks |

---

## MEDIUM（推奨）

| # | 機能 | 出典 | 概要 | 難易度 | 影響範囲 |
| --- | --- | --- | --- | --- | --- |
| 6 | **ソース帰属** | Tsumiki | 仕様項目に `[source: PRD §3.2]` / `[source: inference]` タグ。根拠の追跡可能性 | S | spec.md テンプレート |
| 7 | **spec drift 検出** | OpenSpec `sync` | 実装後に spec.md と実際のコードの乖離を検出。Delta Spec (ADDED/MODIFIED/REMOVED) で差分管理 | M | sotp CLI, spec.md |
| 8 | **clarify フェーズ (9 カテゴリスキャン)** | Spec Kit `clarify` | functional scope, data model, UX flows, non-functional, integrations, edge cases, constraints, terminology, completion signals の網羅スキャン。最大 5 問/セッション | M | `/track:plan` skill |
| 9 | **明示的承認ゲート** | CC-SDD | `/track:plan` を 2 段階に: (1) 要件定義 → ユーザー承認 → (2) 設計・タスク分解。`-y` で自動承認も可 | S | `/track:plan` skill |
| 10 | **3 次元 verify (completeness/correctness/coherence)** | OpenSpec `verify` | 完全性 (全タスク完了・全要件実装)、正確性 (spec 意図合致・エッジケース)、一貫性 (設計判断反映・パターン統一) | M | sotp CLI, verification.md |
| 11 | **仕様品質テスト (checklist)** | Spec Kit `checklist` | 「英語のユニットテスト」: 要件の完全性・明確性・一貫性・測定可能性を検証。トレーサビリティ参照 80% 必須。実装テストとは別 | M | verification.md, sotp CLI |
| 12 | **差分ヒアリング** | Tsumiki | 既存ドキュメントを先に読み込み、不足情報のみ質問。毎回ゼロからの質問を回避 | S | `/track:plan` skill |
| 13 | **Hook profile (minimal/standard/strict)** | ECC | 開発フェーズに応じて hook の厳格さを切替。bootstrap 時は minimal、通常は standard、リリース前は strict | M | hooks, settings.json |
| 14 | **metadata.json タスク説明の immutable 化** | Anthropic 公式 | AI がタスクの `description` フィールドを変更不可に。`status` / `passes` のみ変更可。タスク定義の改竄防止 | S | sotp CLI |
| 15 | **Plankton 3 フェーズ品質 (write-time)** | ECC | PostToolUse で (1) silent auto-fix (formatter) → (2) subprocess remediation → (3) main agent reporting。Hook とは別の品質保証層 | L | hooks |
| 16 | **Steering 自動生成 (プロジェクトコンテキスト)** | CC-SDD | コードベース自動分析から `tech-stack.md` / `conventions/` のドラフトを生成。オンボーディング効率化 | M | `/track:catchup` skill |
| 17 | **Session Startup Ritual** | Anthropic 公式 | セッション開始時に (1) pwd (2) git log (3) feature list 確認 (4) スモークテスト (5) 作業開始。体系化された立ち上げ | S | `/track:catchup` skill |

---

## LOW（参考・中長期）

| # | 機能 | 出典 | 概要 | 難易度 | 影響範囲 |
| --- | --- | --- | --- | --- | --- |
| 18 | **EARS 記法** | CC-SDD, Tsumiki | WHEN/IF/WHILE/THEN の構造化要件記法。曖昧性排除に効果的だが Rust 特化テンプレートには過剰な場合あり | S | spec.md テンプレート |
| 19 | **Delta Spec** | OpenSpec | 全体再記述ではなく ADDED/MODIFIED/REMOVED の差分のみ記述。Brownfield 対応の基盤 | M | spec.md |
| 20 | **Constitution 形式化** | Spec Kit | `tech-stack.md` + `conventions/` をセマンティックバージョニング付き憲法として権威化 | M | tech-stack.md, conventions/ |
| 21 | **Two-Agent Architecture** | Anthropic 公式 | Initializer Agent + Coding Agent の分離。`/track:auto` 設計入力 | L | `/track:auto` skill |
| 22 | **TDD 完了時の要件網羅率** | Tsumiki | spec.md 各要件 → テストケースのトレーサビリティマトリクス。80% 未満で警告 | M | verification.md, sotp CLI |
| 23 | **Spec YAML frontmatter** | SpecPulse | spec.md に `status: draft/approved/implemented`、`version`、`signals` メタデータ | S | spec.md テンプレート |
| 24 | **Instinct Learning** | ECC | セッションからパターン自動抽出、confidence scoring (0.3-0.85)、skill への evolution | L | hooks, memory |
| 25 | **component validation tests** | ECC | agent/skill/hook の frontmatter・構造テスト (992 tests 相当)。CI 回帰防止 | M | CI |
| 26 | **LLM Compliance 強制** | SpecPulse | AI 操作をプロジェクトディレクトリに制限 + 全 file ops を監査ログに記録 | M | hooks |
| 27 | **実装検証コマンド (validate-impl)** | CC-SDD | spec.md 要件と実装済みテストケースを照合。done 遷移条件に validate 通過を追加 | M | sotp CLI |

---

## クイックウィン（小難易度 + 高効果）

すぐに着手可能で効果が高いもの:

| # | 機能 | 難易度 | 効果 |
| --- | --- | --- | --- |
| 5 | テストファイル削除ブロック | S | アンチパターン防止。hook 1 本追加 |
| 6 | ソース帰属 | S | spec.md テンプレートに `[source: ...]` を追加するだけ |
| 9 | 明示的承認ゲート | S | `/track:plan` skill のプロンプト変更 |
| 12 | 差分ヒアリング | S | `/track:plan` skill のプロンプト変更 |
| 14 | タスク説明 immutable 化 | S | sotp CLI のバリデーション追加 |
| 17 | Session Startup Ritual | S | `/track:catchup` skill の拡充 |
| 23 | Spec YAML frontmatter | S | spec.md テンプレートに frontmatter 追加 |

---

## 実装ロードマップ

### Phase 1: 仕様品質の基盤（クイックウィン含む）

```
#1  信号機評価 🔵🟡🔴
#2  双方向トレーサビリティ
#6  ソース帰属
#23 Spec YAML frontmatter
#14 タスク説明 immutable 化
```

**ゴール**: spec.md の各項目に「信頼度 + 根拠 + タスクリンク」が付き、仕様の品質が可視化される

### Phase 2: 検証の自動化

```
#4  多段階バリデーション
#5  テストファイル削除ブロック
#3  cross-artifact 整合性分析
#10 3 次元 verify
#11 仕様品質テスト (checklist)
```

**ゴール**: `sotp verify` で spec → plan → task → impl の各段階を自動検証

### Phase 3: ワークフロー改善

```
#8  clarify フェーズ
#12 差分ヒアリング
#9  明示的承認ゲート
#17 Session Startup Ritual
#7  spec drift 検出
```

**ゴール**: `/track:plan` が体系的な曖昧性解消と承認フローを持つ

### Phase 4: ハーネス最適化

```
#15 Plankton 3 フェーズ品質
#13 Hook profile
#16 Steering 自動生成
#22 TDD 要件網羅率
```

**ゴール**: write-time 品質強制 + 段階的 hook + プロジェクト立ち上げ効率化

---

## 追加調査: OpenAI Symphony (2026-03-17)

> **出典**:
> - <https://zenn.dev/komlock_lab/articles/openai-symphony> (Zenn 解説記事)
> - <https://github.com/openai/symphony> (公式リポジトリ)
> - <https://github.com/openai/symphony/blob/main/SPEC.md> (完全仕様書)
> **調査データ**: `.claude/docs/research/symphony-analysis-2026-03-17.md`

Symphony はチケット駆動のマルチエージェントオーケストレーション仕様。
Linear のチケットを監視し、エージェント (Codex) にディスパッチする meta-system。
SPEC.md で 7 コンポーネント (Workflow Loader, Config Layer, Issue Tracker Client, Orchestrator, Workspace Manager, Agent Runner, Status Surface) を定義。

### SPEC.md から判明した追加アーキテクチャ

#### 7 コンポーネント構成

| コンポーネント | 役割 |
| --- | --- |
| Workflow Loader | WORKFLOW.md (YAML + Liquid) を読み込み、動的リロード対応 |
| Config Layer | YAML frontmatter + 環境変数オーバーライド |
| Issue Tracker Client | Linear/GitHub Issues の抽象化、ポーリング + 状態遷移 |
| Orchestrator | コアループ: ポーリング → クレーム → ディスパッチ → 監視 → リトライ |
| Workspace Manager | Per-issue git clone + ブランチ作成 + サンドボックス + クリーンアップ |
| Agent Runner | Codex App-Server JSON-RPC 統合、run attempt 状態管理 |
| Status Surface | リアルタイム可視性、構造化ログ、集約メトリクス |

#### 3 層リトライ分類

| リトライ種別 | 遅延 | 用途 |
| --- | --- | --- |
| Continuation | 1000ms 固定 | エージェントが追加ターンを必要とする場合（同一スレッド継続） |
| Failure | 指数バックオフ: `min(10000 * 2^(attempt-1), max_backoff)` | エージェント失敗、スクラッチからリトライ |
| Fatal | リトライなし、issue リリース | 回復不可能なエラー |

#### Workspace 安全性不変条件 (3 つ)

1. エージェントは per-issue workspace 内でのみ動作（共有リポジトリには触れない）
2. workspace パスは設定されたルートディレクトリ内に留まる
3. workspace キーはサニタイズ済み（英数字 + ハイフンのみ）

### Symphony から得られる示唆

| # | パターン | 優先度 | 概要 |
| --- | --- | --- | --- |
| 28 | **Rework = 完全リセット戦略** | MEDIUM | 失敗時にパッチ修正ではなく、PR を閉じて新しいブランチで全面やり直し。「増分パッチより clean restart の方が品質が高い」 |
| 29 | **WORKFLOW.md (Configuration as Code)** | LOW | ワークフロー定義を YAML frontmatter + Liquid テンプレートで version control。チーム間で同一プロンプトを共有 |
| 30 | **Per-ticket workspace isolation** | LOW | チケットごとに独立ディレクトリ + git clone + sandbox。Agent Teams の `isolation: "worktree"` で既に部分対応 |
| 31 | **「チケット品質 = コード品質」原則** | MEDIUM | spec.md / タスク記述の品質がそのまま実装品質を決定する。信号機評価 (#1) + clarify (#8) で品質を担保 |
| 32 | **インフラ前提条件の明文化** | LOW | 「テストも CI もないリポジトリに入れても意味がない」— テンプレート利用の前提条件を明記 |
| 33 | **3 層リトライ分類** | LOW | Continuation (即時) / Failure (指数バックオフ) / Fatal (リリース) の 3 分類。現状の手動リトライを体系化 |
| 34 | **Workspace 安全性不変条件** | LOW | パストラバーサル防止 + サニタイズ。WORKER_ID 分離の形式化に有用 |
| 35 | **Status Surface (構造化可観測性)** | LOW | per-issue ステータス + 集約メトリクス + 構造化ログ。registry.md の拡充 |

### 既存候補との関連

- **#28 (Rework = 完全リセット)**: `/track:implement` のサブエージェントが同一エラーで 2 回失敗した場合、incremental fix ではなく clean restart を選択するルール。CLAUDE-BP-05 (「2 回失敗 → /clear」) と合流
- **#31 (チケット品質原則)**: 信号機評価 (#1) + clarify (#8) + 仕様品質テスト (#11) の組み合わせで実現。Symphony の知見は「仕様品質への投資が最もレバレッジが高い」ことの追加根拠
- **#33 (3 層リトライ)**: `/track:implement` の Agent Teams リトライ戦略に応用可能。現状は手動リトライだが、continuation (追加ターン) / failure (クリーンリスタート) / fatal (ユーザーエスカレーション) の分類は有用
