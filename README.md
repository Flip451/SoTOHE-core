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
| **仕様書** | `spec.json` | track ごとに作成される要件書 |
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

SoT はリポジトリに保存する正本ファイルであり、ADR は Markdown、track の仕様・型契約は JSON、実装は Rust ソースである。`spec.md` は `spec.json` から、`plan.md` は `metadata.json` から `bin/sotp track views sync` が生成する読み取り専用ビューで、SoT ではない — 直接編集しても次の同期で上書きされる。

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

track 作業には `/adr:add <slug>` で ADR を作り
`/track:adr2pr` で PR まで進める、という正規フローがある。feature 名と主 ADR の引数は任意で、省略した値は会話文脈から解決して user が 1 回確認する（明示指定時はその値が優先される）。`/track:init` は orchestrator が選んだ主 ADR の逐語 baseline を記録する。現在の ADR 文面とその baseline とのバイト照合を行うのは commit と track-aware CI であって、review ではない — Phase 0 で ADR が baseline と食い違っているのは収束中の正常な draft 状態であり、review を止めない。

## 前提条件

このテンプレートを使うには以下が必要:

- **Rust toolchain + cargo-make** — `rust-toolchain.toml` が Rust / rustfmt / clippy を固定する。未初期化の export ツリーでは host で `cargo make init` を実行する
- **Docker（任意）** — 既定の品質ゲートと CI は host toolchain で実行する。隔離環境を選ぶ場合だけ `Makefile.toml` の extend 参照先を `Makefile.docker.toml` に切り替える
- **`bin/sotp` の入手** — 以下 2 経路のいずれか (詳細は「はじめ方」参照)
  - a. SoTOHE-core を clone → `sotp template export` を実行すると、出力ツリーに `bin/sotp` が移植された状態で完結する (タグ非依存、初回導入向け)
  - b. 更新時 / 別ホスト再導入時は `.harness/config/sotp-version.json` の固定タグから `cargo install` で導入する
- **Claude Code** — 主操作面。`/track:*` コマンドの入口
- **Codex CLI** — 既定 profile (`default`) のレビュー担当 (`reviewer`) とリサーチ担当 (`researcher`)

補足:

- capability の担当者は `.harness/config/agent-profiles.json` で切り替えられる
- 出力ツリー内 `bin/sotp` を git 管理するかどうか (ignore / track) は利用者側の判断領域であり、本テンプレートは強制しない
- プレビルトバイナリ配布 (GitHub Releases) は現時点では実施していない (将来検討)

## 外部プロバイダーを capability 単位で設定する

外部プロバイダーを Codex の custom model provider として使う場合は、Codex の
`config.toml`（通常は `~/.codex/config.toml`、または `$CODEX_HOME/config.toml`）に
プロバイダーを定義し、`agent-profiles.json` の対象 capability に
`model_provider` としてその定義名を指定する。SoTOHE 側の `provider` は `codex` のままにし、
`model_provider` と `[model_providers.<id>]` の `<id>` を一致させる。API key は TOML に書かず、
`env_key` で参照する環境変数名を指定する。現在の Codex が custom provider に送る wire API は
Responses API のみであり、SoTOHE は Chat Completions への変換を行わない。

```toml
# $CODEX_HOME/config.toml

# Qwen (DashScope): Responses API の接続先を使う実行可能な例。
[model_providers.qwen]
name = "Qwen (DashScope)"
base_url = "https://dashscope-intl.aliyuncs.com/compatible-mode/v1"
env_key = "DASHSCOPE_API_KEY"
wire_api = "responses"

# DeepSeek: Responses API の接続先を使う実行可能な例。
[model_providers.deepseek]
name = "DeepSeek"
base_url = "https://api.deepseek.com"
env_key = "DEEPSEEK_API_KEY"
wire_api = "responses"

# GLM (Z.ai) の公式 OpenAI 互換 endpoint は Chat Completions 経路であり、
# 現在の Codex の Responses-only custom provider には直結できない。
# 次のブロックは設定形状を示す参考（実行可能な設定ではない）。
#
# [model_providers.glm]
# name = "GLM (Z.ai)"
# base_url = "https://api.z.ai/api/coding/paas/v4"
# env_key = "ZAI_API_KEY"
# wire_api = "responses" # endpoint が Chat Completions のため使用不可
```

Responses API 対応 provider を使う generic `orchestrator-output` capability の設定は次のような
JSON 断片になる。これはテンプレートに実在する `spec-designer` の完全な entry であり、モデル名は
契約している provider のモデル ID に置き換える。`model_provider` の pass-through はこの generic
dispatch 経路だけが行う。

