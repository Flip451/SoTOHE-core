# SoTOHE-core 全体計画 v4 ドラフト

> **作成日**: 2026-03-23
> **前版**: `knowledge/strategy/TODO-PLAN.md` (v3)
> **ビジョン**: [`knowledge/strategy/vision.md`](vision.md) ← v4 採用時に改訂
> **変更理由**: sotp CLI をスタンドアロンツールとして切り出し、テンプレートは sotp ワークフローを前提とする方針に転換
> **分析レポート**: [`tmp/template-overfitting-analysis-2026-03-23.md`](./template-overfitting-analysis-2026-03-23.md)
> **リファクタリング詳細**: [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md)
> **進捗管理**: [`knowledge/strategy/progress-tracker.md`](progress-tracker.md) ← v4 採用時に改訂
> **TODO 詳細**: [`knowledge/strategy/TODO.md`](TODO.md)

---

## 戦略サマリー v4

**v3**: ハーネス自身のコードとテンプレート出力を概念的に区別する。

**v4**: sotp CLI を独立ツールとして物理的に分離する。
テンプレート利用者は sotp の正規ワークフロー（track, hooks, review cycle）を使う前提。
テンプレートは sotp インストール済みの環境で動作し、利用者向けの外枠は後から実装する。

```
Phase 0 (✅)   基盤: shell wrapper Rust 化
Phase 1 (✅)   クイックウィン: 事故予防 + spec テンプレート基盤
Phase 1.5 (▶)  sotp CLI 品質改善 + 論理分離
Phase 2        仕様品質: 信号機 + トレーサビリティ
Phase 3        sotp テスト生成サブコマンド ← Moat
Phase 4        インフラ + sotp 配布
Phase 5        ワークフロー最適化
Phase 6        テンプレート外枠（scaffold）
```

### v3 → v4 の変更点

| 項目 | v3 | v4 |
|---|---|---|
| sotp の位置づけ | テンプレートに埋め込み | **スタンドアロン CLI ツール** |
| テンプレートの Cargo workspace | sotp ソース込み | **ユーザーのコード専用（空スケルトン）** |
| 非 Rust インフラの過学習 | 問題として未認識 | **sotp エコシステムの一部として正当化** |
| BRIDGE-01 | 生成プロジェクト向けツール | **sotp のサブコマンド** |
| Phase 1.5 | コード品質改善のみ | **+ 論理分離（SPLIT-01/02）** |
| Phase 4 | インフラのみ | **+ sotp 配布（SPLIT-03/04/05）** |
| Phase 6 | なし | **新設: テンプレート外枠（scaffold）** |

---

## Phase 0: ✅ 完了

（v3 と同じ）

---

## Phase 1: ✅ 完了（10/10）

（v3 と同じ）

---

## Phase 1.5: sotp CLI 品質改善 + 論理分離（▶ 進行中）

> **詳細計画**: [`tmp/refactoring-plan-2026-03-19.md`](./refactoring-plan-2026-03-19.md)

