# SoTOHE-core

**S**ource **o**f **T**ruth **O**riented **H**arness **E**ngine

AI エージェントによる仕様駆動開発 (SDD) を管理する Rust 製 CLI テンプレート。SoT（真実の源泉）指向のRust開発向けハーネスの核となるCLIを提供する。

想定する使い手は Rust で中〜大規模プロジェクトを開発するチーム — 仕様・設計・実装の乖離が起きやすく、真実の源泉が散在する問題を抱えている現場に向く。中核機能は track ワークフロー管理 CLI、メタデータ駆動の状態遷移エンジン、CI 連携バリデーションの 3 つ（GUI / Web ダッシュボードは提供しない）。

## 価値：SoT Chain とは何か

SoTOHE の中核にある **SoT Chain** は「要件 → 型契約 → 実装」を一方向の参照チェーンで結ぶ仕組みで、仕様と実装のドリフトを構造的に防止する。

### 4 階層の独立した SoT

SoTOHE は Source of Truth (SoT) を 4 階層に分解し、それぞれを独立したファイルとして保存する:

| 層 | SoT ファイル | ライフサイクル |
|---|---|---|
| **ADR** | `knowledge/adr/*.md` | track を跨ぐ恒久的な設計決定 |
| **仕様書** | `spec.md` / `spec.json` | track ごとに作成される要件書 |
| **型契約** | `<layer>-types.json` | track の型宣言 (型レベルのテスト) |
| **実装** | `libs/<layer>/src/**/*.rs` | track を跨ぐ恒久的なコード (各 track が編集を加える) |

```
ADR (恒久的)
  ↑ 参照
仕様書 (track ごと)
  ↑ 参照
型契約 (track ごと)
  ↑ 参照
実装 (恒久的 / track を跨ぐ)
```

下流は上流を必ず参照する。参照が切れると CI で 🔴 Red となり merge がブロックされる。

### 信号機：参照の評価

| 参照 | 🔵 Blue | 🟡 Yellow | 🔴 Red |
|---|---|---|---|
| **実装 → 型契約** | 実装と契約が一致 | 未実装 | 契約違反 |
| **型契約 → 仕様書** | 宣言の根拠あり | 根拠あるが未文書化 | 根拠なし |
| **仕様書 → ADR** | 永続化文書に根拠あり | 根拠あるが非永続化 | 根拠なし |

- 🔵 — そのまま進める
- 🟡 — コミット可能、ただし track 終了前に解消が必要
- 🔴 — コミット不可（即修正必須）

参照チェーンが全て 🔵 で埋まらない限り track は完了できない。

### 開発単位 = track

SoTOHE はすべての作業を **track** で管理する。1 track = 1 機能追加・1 バグ修正・1 リファクタリング相当で、`仕様 → 型契約 → 実装 → レビュー → コミット & マージ` が独立したファイルとして保存される。各 track は専用ブランチ `track/<track-id>` 上で進む。

track 作業には `/adr:add <slug>` で ADR を作り `/track:adr2pr` で PR まで進める、という正規フローがある。

## 前提条件

このテンプレートを使うには以下が必要:

- **Docker + docker compose** — CI と開発用サービスはコンテナ内で実行される
- **Rust toolchain + cargo-make** — host で `cargo make bootstrap` を実行する
- **`bin/sotp` の入手** — 以下 2 経路のいずれか (詳細は「はじめ方」参照)
  - a. SoTOHE-core を clone → `sotp template export` を実行すると、出力ツリーに `bin/sotp` が移植された状態で完結する (タグ非依存、初回導入向け)
  - b. 更新時 / 別ホスト再導入時は `.harness/config/sotp-version.json` の固定タグから `cargo install` で導入する
- **Claude Code** — 主操作面。`/track:*` コマンドの入口
- **Codex CLI** — 既定 profile (`default`) のレビュー担当 (`reviewer`)
- **Gemini CLI** — 既定 profile (`default`) のリサーチ担当 (`researcher`)

補足:

- Linux で uid/gid が `1000:1000` 以外なら `HOST_UID=$(id -u)` / `HOST_GID=$(id -g)` を export してから compose wrapper を使う
- capability の担当者は `.harness/config/agent-profiles.json` で切り替えられる
- 出力ツリー内 `bin/sotp` を git 管理するかどうか (ignore / track) は利用者側の判断領域であり、本テンプレートは強制しない
- プレビルトバイナリ配布 (GitHub Releases) は現時点では実施していない (将来検討)

