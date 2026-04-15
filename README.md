# SoTOHE-core

**S**ource **o**f **T**ruth **O**riented **H**arness **E**ngine

AI エージェントによる仕様駆動開発 (SDD) を管理する Rust 製 CLI テンプレート。
**提供する価値 = SoT Chain**: 要件・型契約・実装を一方向参照で結び、仕様と実装のドリフトを構造的に防止する。

## SoTOHE の開発ワークフローと track

SoTOHE はすべての作業を **track** という単位で管理する。

> **track** = 開発ワークフローの最小単位 (機能追加・バグ修正・リファクタリング等)。大雑把に言えば、1 track ごとに「**仕様・計画策定 → 型デザイン → 実装 → レビュー → コミット & マージ**」の順で作業が進む。

1 track = 1 機能追加 (または 1 バグ修正、1 リファクタリング) が基本。track ごとに仕様書・型契約・実装・検証結果が独立したファイルとして保存される。

## SoT Chain: 4 階層の独立 SoT

SoTOHE が提供する価値は **SoT Chain**: プログラムの Source of Truth (SoT) を 4 階層に分解し、層間を一方向の参照チェーンで結ぶ仕組み。

4 階層はライフサイクルが異なる:

| 層 | SoT ファイル | ライフサイクル |
|---|---|---|
| **ADR** | `knowledge/adr/*.md` | **track を跨ぐ恒久的な設計決定**。一度書いたら原則不変 |
| **仕様書** | `spec.md` / `spec.json` | **track ごとに作成** される track の要件書 |
| **型契約** | `<layer>-types.json` | track の型宣言 (型レベルのテスト) |
| **実装** | `libs/<layer>/src/**/*.rs` | track のコード |

```
ADR (恒久的)
  ↑ 参照
仕様書 (track ごと)
  ↑ 参照
型契約 (track ごと)
  ↑ 参照
実装 (track ごと)
```

各層は独立したライフサイクルを持つが、**下流の層は上流の層を必ず参照する**。参照漏れは CI で Red となり、merge がブロックされる。これが SoT Chain による仕様と実装のドリフト防止の仕組み。

## 参照チェーンの評価

各参照は CI で以下のように評価される:

| 参照 | 🔵 Blue | 🟡 Yellow | 🔴 Red |
|---|---|---|---|
| **実装 → 型契約** | 実装と契約が一致 | 未実装 | 契約違反 |
| **型契約 → 仕様書** | 型の宣言の根拠あり | 根拠はあるが未文書化 | 根拠なし |
| **仕様書 → ADR** | 記述の根拠が永続化文書にあり | 根拠はあるが非永続化 | 根拠なし |

**各信号の意味**:

- 🔵 **通す** — そのままでよい
- 🟡 **コミット可能、マージ不可** — track 終了までに直す必要がある
- 🔴 **コミット不可** — 即修正必須

つまり、**参照チェーンが全て 🔵 で埋まっていない限り track は完了できない**。これが SoT Chain がドリフト防止を保証する仕組み。

**参照の表現方法**: 「型契約 → 仕様書」「仕様書 → ADR」は成果物内に明示リンク (JSON フィールド / テキストタグ) として埋め込まれる。一方「**実装 → 型契約**」は実装コード内に参照を埋め込むわけではなく、rustdoc JSON で抽出した実装の型情報と型カタログの宣言を CI が **突合** して評価する。

## 探索的精緻化ループ

要件は始めは曖昧。各層を下流に進めると矛盾・漏れが発見される。評価が 🔴/🟡 なら退行して修正し、最終的に全て 🔵 で埋まる状態を目指す:

```
[ADR]
  ↑ ↓ ①
[仕様書]
  ↑ ↓ ②
[型契約]
  ↑ ↓ ③
[実装]
```

- **↓ フェーズ進行**: ① `/track:plan`、② `/track:design`、③ `/track:implement` (または自律実装の `/track:full-cycle`) による成果物の作成、または強制退行からの自動復帰
- **↑ フェーズ退行**: 🔴/🟡 を 🔵 にするため、前の成果物に戻って修正する (信号機評価に基づく強制遷移)

## クイックスタート

```bash
# 前提: Docker + docker compose がインストール済み
cargo make bootstrap    # 初回セットアップ

# Claude Code チャットで:
/track:catchup                 # 環境確認 + プロジェクト状態の把握
/track:plan <feature>          # Phase A: 要件スケッチ + tech-stack 確定
/track:design                  # Phase B: 型デザイン (多層対応)
/track:implement               # Phase C: 並列対話型実装
# または /track:full-cycle <task>  (自律実装)
/track:review                  # 外部レビュアーによるレビュー
/track:commit <message>        # ガード付きコミット + git note

# 以降は GitHub 上の操作 (SoTOHE の中核ではないため必要に応じて):
# /track:pr — PR 作成
# /track:merge <pr> — CI 待ち → マージ
# /track:done — main に切替 + 完了サマリー
# (/track:ci は review / commit から自動呼び出しされるため明示実行は不要)
```

詳細: `DEVELOPER_AI_WORKFLOW.md`

## エージェント協調

複数の AI エージェントが役割分担してワークフローを進める:

| 役割 | 既定プロバイダ | 用途 |
|---|---|---|
| orchestrator | Claude Code | ワークフロー制御、ファイル編集 |
| planner | Claude Code (Opus) | アーキテクチャ設計、タスク分解 |
| designer | Claude Code | 型カタログ宣言、シグネチャ検証設計 |
| implementer | Claude Code (Opus) | 並列実装 (`.harness/config/agent-profiles.json` で capability override) |
| reviewer | Codex CLI | コード品質レビュー、正確性確認 |
| researcher | Gemini CLI | 外部調査、crate サーベイ、依存監査 |

設定: `.harness/config/agent-profiles.json`

## ロードマップ

| 状態 | 項目 |
|---|---|
| ✅ | 基盤整備 (CLI 安全性、spec テンプレート、spec.json SSoT) |
| ✅ | 仕様書 → ADR の評価 (🔵🟡🔴) + 要件トレーサビリティ |
| ✅ | ヒアリング UX 改善 (構造化質問、モード選択) |
| ✅ | 型カタログ分離 + 型カテゴリ taxonomy (domain + application 層の 12 variants) |
| ▶ | **実装 → 型契約の評価拡張** (multilayer + シグネチャ検証 + baseline + action) |
| ▶ | **ハーネスの段階的 typestate-first リファクタリング** |
| 計画中 | **型契約 → 仕様書の評価実装** (SoT Chain の 3 番目のリンクを完成させる) |
| 計画中 | **SoT Chain をテストコードまで拡張** (仕様書 + 型スケルトンからテスト自動生成、実装の振る舞いを制御) |
| 計画中 | sotp 独立配布 + Drift Detection |
| 計画中 | Harness Template 展開 + 多言語対応 |

## 関連ドキュメント

- `DEVELOPER_AI_WORKFLOW.md` — 利用者向けワークフローガイド
- `knowledge/strategy/vision.md` — 全体ビジョン (v6 draft: `tmp/vision-v6-draft.md`)
- `knowledge/strategy/TODO-PLAN.md` — 全体計画 (v6 draft: `tmp/TODO-PLAN-v6-draft.md`)

## ライセンス

MIT OR Apache-2.0 のデュアルライセンス。
