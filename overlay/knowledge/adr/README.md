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
- **Status**: `Proposed` / `Accepted` / `Superseded` / `Deprecated`
  - `Proposed`: ADR is authored and under review / pending activation of the associated track
  - `Accepted`: Decision is accepted and implementation may proceed
  - `Superseded`: Replaced by a newer ADR (reference the superseding ADR)
  - `Deprecated`: Decision is withdrawn without replacement
- **Superseded の場合**: 新 ADR を作成し、旧 ADR の Status を `Superseded by YYYY-MM-DD-HHMM-slug.md` に変更

## ADR テンプレート

```markdown
# {タイトル}

## Status

Proposed / Accepted / Superseded / Deprecated

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