```json
{
  "capabilities": {
    "spec-designer": {
      "provider": "codex",
      "model": "qwen3.7-plus",
      "model_provider": "qwen",
      "reasoning_effort": "high",
      "execution_mode": "orchestrator-output"
    }
  }
}
```

`reviewer` や `review-fix-lead` など `execution_mode = "typed-pipeline"` の capability は専用の
固定出力経路を持ち、`model_provider` を generic dispatch のようには転送しない。そのため、
typed-pipeline の provider gate は `codex` のままであり、外部 provider への routing を設定で
有効化できない。DeepSeek、GLM (Z.ai)、Qwen (DashScope) の typed-pipeline capability としての
verdict envelope / structured output 準拠も未検証である。この未検証状態は、接続経路が実際に
選択されることを意味しない。provider ごとの status と更新条件は、次の表で管理する。

### typed-pipeline の検証ステータス

`typed-pipeline` の provider gate が `codex` のまま維持されることと、外部 provider が
typed-pipeline の出力契約に準拠することは別の判定である。現時点では、次の provider は
すべて **未検証** として扱う。

| 外部 provider | typed-pipeline 準拠 | status の扱いと、別に扱う既知の制約 |
|---|---|---|
| DeepSeek | 未検証 | provider 固有の実行で `verdict envelope` と `structured output` の両方が受理されるまで未検証。 |
| GLM (Z.ai) | 未検証 | Chat Completions / Responses の互換性とは別に、typed-pipeline 準拠は未検証。 |
| Qwen (DashScope) | 未検証 | Anthropic 互換経路の system-message gap とは別に、typed-pipeline 準拠は未検証。 |

ここで `検証済み` に更新できるのは、対象 provider を使った typed-pipeline の実行が実際に
成功し、`verdict envelope` と `structured output` の両方を pipeline が受理した場合だけである。
`config.toml` の接続確認、通常の会話応答、Anthropic 互換 subprocess の動作確認だけでは
検証済みとはしない。検証結果が得られたら、この README の表の該当行を provider 単位で更新する。
`agent-profiles.json` の `model_provider` 設定や `sotp` の provider gate は、この status を自動更新しない。

GLM (Z.ai) を Codex の custom provider として直接使うことは、現行の公式 OpenAI 互換 endpoint
（Chat Completions）と Codex の Responses-only 制約が一致しないため未対応である。
`wire_api = "chat"` に変更しても現行 Codex は受け付けない。GLM を使う場合は、下の
Anthropic 互換 subprocess 経路、または consumer が管理する Responses 変換 gateway を選ぶ。
GLM の OpenAI 互換 endpoint と Anthropic 互換 endpoint、DashScope の地域別 endpoint は別物
なので、経路を混同しない。

### Anthropic 互換経路の注意

Anthropic 互換 endpoint は Codex の `[model_providers.*]` ではなく、Claude 互換の
subprocess を起動するときに使う。`ANTHROPIC_BASE_URL` と認証トークンは、次のように
対象プロセスだけへ注入する。

```bash
env \
  ANTHROPIC_BASE_URL="https://api.z.ai/api/anthropic" \
  ANTHROPIC_AUTH_TOKEN="$ZAI_API_KEY" \
  claude
```

代表的な Anthropic 互換 endpoint は次のとおりである（利用地域の URL を優先する）。

| provider | Anthropic 互換 `base_url` の例 |
|---|---|
| DeepSeek | `https://api.deepseek.com/anthropic` |
| GLM (Z.ai) | `https://api.z.ai/api/anthropic` |
| Qwen (DashScope) | `https://dashscope-intl.aliyuncs.com/apps/anthropic` |

`ANTHROPIC_BASE_URL` をシェル全体へ `export` して一括 redirect しないこと。per-subprocess
注入だけが対象 provider の経路を変更し、`delegate-in-host` の経路は redirect されない。
また、Qwen の Anthropic 互換経路には、Claude Code の新しい会話フローが会話途中に送る
system message を拒否する既知の gap がある。会話途中の system message を必要とする
処理では、この経路を互換性があるものとして扱わない。

