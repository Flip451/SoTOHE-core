# SoTOHE-core

**S**ource **o**f **T**ruth **O**riented **H**arness **E**ngine

AI エージェントによる仕様駆動開発 (SDD) を管理する Rust 製 CLI テンプレート。
**提供する価値 = SoT Chain**: 要件・型契約・実装を一方向参照で結び、仕様と実装のドリフトを構造的に防止する。

## 開発単位 = track

SoTOHE はすべての作業を **track** で管理する。1 track = 1 機能追加・1 バグ修正・1 リファクタリング相当で、`仕様 → 型契約 → 実装 → レビュー → コミット & マージ` が独立したファイルとして保存される。

## SoT Chain: 4 階層の独立 SoT

SoTOHE は Source of Truth (SoT) を 4 階層に分解し、層間を一方向の参照チェーンで結ぶ:

| 層 | SoT ファイル | ライフサイクル |
|---|---|---|
| **ADR** | `knowledge/adr/*.md` | track を跨ぐ恒久的な設計決定 |
| **仕様書** | `spec.md` / `spec.json` | track ごとに作成される要件書 |
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

下流は上流を必ず参照し、参照漏れは CI で Red となり merge がブロックされる。

## 参照の評価 (信号機)

| 参照 | 🔵 Blue | 🟡 Yellow | 🔴 Red |
|---|---|---|---|
| **実装 → 型契約** | 実装と契約が一致 | 未実装 | 契約違反 |
| **型契約 → 仕様書** | 宣言の根拠あり | 根拠あるが未文書化 | 根拠なし |
| **仕様書 → ADR** | 永続化文書に根拠あり | 根拠あるが非永続化 | 根拠なし |

- 🔵 通す
- 🟡 コミット可能、マージ不可 (track 終了までに直す)
- 🔴 コミット不可 (即修正必須)

参照チェーンが全て 🔵 で埋まらない限り track は完了できない。

## クイックスタート

```bash
cargo make bootstrap                 # 初回セットアップ (Docker + docker compose 必須)

# Claude Code チャットで:
/track:catchup                       # 環境確認 + 状態把握
/track:plan <feature>                # 仕様 + 計画 + 型契約 + 実装計画
/track:type-design                   # (任意) TDDD 型カタログ宣言
/track:implement                     # 対話型並列実装
# または /track:full-cycle <task>    (自律実装)
/track:review                        # 外部レビュアーによるレビュー
/track:commit <message>              # ガード付きコミット + git note
```

詳細: `DEVELOPER_AI_WORKFLOW.md`
エージェント設定: `.harness/config/agent-profiles.json`

## ロードマップと関連ドキュメント

- 全体計画: `knowledge/strategy/TODO-PLAN.md`
- ビジョン: `knowledge/strategy/vision.md`
- 利用者向けワークフロー: `DEVELOPER_AI_WORKFLOW.md`

## ライセンス

MIT OR Apache-2.0 のデュアルライセンス。