**目標**: CLI 肥大化解消 + domain 型化 + sotp とテンプレートの論理的境界を確立。

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
| 1.5-17 | ~~WF-55 Phase 1: view-freshness CI~~ | S | ✅ done (PR #46) |
| 1.5-18 | **capability 追加**: domain_modeler, spec_reviewer, acceptance_reviewer | S | 未着手 |
| 1.5-19 | ~~INF-15: usecase-purity CI~~ | S | ✅ done |
| 1.5-20 | ~~INF-16: pr_review.rs hexagonal~~ | S | ✅ done (PR #51) |
| 1.5-21 | ~~INF-17: usecase-purity error 昇格~~ | S | ✅ done (PR #52) |
| 1.5-22 | INF-18: verify ルール定義の外部設定化 | S | 未着手 |
| 1.5-23 | INF-19: domain-purity CI | S | 未着手 |
| 1.5-24 | INF-20: conch-parser を infrastructure に移動 | M | 未着手 |
| **1.5-25** | **SPLIT-01: sotp / テンプレートの論理分離** | **M** | **NEW** |
| **1.5-26** | **SPLIT-02: bin/sotp パス抽象化** | **S** | **NEW** |
| **1.5-27** | **RVW-03: record-round verdict attestation** | **S** | **NEW** — WF-43 |

### SPLIT-01: sotp / テンプレートの論理分離（NEW）

**目標**: 同一リポ内で sotp CLI のコードとテンプレートスケルトンの境界を明確にする。

**内容**:
- README / CLAUDE.md に「sotp = スタンドアロン CLI ツール」「テンプレート = sotp を使うプロジェクト基盤」の区別を明記
- Cargo workspace 内で sotp グループ（libs/domain, usecase, infrastructure, apps/cli）とテンプレートグループ（apps/server）を文書化
- `docs/architecture-rules.json` に sotp/template 境界を反映（将来の物理分割の準備）
- vision v4 を作成し `knowledge/strategy/vision.md` に保存

**やらないこと（Phase 4 に送る）**:
- 物理的なリポ分割
- sotp のバイナリ配布
- Dockerfile の sotp インストール化

### SPLIT-02: bin/sotp パス抽象化（NEW）

**目標**: `bin/sotp` のハードコード参照を抽象化し、将来の PATH ベースインストールに備える。

**内容**:
- Makefile.toml の `SOTP_BIN` 変数を導入: デフォルト `bin/sotp`、環境変数 `SOTP_BIN` でオーバーライド可能
- hooks / scripts 内の `bin/sotp` 参照を `SOTP_BIN` 変数経由に変更
- `sotp` が PATH 上にある場合は `bin/sotp` より優先するロジック

**やらないこと**:
- 既存の `build-sotp` / `bootstrap` フローの変更（引き続き bin/sotp をビルド）

---

## Phase 2: 仕様品質（テスト生成の入力品質保証）

（v3 と同じ — sotp のサブコマンドとして実装される点が明確になるが、項目自体は変わらない）

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 2-1 | **TSUMIKI-01** 信号機評価 🔵🟡🔴 | M | spec の信頼度可視化 |
| 2-2 | **SPEC-05** metadata.json に `confidence_signals` | M | 垂直スライス: 2-1 と同時 |
| 2-3 | **CC-SDD-01** 要件-タスク双方向トレーサビリティ | M | テスト ↔ 要件の紐付け基盤 |
| 2-4 | **CC-SDD-02** 明示的承認ゲート | S | plan skill プロンプト変更 |
| 2-5 | **TSUMIKI-03** 差分ヒアリング | S | plan skill プロンプト変更 |
| 2-6 | **SSoT-07** 二重書き込み解消 | M | plan.md の一貫性 |
| 2-7 | spec.md Domain States 必須化 + spec-states CI | M | テスト生成の入力 |

---

## Phase 3: sotp テスト生成サブコマンド（Moat）

**目標**: sotp のサブコマンドとして、生成プロジェクトの spec → テスト自動生成パイプラインを提供する。

> v3 からの変更: 「テンプレートツール」→「sotp サブコマンド」に位置づけ変更。
> sotp が独立ツールであることを前提に、`sotp domain export-schema` 等は
> テンプレート利用者が自分のプロジェクトで `sotp` コマンドとして実行する。

### テスト生成の 3 手法

| 手法 | 対象 | 入力 |
|---|---|---|
| spec 例 → テスト変換 | domain impl | spec の Given/When/Then |
| proptest + typestate | domain impl | export-schema のシグネチャ（関数の存在 = 有効遷移） |
| usecase モック自動生成 | usecase | `impl Fn` のクロージャモック |

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 3-1 | **BRIDGE-01** `sotp domain export-schema` (syn) | M | 生成プロジェクトの domain から pub シグネチャを抽出。sotp サブコマンド |
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

### テンプレートが推奨する生成プロジェクトの設計

（v3 と同じ）

---

## Phase 4: インフラ + sotp 配布

> v3 からの変更: sotp 配布関連の 3 項目を追加。

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
| **4-9** | **SPLIT-03: sotp crate 公開準備** | **M** | **NEW** |
| **4-10** | **SPLIT-04: sotp バイナリ配布 (GitHub Releases)** | **M** | **NEW** |
| **4-11** | **SPLIT-05: Dockerfile sotp インストール化** | **S** | **NEW** |

### SPLIT-03: sotp crate 公開準備（NEW）

**内容**:
- sotp の Cargo.toml を publish 可能な状態に整備（license, description, repository 等）
- `cargo install sotp` でインストールできるよう crates.io 公開準備
- バージョニング戦略の決定（semver、テンプレートとの互換性マトリクス）

### SPLIT-04: sotp バイナリ配布（NEW）

**内容**:
- GitHub Actions で sotp バイナリをクロスコンパイル + リリース
- `cargo-binstall` 対応メタデータ
- インストール手順のドキュメント（README, DEVELOPER_AI_WORKFLOW.md）

### SPLIT-05: Dockerfile sotp インストール化（NEW）

**内容**:
- Dockerfile の tools ステージに sotp バイナリインストールを追加
- `COPY --from=builder` パターンまたは GitHub Releases からのダウンロード
- `bin/sotp` ビルドステップの代替として sotp がコンテナ内で直接使えるように
- テンプレートの bootstrap フロー更新

---

## Phase 5: ワークフロー最適化

（v3 と同じ）

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

## Phase 6: テンプレート外枠（NEW）

> **前提**: Phase 4 の SPLIT-03/04/05 完了後（sotp が独立配布可能な状態）

**目標**: テンプレート利用者向けのプロジェクト生成・カスタマイズ機能を提供する。

| # | 項目 | 難易度 | 根拠 |
|---|---|---|---|
| 6-1 | **sotp init**: 新規プロジェクト生成 | M | fork/clone モデルからジェネレータモデルへ |
| 6-2 | **sotp scaffold**: レイヤー/モジュール追加 | M | `/architecture-customizer` の CLI 化 |
| 6-3 | **STRAT-11**: 多言語プロジェクト対応 | L | Rust 以外への展開 |
| 6-4 | **テンプレートリポ分割** | M | sotp ソースをテンプレートリポから完全除去 |
| 6-5 | **sotp upgrade**: テンプレートの sotp バージョン更新 | S | バージョン互換性管理 |

### sotp init の設計方向

```
$ sotp init my-project
? 言語: [Rust] / TypeScript / Go
? アーキテクチャ: [Hexagonal] / Layered / Clean
? レイヤー名: [libs/domain, libs/usecase, libs/infrastructure, apps/cli]
? 非同期ランタイム: [なし] / Tokio / async-std
? CI プロバイダ: [GitHub Actions] / GitLab CI
→ my-project/ にテンプレート生成
→ sotp バージョン互換性チェック
→ track/tech-stack.md の TODO: を自動充填
```

---

## Phase 間の依存関係

```
Phase 0 (✅) + Phase 1 (✅)
    ↓
Phase 1.5 (▶ sotp 品質改善 + 論理分離)
    ↓ domain 型化完了、CLI が薄くなる、sotp/template 境界が明確
Phase 2 (仕様品質 — テスト生成の入力品質)
    ↓ 信号機 + Domain States + トレーサビリティが整う
Phase 3 (sotp テスト生成サブコマンド) ← Moat
    ↓ sotp が「spec → テスト → 実装」を提供
Phase 4 (インフラ + sotp 配布)
    ↓ sotp が独立配布可能に
Phase 5 (ワークフロー)  ← Phase 4 と並行可能
Phase 6 (テンプレート外枠)  ← Phase 4 完了後
```

### v3 → v4 の依存関係変更

```
v3: Phase 1.5 → Phase 2 → Phase 3 → Phase 4 || Phase 5
v4: Phase 1.5 → Phase 2 → Phase 3 → Phase 4(+配布) → Phase 6(外枠)
                                        ↕ 並行
                                      Phase 5
```

**Phase 6 が新設**されたが、Phase 3（Moat）までの流れは変わらない。
sotp の物理分割（Step 3）は Phase 4 に入り、テンプレート外枠（Step 4）は Phase 6。
つまり **Moat 到達までのクリティカルパスに影響しない**。

---

## 見積もり

| Phase | 項目 | 残 | 推定日数 | v3 差分 |
|---|---|---|---|---|
| 1.5 | 20 (+2) | 12 (+2) | 4 日 | +1 日（SPLIT-01/02） |
| 2 | 7 | 7 | 3 日 | 変更なし |
| 3 | 11 | 11 | 5 日 | 変更なし |
| 4 | 11 (+3) | 9 (+3) | 4 日 | +1 日（SPLIT-03/04/05） |
| 5 | 7 | 7 | 3 日 | 変更なし |
| 6 | 5 (NEW) | 5 | 4 日 | +4 日（新 Phase） |
| **合計** | **61** | **51** | **~23 日** | **v3 比 +6 日** |

### クリティカルパス（Moat 到達まで）

| | v3 | v4 |
|---|---|---|
| Phase 1.5 → 3 完了 | ~11 日 | ~12 日（+1 日: SPLIT-01/02） |
| 全体完了 | ~17 日 | ~23 日（+6 日: Phase 6 新設分） |

**Moat 到達への影響は +1 日のみ。** Phase 6 は Moat 後の拡張。

---

## v4 採用時に更新が必要なファイル

| ファイル | 変更内容 |
|---------|---------|
| `knowledge/strategy/vision.md` | sotp 独立ツール化の方針を反映 |
| `knowledge/strategy/TODO-PLAN.md` | このドラフトを正式版に |
| `knowledge/strategy/progress-tracker.md` | SPLIT-01/02 を Phase 1.5 に追加、Phase 6 追加 |
| `knowledge/strategy/TODO.md` | SPLIT-01〜05 を新セクションとして追加 |
| `README.md` | sotp の位置づけ説明（「テンプレートに同梱された CLI」→「スタンドアロンツール」） |
| `CLAUDE.md` | sotp/template 分離の概要参照 |
| `DEVELOPER_AI_WORKFLOW.md` | sotp インストール手順（Phase 4 以降） |
