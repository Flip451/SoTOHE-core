# SoTOHE-core

**S**ingle Source **o**f **T**ruth **O**riented **H**arness **E**ngine

Rust ベースの AI 開発オーケストレーションハーネス。仕様駆動開発（SDD）ワークフローを型安全な状態遷移と厳格な品質ゲートで管理する CLI テンプレート。

## ビジョン: 型 + trait + テスト = プログラムの SoT

プログラムの真の Single Source of Truth は 3 要素で構成される:

| 要素 | 定義するもの |
|---|---|
| **型 (enum/struct)** | 何が存在しうるか — 状態空間の制約 |
| **trait** | 何ができるか — 振る舞いの契約 |
| **テスト** | 具体的にどう動くか — 入出力の証拠 |

**実装はこれらに従う導出物**。TDD の Red = SoT を定義する、Green = SoT から導出する。

要件は始めは曖昧。「書いてみて、型に起こしてみて、矛盾に気づいて、要件に戻る」探索的ループでしか精度は上がらない:

```
[ざっくり要件] ⇆ [spec.md + 信号機] ⇆ [型 + trait スケルトン]
      ↑                                        ↓
      └──────────── 矛盾・漏れの発見 ───────────┘
```

## 設計哲学

### 不変条件

このテンプレートのすべての設計判断は、以下の不変条件に拘束される。

1. **Make Illegal States Unrepresentable** — 不正な状態を型で表現不可能にする。有限の状態は `String` ではなく enum、振る舞いは trait、遷移条件はテストで定義する。domain に `String` が残ると解釈ロジックが全層に散らばる
2. **ルールとバリデーションの同時デプロイ** — AI にルールだけ渡しても守られない。仕様定義と検証コマンドは常にセットで導入する（垂直スライス原則）
3. **インクリメンタル修復より完全リセット** — AI のコンテキスト汚染はトークン浪費とハルシネーションの連鎖を招く。致命的エラー時はワークスペースごと破棄してゼロから再構築する方がトータルコストが安い
4. **ワークスペースの物理的隔離** — 並列エージェントには物理的に隔離された worktree を割り当てる
5. **コンパイラが最終審判** — AI の主観的判断で品質を昇格させない。`cargo make ci` が通ったという客観的証拠のみが品質の根拠となる

### Human-in-the-Loop

人間の役割を明示的に制限することで、逆説的に人間の負担を減らす。

- **人間が行うこと**: 承認ゲートの通過、仕様の不確実性（🔴）の解消、アーキテクチャレベルの意思決定
- **システムに委譲すること**: 状態遷移、実装ループ、リンター/CI フィードバック、レビューサイクル、リカバリー判断

人間はこのシステムの**指揮者**であり、すべての楽器を自分で演奏する必要はない。

## アーキテクチャ

```
apps/cli          Composition Root（clap CLI）
libs/usecase      アプリケーションロジック
libs/domain       ドメイン型・ポート（trait）・ガード
libs/infrastructure  アダプター（ファイルシステム、Git、GitHub CLI）
```

依存方向は一方向のみ: `cli → usecase → domain ← infrastructure`。`cargo make check-layers` で CI 強制。

## 技術スタック

| 項目 | 選定 |
|------|------|
| 言語 | Rust stable, Edition 2024, MSRV 1.85 |
| ランタイム | 同期（async なし） |
| CLI | clap 4.5 |
| 永続化 | JSON ファイルベース（DB なし） |
| タスクランナー | cargo-make |
| テスト | cargo nextest + rstest + mockall |
| カバレッジ | cargo-llvm-cov |
| 静的解析 | clippy（unwrap/expect/panic/indexing を deny） |
| コンテナ | Docker + docker compose |

詳細: `track/tech-stack.md`

## クイックスタート

```bash
# 前提: Docker + docker compose がインストール済み
cargo make bootstrap    # 初回セットアップ

# Claude Code チャットで:
/track:catchup          # 環境確認 + プロジェクト状態の把握
/track:plan <feature>   # 機能の計画 → spec.md / plan.md 生成
/track:implement        # 並列実装
/track:review           # 外部レビュアーによるレビュー
/track:ci               # CI ゲート通過確認
/track:commit <message> # ガード付きコミット + git note
/track:pr               # PR 作成
/track:merge            # CI 待ち → マージ
/track:done             # main に切替 + 完了サマリー
```

詳細: `DEVELOPER_AI_WORKFLOW.md`

## 品質ゲート

