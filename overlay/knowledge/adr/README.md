---
adr_id: adr-readme-index
decisions: []
---
# Architecture Decision Records (ADR)

このディレクトリは設計判断の記録を管理する。

## 運用ルール

- **フォーマット**: Nygard 式 + Rejected Alternatives + Reassess When
- **言語**: 日本語
- **採番**: `YYYY-MM-DD-HHMM-slug.md`（例: `<date>-<time>-<slug>.md`）
- **front-matter**: MD body の前に `adr_id` と `decisions[]` を必須で置く。各 decision は根拠 ref と decision 単位の `status` を持つ。
- **decision status**: `proposed` / `accepted` / `implemented` / `superseded` / `deprecated`。`implemented` には `implemented_in`、`superseded` には `superseded_by` が必須。
- **根拠**: 新規 decision には `user_decision_ref` または `review_finding_ref` を入れる。file-level の `## Status` は使用しない。

## ADR テンプレート

```markdown
---
adr_id: "<YYYY-MM-DD-HHMM>-<slug>"
decisions:
  - id: decision-1
    user_decision_ref: "chat_segment:<session>:<date>"
    status: proposed
---
# {タイトル}

## Context

{なぜこの判断が必要だったか}

## Decision

{何を選んだか}

## Rejected Alternatives

- {選択肢B}: {却下理由}
- {選択肢C}: {却下理由}

## Consequences

- Good: {良い影響}
- Bad: {悪い影響・トレードオフ}

## Reassess When

- {前提が変わる条件}
```

## ADR と Convention の関係

| | ADR | Convention |
|---|---|---|
| 問い | 「なぜこうした？」 | 「これからどうする？」 |
| 時制 | 過去形（あの時点で判断した） | 現在形（今後はこうせよ） |
| 寿命 | 永続（superseded でも残る） | 現行ルールのみ有効 |
| 例 | 「PostgreSQL を選んだ。理由は...」 | 「永続化は Repository port 経由で行え」 |

Convention に `## Decision Reference` セクションを追加し ADR にリンクする。

## 索引

ADR を作成したらこの索引に追記する。テーマ別にセクションを分けてよい。

| ADR | Status | Date |
|-----|--------|------|
| （まだ ADR はありません） | — | — |
