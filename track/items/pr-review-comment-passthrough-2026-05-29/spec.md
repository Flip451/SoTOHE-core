<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 35, yellow: 0, red: 0 }
---

# PR レビュー結果を解釈せず最新ラウンドのコメントを agent に渡す

## Goal

- [GO-01] `sotp pr review-cycle` が Codex Cloud のレビュー内容を Rust 側で解釈・分類・判定せず、sanitize したまま呼び出し元の agent に渡す「運搬役」に徹することで、外部書式変更に追従できない壊れやすい解釈ロジックを撤去し、COMMENTED レビューが構造的に PASS 不能だった不具合を解消する [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1]
- [GO-02] 最新の Codex レビュー1ラウンドのみを対象にすることで、`@codex review` の再投稿で累積した過去ラウンドの重複指摘を排除し、「いま開いている指摘」の現状を重複なく agent に届ける [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D2]

## Scope

### In Scope
- [IN-01] `classify_severity` 関数と `parse_body_findings` 関数の撤去: これらは Rust 側の「解釈」ロジックの中核であり D1 に従い削除する [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T002]
- [IN-02] `PrReviewFinding` 型と `PrReviewResult` 型の縮小: 現行の型が持つ D1 の解釈責務に由来するフィールド（`PrReviewFinding` の `severity` / `rule_id`、`PrReviewResult` の `actionable_count` / `passed`）を除去する。`PrReviewResult.findings` フィールドは維持するが、意味を変更する — 解釈済みの findings リストから、最新レビューの inline comments（`path` + `line` + sanitized `body`）を格納する passthrough コンテナへ。この意味変更は IN-05 および IN-03 と整合する [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T001]
- [IN-03] 最新 Codex レビュー1件とその inline comments の取得: `poll_review_for_cycle` が返す `ReviewFound` ペイロードは最新ラウンド1件（最新 `submitted_at` で選択）に限定する（D2） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D2] [tasks: T003]
- [IN-04] review.body の sanitize と出力: 最新レビューの `review.body`（定型文含む）を `sanitize_text` で処理し agent に渡す。body を捨てない（D3） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003, T005]
- [IN-05] inline review comments の sanitize と出力: 最新レビューに紐づく inline comments（`path` + `line`/`start_line` + 本文）を `sanitize_text` で処理し、`path:line` 位置情報とともに agent に渡す（D3） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003, T005]
- [IN-06] zero-findings シグナルの維持: bot の 👍 reaction または「Didn't find any major issues」issue comment を機械的に検出し、PASS として出力する。この検出は Rust 側で行い agent に委ねない（D4） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D4] [tasks: T004, T005]
- [IN-07] `sanitize_text` / `parse_paginated_json` / Codex bot 判定 / ポーリング・recovery の流用: これらは撤去対象でなく維持する（D1 Neutral Consequences） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T003]
- [IN-08] `parse_review` ヘルパーの改修: `parse_review` は解釈・分類ロジックを呼ばず、review.body と inline comments を sanitize して返す形式に切り替える。`format_review_summary` も新しい出力形式に合わせて更新する [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1, knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003]
- [IN-09] `/track:pr-review` コマンド定義（`.claude/commands/track/pr-review.md`）の更新: 解釈ロジックに依存した「P0/P1 finding count」「zero actionable findings」の表現を、「agent が判断する」新しい振る舞いに合わせて改訂する [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T006]

### Out of Scope
- [OS-01] 「厳格パースを残し定型文だけ除外フィルタで弾く」案（Rejected Alternative A）: Codex の書式は外部都合で変わり除外リストが追従できず根本解決にならない [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T002, T003]
- [OS-02] 全ラウンドを内容で重複排除して全部出す案（Rejected Alternative B）: 修正済みで再投稿されていない古い指摘も残りノイズになる [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D2] [tasks: T003]
- [OS-03] 現 HEAD commit に紐づくコメントのみを対象にする案（Rejected Alternative C）: HEAD が進んでレビュー未実施の状態だと有効なレビューを取りこぼす [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D2] [tasks: T003]
- [OS-04] review.body を定型文として丸ごと捨てる案（Rejected Alternative D）: まれに本文側に本物の指摘が書かれるため捨てると取りこぼす [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003]
- [OS-05] zero-findings を agent に判断させること: 👍 reaction と「Didn't find any major issues」コメントは機械的に判定できる確実なシグナルであり Rust 側で判定する（D4） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D4] [tasks: T004, T005]
- [OS-06] `validate_reviewer_provider` / `parse_paginated_json` / `sanitize_text` / Codex bot 判定の変更、およびポーリング再試行・recovery ロジック自体の変更: これらは本トラックで変更しない（D1 Neutral Consequences）。なお `poll_review_for_cycle` 内の最新ラウンド選択ロジックは IN-03 の対象として変更する（D2） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T001, T002]
- [OS-07] Codex Cloud が findings を構造化データで提供するようになった場合の再設計: ADR Reassess When 節に記載されており、現段階では対象外 [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T001, T002, T003]

