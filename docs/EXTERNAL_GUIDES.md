# External Guides Policy

この文書は、長文の外部ドキュメントをこのプロジェクトへ安全に取り込み、
コンテキスト消費を抑えながら運用するための方針をまとめたものです。

索引の真実の源泉:

- `docs/external-guides.json`

原文キャッシュの保存先:

- `.cache/external-guides/`

## 1. 目的

対象例:

- PostgreSQL ガイドライン
- クラウド設計ガイド
- セキュリティ標準
- 運用チェックリスト
- ORM / SQL / migration の深いベストプラクティス文書

実際の登録対象は `docs/external-guides.json` を真実の源泉とする。

狙い:

- 有用な外部知識を使えるようにする
- ただし本文は git に含めない
- AI が毎回長文本文を読まずに済むようにする
- 実際に採用した判断だけをプロジェクト固有ルールへ落とし込む

## 2. 基本方針

1. 外部ドキュメントの本文は git 管理しない
2. git 管理するのは索引と運用ルールだけにする
3. AI / 開発者はまず索引を読む
4. 要約で足りない時だけローカルキャッシュ済み本文を読む
5. プロジェクトで採用した内容は `track/` や設計文書へ要約して反映する
6. 原文の長文転載は避け、出典とライセンスを明記する

## 3. 運用フロー

### 導入時

```bash
cargo make guides-setup
cargo make guides-list
cargo make guides-usage
cargo make guides-fetch <guide-id>
```

新しい外部ガイドを索引へ追加する場合は、Claude Code から `/guide:add` を使う。
不足している項目だけを確認し、正規化した `id` / `raw_url` / `cache_path` を提案した上で
`docs/external-guides.json` を更新する想定とする。

### 利用時

1. `docs/external-guides.json` の要約・トリガー語・用途を見る
2. 関連性が高い時だけ `.cache/external-guides/...` の本文を読む
3. 採用した判断を `track/spec` / `track/plan` / 設計文書に要約で残す

`/track:plan`, `/track:implement`, `/track:full-cycle` では、入力プロンプトに加えて
最新トラックの `spec.md` / `plan.md` も走査し、`trigger_keywords` に一致した場合は
対応ガイドの `summary` / `project_usage` が Claude Code の追加コンテキストへ自動注入される。

### キャッシュ削除

```bash
cargo make guides-clean              # 全キャッシュ削除
cargo make guides-clean <guide-id>   # 個別キャッシュ削除
```

### 更新時

1. `docs/external-guides.json` の索引情報を更新する
2. 追加するガイドには `id`, `source_url`, `raw_url`, `license`, `cache_path`, `trigger_keywords`, `summary`, `project_usage` を揃えて記述する
3. `cargo make guides-list` と `cargo make guides-usage` で表示内容を確認する
4. 必要なら `cargo make guides-fetch <guide-id>` でローカルキャッシュを再取得する
5. 採用済みルールに変更が出る場合は関連文書も更新する

## 4. コンテキスト節約ルール

AI が読む順番:

1. `docs/external-guides.json`
2. この `docs/EXTERNAL_GUIDES.md`
3. 必要時のみローカルキャッシュ本文

避けること:

- 毎回本文全体を読む
- 本文の丸ごと要約を都度チャットへ貼る
- 原文をそのまま `track/` 文書へコピーする

推奨:

- 索引の `summary` と `project_usage` を先に判断材料に使う
- 必要箇所だけ読む
- 採用結論だけをプロジェクト固有ルールとして残す

補足:

- `.cache/external-guides/` は raw guide の fallback 用キャッシュであり、他のビルド/ツール系キャッシュと違って blanket deny の対象にはしない
- ただし、索引と要約で足りる限り raw guide を開かない運用を維持する

## 5. 著作権とライセンス

- 原文取得前に出典とライセンスを `docs/external-guides.json` に記録する
- 原文はローカルキャッシュとして保持し、git には含めない
- AI 応答や設計文書では長文引用を避け、要約・解釈・適用方針に変換する

## 6. 役割分担

- `docs/external-guides.json`
  - 外部ドキュメント索引の真実の源泉
- `docs/EXTERNAL_GUIDES.md`
  - 運用方針と参照手順
- `.cache/external-guides/`
  - ローカルキャッシュ済み原文
- `track/` / `.claude/docs/`
  - 実際に採用した判断の反映先

## 7. 反映先

外部ガイドを参照した後、必要なら次へ反映する:

- `track/tech-stack.md`
- `track/items/<id>/spec.md`
- `track/items/<id>/plan.md`
- `.claude/docs/DESIGN.md`

外部ガイドは「参照元」であり、最終的なプロジェクト判断はこのリポジトリ内文書へ落とし込む。