## はじめ方

### 初回セットアップ

以下は `sotp template export` で生成された出力ツリーでの初回セットアップ手順である。SoTOHE-core source repo 自体の `cargo make bootstrap` は開発用に `build-sotp` を実行する。

出力ツリーでの `bin/sotp` 入手には 2 経路がある。デフォルトの分岐条件はテンプレート利用者が意識せず自動で選ばれる (出力ツリーの `cargo make bootstrap` Step 3 で判定される):

- **経路 a (初回導入 / タグ非依存)**: SoTOHE-core を clone → build して `sotp template export` を出力ツリーに実行すると、実行中の自バイナリが出力ツリーの `bin/sotp` に移植される (実行権限保持のコピー)。出力ツリーで `cargo make bootstrap` を実行すると、Step 3 は `bin/sotp` が既に存在することを検出し、`install-sotp` を呼ばずに完結する。公開リポジトリに sotp タグが 1 本もなくてもこの経路は成立する。
- **経路 b (更新 / 別ホスト再導入)**: 出力ツリーに `bin/sotp` が存在しない (または起動に失敗する) 場合、Step 3 は `cargo make install-sotp` を呼び、`.harness/config/sotp-version.json` の固定タグから `cargo install --git ... --tag ... --locked` で `bin/sotp` を導入する。別ホストへの再導入や sotp バージョンの更新はこの経路で行う。

```bash
# 出力ツリーのターミナルで:
cargo make bootstrap      # Docker イメージビルド + bin/sotp 入手 (経路 a/b を自動判定) + CI 一括
```

```text
# Claude Code チャットで:
/track:catchup            # 環境確認 + プロジェクト状態把握
```

出力ツリーで `bin/sotp` を明示的に (再) 導入したい場合 (経路 b を強制したい場合など) は、bootstrap 前に `cargo make install-sotp` を単独で実行できる。移植バイナリはビルドしたホスト固有 (glibc / OS ABI 依存) なので、別ホストで動かない場合は経路 b で再導入する。

### 機能を開発する（正規フロー）

1. ADR（設計決定記録）を作成する

   ```text
   /adr:add <slug>
   ```

2. ADR をベースに track 初期化から PR レビューまで進める（merge はしない）

   ```text
   /track:adr2pr
   ```

   このコマンドは `/track:init` → ADR baseline の `/track:review` / `/track:commit` → `/track:spec-design` / `/track:type-design` / `/track:impl-plan` → 計画 artifact の review / commit → `/track:full-cycle` → `/track:pr-review` を順に実行し、PR を開いた状態で停止する。

### コマンドを個別に使う場合

```text
/track:plan <feature>         # 仕様 + 計画 + 型契約 + 実装計画（Phase 0-3）
/track:implement              # 対話型並列実装
/track:full-cycle <task>      # 自律実装（1 タスクを丸ごと任せる）
/track:review                 # 外部レビュアーによるレビュー
/track:commit <message>       # ガード付きコミット + git note
/track:pr                     # ブランチ push + PR 作成
/track:merge <pr>             # CI 通過後に PR をマージ
/track:done                   # 設定された base branch に戻り完了サマリー
```

`/track:status` はどの段階でも呼べる。

## 自由文での依頼例

`/track:*` コマンドを明示しなくても、Claude Code に自由な言葉で依頼できる。必要な情報を完全に整理してから渡す必要はない。分かる範囲だけ伝えれば、Claude Code が目的・制約・受け入れ条件・影響範囲を対話で整理する。

```text
認証機能を追加したい。どの /track:* コマンドから始めるべきか教えて
```

```text
注文検索 API を改善したい。必要なら計画から進めて
```

```text
この設計で進めてよいか確認したい
```

## ロードマップと関連ドキュメント

- ADR 索引: `knowledge/adr/README.md`
- 規約索引: `knowledge/conventions/README.md`
- エージェント設定: `.harness/config/agent-profiles.json`

## ライセンス

MIT OR Apache-2.0 のデュアルライセンス。
