# SoTOHE-core

**S**pec-driven **o**rchestration and **T**est-generation **O**riented **H**arness **E**ngine

AI エージェントによる仕様駆動開発（SDD）を管理する Rust 製 CLI テンプレート。
要件から型とテストを導出し、実装を「テストを通すだけ」の作業にすることを目指す。

## ビジョン

```
要件 → テスト（大量・矛盾なし・網羅的） → 実装（テストを通すだけ）
      ↑ SoTOHE が提供する価値
```

### 型はテスト数を減らす手段

型が強いほど、テストで検証すべき状態空間が縮小する：

| 型の強さ | テストで検証する範囲 | 例 |
|---|---|---|
| `String` | 無限の入力空間を網羅 | `"zero_findings"`, `""`, `"typo"`, ... |
| `enum` | 有限の variant のみ | `Verdict::ZeroFindings`, `Verdict::FindingsRemain` |
| typestate | 有効遷移のみ。不正遷移はコンパイルエラー | `impl InProgress { fn pass_fast(self) -> FastPassed }` |

### 信号機: 仕様の信頼度を 3 段階で管理

仕様書（`spec.md`）の各要件に信頼度シグナルを付与する：

| 信号 | 意味 | 例 |
|---|---|---|
| 🔵 | 確実。裏付けあり | `[source: RFC 5321]` |
| 🟡 | 合理的推定。未検証 | `[source: inference]` |
| 🔴 | 根拠なし。要確認 | `[source: unknown]` |

🔴 が残っている要件は実装フェーズに進めない。将来的には、テストの繰り返し失敗時に信号を自動降格させ、仕様の再検討を促す仕組みを構築予定。

### 探索的精緻化ループ

要件は始めは曖昧。型に起こすことで矛盾を発見し、テストスケルトンで検証する：

```
[ざっくり要件] ⇆ [spec.md + 信号機] ⇆ [型スケルトン + テストスケルトン]
      ↑                                          ↓
      └────────────── 矛盾・漏れの発見 ───────────┘
```

## 設計哲学

1. **型でテスト数を減らす** — 不正な状態を型で表現不可能にする。テスト生成が検証すべき状態空間が縮小する
2. **仕様からテストを導出する** — 要件の具体例 (Given/When/Then) と状態遷移表からテストスケルトンを自動生成する。実装はテストを通すだけ
3. **コンパイラと CI が最終審判** — AI の主観的判断で品質を昇格させない。テスト通過という客観的証拠のみが根拠
4. **Human-in-the-Loop** — 人間は承認ゲートの通過と仕様の不確実性（🔴）の解消のみ。それ以外はシステムに委譲

## テンプレートが推奨するプロジェクト設計

このテンプレートで管理するプロジェクトでは、テスト生成の効率化を目的として以下の設計を推奨する。

- **Typestate パターンで状態遷移を定義** — 有効な遷移だけ `impl` メソッドとして存在し、不正遷移はコンパイルエラーになる。テストは存在する関数に対してのみ書けばよく、不正遷移のテストが不要になる
- **Usecase 層は `impl Fn` で依存を受け取る** — trait + mockall ではなくクロージャを渡すだけでモックが完成する。テスト生成コストが最小になる
- **DDD 概念でファイル分割し、`pub` 可視性で AI 向けコンテキストをフィルタ** — domain 層の公開 API のみをテスト生成 AI に渡し、ノイズを排除する

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

## エージェント協調

複数の AI エージェントが役割分担してワークフローを進める：

| 役割 | 既定プロバイダ | 用途 |
|---|---|---|
| orchestrator | Claude Code | ワークフロー制御、ファイル編集 |
| planner | Codex CLI | タスク分解、依存関係整理 |
| implementer | Claude Code | TDD で実装 |
| code_reviewer | Codex CLI | コード品質レビュー |
| researcher | Gemini CLI | 外部調査、crate サーベイ |

設定: `.harness/config/agent-profiles.json`

## ロードマップ

| 状態 | 何をするか |
|---|---|
| ✅ | 基盤整備（CLI 安全性、spec テンプレート） |
| **▶** | **コード品質改善（domain 型化、モジュール分割）** |
| 計画中 | 信号機 (🔵🟡🔴) による仕様品質の可視化 |
| 計画中 | 仕様 → テスト自動生成パイプライン |
| 計画中 | worktree 分離、ワークフロー最適化 |

## ライセンス

MIT OR Apache-2.0 のデュアルライセンス。