endpoint とモデル ID は provider 側で更新されるため、利用前に [Codex の設定リファレンス](https://developers.openai.com/codex/config-reference)、[DeepSeek API ドキュメント](https://api-docs.deepseek.com/)、
[Z.AI のツール連携ドキュメント](https://docs.z.ai/devpack/tool/others)、[DashScope の base URL 一覧](https://help.aliyun.com/en/model-studio/base-url)
を確認する。

## はじめ方

### 初回セットアップ

以下は `sotp template export` で生成された出力ツリーでの初回セットアップ手順である。まず `cargo-make` を host に導入し、固定 toolchain 上で `init` を実行する。

出力ツリーでの `bin/sotp` 入手には 2 経路がある。デフォルトの分岐条件はテンプレート利用者が意識せず自動で選ばれる (`cargo make init` が内部で実行する `cargo make bootstrap` の Step 3 で判定される):

- **経路 a (初回導入 / タグ非依存)**: SoTOHE-core を clone → build して `sotp template export` を出力ツリーに実行すると、実行中の自バイナリが出力ツリーの `bin/sotp` に移植される (実行権限保持のコピー)。出力ツリーで `cargo make init` を実行すると、Step 3 は `bin/sotp` が既に存在することを検出し、`install-sotp` を呼ばずに完結する。公開リポジトリに sotp タグが 1 本もなくてもこの経路は成立する。
- **経路 b (更新 / 別ホスト再導入)**: 出力ツリーに `bin/sotp` が存在しない (または起動に失敗する) 場合、Step 3 は `cargo make install-sotp` を呼び、`.harness/config/sotp-version.json` の固定タグから `cargo install --git ... --tag ... --locked` で `bin/sotp` を導入する。別ホストへの再導入や sotp バージョンの更新はこの経路で行う。

```bash
# 出力ツリーのターミナルで:
cargo install --locked cargo-make --version 0.37.24
cargo make init           # Git 初期化 + lockfile / 初回 commit + bootstrap (aux tools / bin/sotp / host CI)
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
   /track:adr2pr [<feature>] [--primary-adr <filename>.md]
   ```

   引数は両方とも任意で、省略した値は会話文脈から解決され、`/track:init` へ渡す前に user が 1 回確認する（候補が複数なら選択を、候補がなければ値の直接指定を求める）。明示指定時はその値が優先される。このコマンドは `/track:init`（主 ADR を designation する init baseline 記録を含む）→ ADR baseline の `/track:review` / `/track:commit` → `/track:spec-design` / `/track:type-design` / `/track:impl-plan` → 計画 artifact の review / commit → `/track:full-cycle` → `/track:pr-review` を順に実行し、PR を開いた状態で停止する。必要な刻印の欠落は block される — init designation が 1 件も無ければ review が、`spec.json` が cite した ADR の coverage が欠ければ commit と PR CI が止める。現在の ADR 文面と baseline との byte 不一致を block するのは commit と PR CI だけで、review は止めない。

   Phase 0 では、user が収束した ADR 文面を承認した後にその文面が修正される場合、以前の承認は修正後の文面に引き継がれない。workflow は承認前の収束 loop に戻り、findings を収束させた修正後の全文を user へ再提示して再承認を得てから裁定境界を閉じる。詳細な規範は `.harness/policies/pre-track-adr-authoring.md` を参照する。

### コマンドを個別に使う場合

```text
/track:plan <feature>         # 仕様 + 計画 + 型契約 + 実装計画（Phase 0-3）
/track:implement              # 対話型並列実装
/track:full-cycle             # 自律実装（引数なし。feature バッチ単位で回し、
                              #   scope ごとの diff 上限を超えるときだけタスクを分割する）
/track:review                 # 外部レビュアーによるレビュー
/track:commit <message>       # ガード付きコミット + git note
/track:pr                     # ブランチ push + PR 作成
/track:merge <pr>             # CI 通過後に PR をマージ
/track:done                   # 設定された base branch に戻り完了サマリー
```

個別に review を起動する際、orchestrator は Phase 0 で init snapshot を刻印することで primary ADR source を
designation し、その ledger init record 自体が唯一の designation record になる。review wrapper が検証するのは
2 点だけである — active track にその init record 群が 1 件以上あること、および台帳に記録された全 ledger copy が
実在して記録 hash と一致すること (台帳自体の健全性)。現在の ADR 文面と最新 baseline との byte 照合はここでは
行わない。それを行うのは commit gate と track-aware CI で、`spec.json` が cite した ADR の coverage も同じく
commit gate 側で検証する。直接の
`bin/sotp adr-baseline check-review` 呼び出しだけは `--primary-source <file>.md` を明示 override に使える。
baseline は手で編集せず、必要な snapshot / restore は `bin/sotp adr-baseline` の専用コマンドを使う。

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