## Constraints
- [CN-01] review-found 経路での pass/fail 機械判定を行わない: `actionable_count` を集計したり `passed` フラグを計算したりする実装を禁止する。review-found の出力は sanitize 済みのレビュー内容（review.body と inline comments）のみとし、actionable 判断は agent に委ねる（D1・D3） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T003, T004]
- [CN-02] 最新ラウンド選択は `submitted_at` タイムスタンプで行い、すべての Codex bot レビューの中から最新1件を選ぶ: 全ラウンドの出力や HEAD commit 基準のフィルタリングは行わない（D2） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D2] [tasks: T003]
- [CN-03] `sanitize_text` の適用は維持する: review.body および各 inline comment の本文は出力前に必ず `sanitize_text` を通し、秘密情報・絶対パス・ローカルホスト URL・RFC 1918 IP を除去する（D3） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003, T005]
- [CN-04] zero-findings 判定のみ Rust 側で行い機械的 PASS を維持する: 👍 reaction および「Didn't find any major issues」issue comment による zero-findings 検出は D4 に従い撤去しない [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D4] [tasks: T004, T005]
- [CN-05] usecase 層（`libs/usecase/`）は I/O・`println!`・`eprintln!` を含まない純粋な関数として実装する: hexagonal-architecture convention のユースケース層純粋性ルールに従う [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [conv: knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules] [tasks: T001, T002]

## Acceptance Criteria
- [ ] [AC-01] `libs/usecase/src/pr_review.rs` に `classify_severity` 関数と `parse_body_findings` 関数が存在しない [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T002]
- [ ] [AC-02] `PrReviewResult`（または後継の出力型）に `actionable_count` フィールドと `passed` フィールド（review-found 経路分）が存在しない [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T001]
- [ ] [AC-03] `sotp pr review-cycle` が ReviewFound 経路で返す出力に、sanitize 済みの review.body が含まれる [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003, T005]
- [ ] [AC-04] `sotp pr review-cycle` が ReviewFound 経路で返す出力に、最新レビューに紐づく各 inline comment の sanitize 済み本文と `path:line` 位置情報が含まれる [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003, T005]
- [ ] [AC-05] 複数の Codex レビューラウンドが存在する場合、`sotp pr review-cycle` は `submitted_at` が最新の1件のみを出力対象とする（古いラウンドの inline comments を出力しない） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D2] [tasks: T003, T005]
- [ ] [AC-06] bot の 👍 reaction が trigger 以降に存在する場合、`sotp pr review-cycle` は `zero_findings` シグナルを出力し PASS とする（D4 維持） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D4] [tasks: T004, T005]
- [ ] [AC-07] bot の「Didn't find any major issues」issue comment が trigger 以降に存在する場合、`sotp pr review-cycle` は `zero_findings` シグナルを出力し PASS とする（D4 維持） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D4] [tasks: T004, T005]
- [ ] [AC-08] review.body と inline comments の出力に `sanitize_text` が適用されており、秘密情報・絶対パス・ローカルホスト URL・RFC 1918 IP が `[REDACTED]` / `[PATH]` / `[INTERNAL]` / `[INTERNAL_IP]` に置換されている [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1, knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D3] [tasks: T003, T005]
- [ ] [AC-09] state が `COMMENTED` の Codex レビューが存在する場合、`sotp pr review-cycle` は FAIL を返さず ReviewFound としてコメントを出力する（COMMENTED が構造的に PASS 不能だった不具合の解消） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T004, T005]
- [ ] [AC-10] `sanitize_text` / `parse_paginated_json` / Codex bot 判定 / ポーリング・recovery の既存テストがすべて pass する（流用機能の退行がない） [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T001, T002, T003, T004, T005]
- [ ] [AC-11] `.claude/commands/track/pr-review.md` が改訂されており、「P0/P1 finding count で pass/fail を判定する」旨の記述が除去され、ReviewFound の場合はコメントを agent が判断する旨に更新されている [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T006]
- [ ] [AC-12] `cargo make ci` の全項目（fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-05-29-0526-pr-review-comment-passthrough.md#D1] [tasks: T001, T002, T003, T004, T005, T006]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Usecase Layer Purity Rules
- knowledge/conventions/hexagonal-architecture.md#Adapter Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 35  🟡 0  🔴 0