`cargo make ci` が全ゲートを一括実行:

- `fmt-check` — rustfmt フォーマット確認
- `clippy` — 静的解析（`-D warnings`、panic 系 lint deny）
- `test` — cargo nextest
- `deny` — ライセンス・禁止クレート
- `check-layers` — レイヤー依存方向の違反検出
- `verify-*` — トラックメタデータ・レジストリ・仕様整合性
- `scripts-selftest` / `hooks-selftest` — インフラ回帰テスト

## エージェント協調

### 6 段階ワークフロー

```
domain_modeler → spec_reviewer → planner → implementer → code_reviewer → acceptance_reviewer
     ↑                                                          ↓
     └──────────── 3ラウンド同一 concern → 信号機降格 ───────────┘
```

### Capability と Provider

| Capability | 既定プロバイダ | 用途 |
|---|---|---|
| orchestrator | Claude Code | ワークフロー制御、ファイル編集 |
| domain_modeler | Codex CLI | 状態・遷移の洗い出し、型スケルトン生成 |
| spec_reviewer | Codex CLI | ドメインモデルの設計レビュー、信号機評価 |
| planner | Codex CLI | タスク分解、依存関係整理 |
| implementer | Claude Code | TDD で実装 (Red → Green → Refactor) |
| code_reviewer | Codex CLI | コード品質、アーキテクチャ準拠 |
| acceptance_reviewer | Codex CLI | spec 要件と実装の突き合わせ |
| researcher | Gemini CLI | 外部調査、crate サーベイ |

設定: `.claude/agent-profiles.json`

## ガードレール

- `git add` / `git commit` の直接実行は hook でブロック → `cargo make` ラッパー経由を強制
- テストファイルの削除は PreToolUse hook でブロック
- シェルコマンドは AST パーサー（conch-parser）で解析し、危険な操作を検出
- レビュアー（Codex）が zero findings を返すまでコミット不可
- 同一 concern が 3 ラウンド連続 → エスカレーション発動（サーキットブレーカー）

## ロードマップ

**ゴール**: 「spec → 型 + trait + テスト → 実装（導出）」のパイプラインを仕組みとして完成させる。

```
[今ここ]
  ↓
domain を型で厳密に表現する (Phase 1.5)
  ↓
仕様の品質を信号機で可視化する (Phase 2)
  ↓
TDD を状態マシンで強制し、型+テストから実装を導出する (Phase 3)
  ↓
並列実行を物理的に隔離する (Phase 4)
  ↓
開発体験を磨く (Phase 5-6)
  ↓
実装の自動導出 = /track:auto (Phase 7 将来)
```

| Phase | 状態 | 何をするか | なぜ必要か |
|---|---|---|---|
| 0 | ✅ | Shell wrapper の Rust 化 | Python 依存の排除。CLI の安全性基盤 |
| 1 | ✅ | データロス修正 + spec テンプレート基盤 | 仕様駆動開発の受け皿づくり |
| **1.5** | **▶** | **domain の String → enum 型化 + CLI ロジックの usecase 移動** | **ロジック流出の根本原因を断つ。以降の全 Phase の前提** |
| 2 | — | 信号機 (🔵🟡🔴) + 要件トレーサビリティ | 仕様の不確実性を可視化し、🔴 ありで実装をブロック |
| 3 | — | TDD 状態マシン (Red→Green→Refactor を CI で強制) | 型+テストを先に書くことをシステムで保証 |
| 4 | — | エフェメラル worktree + リカバリー 3 層 | 並列エージェントの物理隔離 + 障害時の自動リカバリー |
| 5 | — | clarify フェーズ + 構造化ログ + UX 改善 | 開発ループの高速化 |
| 6 | — | アーキテクチャ設計スパイク | 長期スケーラビリティ検証 |

## ドキュメント構成

| ファイル | 役割 |
|---------|------|
| `README.md` | 設計哲学 + 入口（このファイル） |
| `DEVELOPER_AI_WORKFLOW.md` | ユーザー向け操作ガイド |
| `CLAUDE.md` | 保守者向けインデックス |
| `track/workflow.md` | 日々のワークフロールール |
| `track/tech-stack.md` | 技術スタック SSoT |
| `.claude/docs/DESIGN.md` | 技術設計ドキュメント |
| `project-docs/conventions/` | プロジェクト固有規約 |
| `docs/architecture-rules.json` | レイヤー依存の機械可読 SSoT |

## ライセンス

MIT OR Apache-2.0 のデュアルライセンス。
