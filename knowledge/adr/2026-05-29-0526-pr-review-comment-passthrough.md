---
adr_id: 2026-05-29-0526-pr-review-comment-passthrough
decisions:
  - id: D1
    user_decision_ref: "chat:2026-05-29:pr-review-parsing-redesign"
    candidate_selection: "from:[A,B] chose:B"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-05-29:pr-review-parsing-redesign"
    candidate_selection: "from:[latest-round-only,all-rounds-uniq,head-commit-only] chose:latest-round-only"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-05-29:pr-review-parsing-redesign"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-05-29:pr-review-parsing-redesign"
    status: proposed
---
# PR レビュー結果を解釈せず、最新ラウンドのコメントをそのまま agent に渡す

## Context

`/track:pr-review` は Codex Cloud の `@codex review` を起動し、その結果を Rust 側
（`classify_severity` / `parse_body_findings` / `actionable_count` / `passed`）で
解釈して pass/fail を判定している。実際の PR の応答と突き合わせると、この「厳格な解釈」が
壊れていることが分かった。

- **定型文を finding として拾う**: Codex のレビュー本文（review.body）は毎回ほぼ同一の
  定型文で、その中に「Reviews are triggered when you」に続く箇条書き
  （`- Open a pull request for review` など）が含まれる。`parse_body_findings` は箇条書き行を
  finding として抽出し、`classify_severity` がすべて `P1`（actionable）に分類するため、
  本文だけで毎回 phantom finding が発生する。結果として `actionable_count` が 0 にならず、
  `state == "COMMENTED"` のレビューは**構造的にぜったい PASS にならない**（PASS できるのは
  bot の「指摘なし」issue comment 経路だけ）。
- **箇条書き以外を取りこぼす**: `parse_body_findings` は `-` / `*` / `+` / `1.` / `•` 行しか
  finding 扱いしない。実際の指摘が見出しや太字（`**![P1 Badge] タイトル**`）形式だと本文から
  何も拾えない。
- **本物の指摘は inline review comment 側にある**: 実際の findings は inline review comment
  （`path` + `line`/`start_line` + 本文）として付く。review.body はほとんどの場合定型文だが、
  まれに本文側に指摘が書かれることもある。
- **ラウンドが累積する**: `@codex review` を再投稿するたびに review と inline comment が増える。
  全部を素朴に集めると同じ指摘が重複して agent に渡る。

問題の本体は「Codex の出力を Rust 側で解釈・分類・判定する」という前提そのものにある。
Codex の本文書式やボイラープレートは外部都合で変わり、ハードコードした抽出・分類ルールは
追従できない。指摘の取捨選択は文脈を読める agent に任せるべきで、Rust 側はコメントを
集めて渡す運搬役に徹するのが頑健である。

関連コード: `libs/usecase/src/pr_review.rs`、`apps/cli-composition/src/pr/poll.rs`
（`parse_review` / `poll_review_for_cycle`）。

## Decision

### D1: Rust 側の「解釈」を撤去し、コメントをそのまま agent に渡す

PR レビュー結果に対する Rust 側の解釈ロジック（`classify_severity` による severity 分類、
`parse_body_findings` による箇条書き finding 抽出、`actionable_count` 集計、
review-found 経路の `passed` 判定）を撤去する。`sotp pr review-cycle` は Codex の
レビュー内容を sanitize して**そのまま出力**し、どれが actionable かの判断は呼び出し側の
agent（`/track:pr-review`）に委ねる。`sanitize_text` / `parse_paginated_json` /
Codex bot 判定 / ポーリングは引き続き流用する。

### D2: 取得対象は最新レビュー1ラウンドのみ（重複排除）

`@codex review` の再投稿でレビューと inline comment がラウンドごとに累積するため、
**最新の Codex レビュー1件とその inline comments だけ**を出力対象にする。これにより
「いま開いている指摘」の現状が重複なく得られる。Codex はラウンドごとに開いている指摘を
全件再掲する挙動なので、最新ラウンドが現状の全指摘を表す。

### D3: review.body は捨てず inline comments と合わせて渡す

review.body は定型文であることが多いが、まれに本文側に本物の指摘が書かれるため、
**review.body を捨てない**。最新レビューの review.body（定型文を含む）と、その review に
紐づく inline comments（`path:line` + 本文）の両方を sanitize して出力する。定型文の
取捨選択は agent が行う。

### D4: zero-findings シグナルの検出は維持する

bot の「指摘なし」シグナル（`@codex review` への 👍 reaction、または
"Didn't find any major issues" issue comment）の検出は維持し、これは明確な PASS として
扱う。zero-findings は機械的に判定できる確実なシグナルであり、agent に渡すまでもない。

## Rejected Alternatives

### A. 厳格パースを残し、定型文だけ除外フィルタで弾く

ボイラープレート文面を除外リストに登録して `parse_body_findings` の phantom finding を
防ぐ案。却下理由: Codex の本文・書式は外部都合で変わり、除外リストが追従できず取りこぼし・
誤検出が再発する。指摘の解釈責務を Rust に持たせる前提自体が脆く、根本解決にならない。

### B. 全ラウンドを内容で重複排除して全部出す

全レビューの inline comments を `path` + `line` + 正規化本文で uniq して出力する案。
却下理由: 修正済みで再投稿されていない古いラウンドの指摘も残り、agent にノイズを渡す。
「いま開いている指摘」の現状を表さない。（D2 で latest-round-only を採用）

### C. 現 HEAD commit に紐づくコメントのみ

`commit_id == 現 HEAD` のレビューだけを対象にする案。却下理由: HEAD が進んでレビュー
未実施の状態だと何も出せず、直近の有効なレビューを取りこぼす。最新ラウンド基準のほうが
運用上頑健。（D2 で latest-round-only を採用）

### D. review.body を定型文として丸ごと捨てる

review.body は定型文なので出力から除外する案。却下理由: まれに本文側に本物の指摘が
書かれるため、捨てると取りこぼす。（D3 で保持を採用）

## Consequences

### Positive

- COMMENTED レビューが構造的に PASS 不能だった不具合が解消する。
- 箇条書き以外の書式の指摘を取りこぼさなくなる。
- 壊れやすい解釈・分類ロジックが消え、外部書式変更に対する保守負担が下がる。
- agent が本文と inline コメントを文脈ごと読んで actionable 判断できる。

### Negative

- review-found 経路の pass/fail 機械判定がなくなり、actionable 判断が agent 依存になる
  （zero-findings は D4 で機械判定を維持）。
- 定型文も渡るため agent のトークン消費が若干増える。
- 重複排除が「最新ラウンド」前提のため、将来 Codex が1レビュー内で指摘を分割し最新ラウンドに
  全件再掲しなくなると取りこぼす可能性がある（現状の Codex はラウンドごとに全件再掲するため
  問題にならない）。

### Neutral

- `sanitize_text` / `parse_paginated_json` / Codex bot 判定 / ポーリング・recovery は流用する。

## Reassess When

- Codex Cloud が findings を構造化データ（API フィールドや機械可読フォーマット）で
  提供するようになったとき。
- Codex が1レビュー内で指摘を分割し、最新ラウンドに全件再掲しない挙動に変わったとき。
- `pr-reviewer` provider に codex 以外（structured 出力を持たない provider）を採用するとき。

## Related

- `knowledge/adr/` — ADR 索引
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR ライフサイクルと配置ルール
