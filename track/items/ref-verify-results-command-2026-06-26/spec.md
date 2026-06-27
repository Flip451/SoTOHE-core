<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 36, yellow: 0, red: 0 }
---

# `sotp ref-verify results` で verify-cache 直読みを置き換える

## Goal

- [GO-01] `bin/sotp ref-verify results` サブコマンドを新設し、ref-verify レーンの結果確認を verify-cache JSON 直読みから CLI に一本化する。caller (skill / agent briefing / user) が cache スキーマを意識せず CLI 出力のみで判定結果を取得できるようにし、`bin/sotp review results` / `bin/sotp dry results` と同等の読み取り面を sotp 配下の verdict-bearing サブコマンド全体に揃える [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D1]
- [GO-02] `sotp ref-verify results` の出力面と option を `bin/sotp review results` / `bin/sotp dry results` と「誤読しにくさ」および「ref-verify が担う SoT Chain semantic verification の意味」を保ちながら方針整合させる。option の 1:1 棚卸しはせず、ref-verify に適用できるものだけを選択的に採用する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2]
- [GO-03] verify-cache の `SemanticVerifyEntry` に `claim_origin` / `evidence_origin` フィールドを追加し、判定済みの pair の source を artifact 種別 + 具体位置レベルで cache に保存する。`sotp ref-verify results` は pass/fail の cached pair について cache 読みだけで origin を表示でき、pending pair については current artifact から read-only で再導出して表示する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3]

## Scope

### In Scope
- [IN-01] `sotp ref-verify results` CLI サブコマンドの実装。option として `--track-id` (optional; 省略時はブランチから自動解決)、`--items-dir` (default: `track/items`)、`--chain {1|2|all}`、`--layer <name>|all`、`--filter {pass|fail|pending|all}` を持つ [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D1, knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T005, T006, T007]
- [IN-02] 出力先頭ブロック (Chain×Layer 集計): Chain① (spec↔ADR) と Chain② (各 layer × catalogue↔spec) を 1 行/lane で `pass=N fail=N pending=N` 形式で表示する。Chain② の layer 集合は `architecture-rules.json` から動的に解決する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T004, T005]
- [IN-03] 出力レコードブロック (filter 対象 pair の列挙): `--filter` に従って pass/fail/pending pair を列挙する。`--filter` 省略時は default 出力として fail/pending pair を列挙する。各 pair を `claim_hash / evidence_hash / verdict / reason / chain+layer` の形で表示し、claim/evidence の source (artifact 種別 + 具体位置: spec.json の section+id / ADR の file+anchor / catalogue の file+section_key+entry_key) を併記する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2, knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T003, T004, T005]
- [IN-04] fail pair の source 表示: verify-cache entry に persist された `claim_origin` / `evidence_origin` から artifact 種別と具体位置を表示する。artifact の再 parse は不要 [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T002, T004]
- [IN-05] pending pair (cache miss / 未確定 pair) の source 表示: `ref-verify run` と同じ pair source 解決を read-only で呼び、current artifact から pending pair と origin meta を再導出して表示する。LLM 呼び出しおよび verifier subprocess は起動しない [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T004]
- [IN-06] 出力末尾の Summary 行: `Summary: N pass, N fail, N pending, N total` を表示する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T004, T005]
- [IN-07] `SemanticVerifyEntry` schema の拡張: `claim_origin` / `evidence_origin` フィールドを追加する。Chain① pair: `claim_origin` は spec.json の `(section, requirement_id, text_label)`、`evidence_origin` は ADR の `(file_path, anchor=decision_id)`。Chain② pair: `claim_origin` は catalogue の `(file_path, section_key=types|traits|functions, entry_key)`、`evidence_origin` は spec.json の `(section, requirement_id, text_label)` [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T001, T002]
- [IN-08] verify-cache schema の旧バージョン (origin meta フィールドなし) を parse error として reject する。read-only の cache consumer (`results` / `check-approved`) は非 0 エラーとして返し、`bin/sotp ref-verify run` は旧 entry を有効 cache として再利用せず、新 schema の cache を再生成する。`#[serde(deny_unknown_fields)]` は維持する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T002]
- [IN-09] `/track:ref-verify` skill の Step 2 更新: `spec-adr-verify-cache.json` / `<layer>-catalogue-spec-verify-cache.json` を手で Read して failing pair を特定する手順を `sotp ref-verify results` 経由の手順に置き換える [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D1] [tasks: T008]

### Out of Scope
- [OS-01] `--format json` 出力: review/dry results のいずれも JSON 出力を提供していないため、本コマンドも採用しない (YAGNI)。需要が顕在化したとき別 ADR で追加できる [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2]
- [OS-02] `--limit` / `--round-type` option の採用: ref-verify に round 概念がないため `bin/sotp review results` の `--limit` / `--round-type` は適用しない [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2]
- [OS-03] writer (spec-designer / type-designer / adr-editor) への振り分けロジック: `results` は source 表示までに留める。failing pair をどの writer に差し戻すかは caller (skill / agent briefing) の責務 [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2]
- [OS-04] verify-cache 直読みを skill 本文で標準化する方針 (Rejected Alternative A): 旧来の `/track:ref-verify` skill Step 2 の「直接 Read」を継続し、caller ごとの読み方を skill で統一する案。cache schema 変更時に各所が壊れる構造のため却下 [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D1]
- [OS-05] `bin/sotp ref-verify run` の標準出力に per-pair 詳細を統合する方針 (Rejected Alternative B): run は verifier 呼び出しを伴う重い操作であり、read-only の `results` を別コマンドとして分ける設計が review/dry と整合する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D1]

## Constraints
- [CN-01] `sotp ref-verify results` は text 単一フォーマットのみを提供する。`--format json` は実装しない [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T005, T006]
- [CN-02] `sotp ref-verify results` の exit code は常に 0 (informational)。verdict gate は `bin/sotp ref-verify check-approved` の責務であり、`results` はゲートに使わない。review/dry results と同じポリシー [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T005, T006]
- [CN-03] `--track-id` を省略したとき、現在のブランチ名 `track/<id>` から必ず自動解決する。ブランチから解決できない場合はエラーとし、手動指定を要求する。手動指定も常にサポートする [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T006, T007]
- [CN-04] `--layer` option の有効な layer 名は `architecture-rules.json` から動的に解決する。ハードコードしない。Chain① (`--chain 1`) に `--layer` を指定した場合は no-op とする (Chain① は layer 粒度を持たない) [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T004, T005]
- [CN-05] 旧 verify-cache (origin meta フィールドなし) は後方互換を持たず parse error として reject する。verify-cache は track-local 実行ログであり `bin/sotp ref-verify run` の再実行で新 schema に再生成できる。旧 entry を silently forward-compatible に読む migration は不要 [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [conv: knowledge/conventions/no-backward-compat.md#Rules] [tasks: T002]
- [CN-06] failing pair / pending pair の writer への振り分けは caller の責務とする。`results` コマンドは source 情報 (artifact 種別 + 具体位置) を提示するにとどまり、writer の指定・エラーメッセージの生成・fix ループの管理は行わない [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T004, T005]

## Acceptance Criteria
- [ ] [AC-01] `sotp ref-verify results` コマンドが実行可能であり、先頭ブロック (Chain×Layer 集計) + レコードブロック (fail/pending pair) + Summary 行を含む text を stdout に出力する。exit code は 0 [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D1, knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T005, T006, T007]
- [ ] [AC-02] 先頭ブロックが Chain① (spec↔ADR) と Chain② (各 layer × catalogue↔spec) を 1 行/lane で表示し、各行が `pass=N fail=N pending=N` 形式を持つ。Chain② の layer 名が `architecture-rules.json` から解決されたものと一致する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T004]
- [ ] [AC-03] レコードブロックの fail pair が `claim_hash / evidence_hash / verdict / reason / chain+layer` + claim/evidence source (artifact 種別 + 具体位置) を表示する。source は cache entry の `claim_origin` / `evidence_origin` から取得し、artifact の再 parse を要しない [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2, knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T002, T004]
- [ ] [AC-04] レコードブロックの pending pair (cache miss) が `claim_hash / evidence_hash / verdict=pending / chain+layer` + claim/evidence source を表示する。source は current artifact から read-only で再導出し、LLM / verifier の呼び出しは発生しない [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2, knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T004]
- [ ] [AC-05] 末尾の Summary 行が `Summary: N pass, N fail, N pending, N total` 形式で正確な集計を出力する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T004, T005]
- [ ] [AC-06] `--track-id` を省略したとき、現在のブランチ名 `track/<id>` から track id が自動解決され、正しい track のデータが表示される [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T006, T007]
- [ ] [AC-07] `--chain 1` を指定すると Chain① のみが表示され (先頭ブロックに Chain① の 1 lane のみ)、`--chain 2` では Chain② のみ (layer 数分の lane)、`--chain all` (既定) では両 Chain が表示される [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T004, T005, T006, T007]
- [ ] [AC-08] `--layer <name>` を指定すると Chain② の指定 layer 分のみが先頭ブロックとレコードブロックに表示される。`--layer all` (既定) は全 layer を表示する。`--layer` を `--chain 1` と同時指定した場合は Chain① に layer 概念がないため no-op として扱われる [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T004, T005, T006, T007]
- [ ] [AC-09] `--filter` 省略時は fail/pending pair がレコードブロックに表示される。`--filter fail` を指定すると fail pair のみ、`--filter pending` は pending pair のみ、`--filter pass` は pass pair のみ、`--filter all` はすべての pair を表示する。先頭ブロック (集計) と Summary 行は `--filter` に関わらず、`--chain` / `--layer` 適用後の lane 全件集計を表示する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T003, T004, T005, T006, T007]
- [ ] [AC-10] `sotp ref-verify results` の exit code が pass/fail/pending pair の存在によらず常に 0 となる。0 以外の exit code を返すのはコマンド実行自体のエラー (引数不正・IO エラー等) のみ [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D2] [tasks: T005, T006]
- [ ] [AC-11] `SemanticVerifyEntry` に `claim_origin` / `evidence_origin` フィールドが追加されており、`bin/sotp ref-verify run` が新規に判定した pair についてこれらのフィールドを正しく populate した状態で cache に保存する。Chain① / Chain② それぞれのフィールド内容が D3 の仕様と一致する [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T001, T002]
- [ ] [AC-12] 旧 verify-cache (origin meta フィールドなし) を read-only consumer が読み込もうとすると parse error が発生し、`bin/sotp ref-verify results` / `bin/sotp ref-verify check-approved` がエラーメッセージと非 0 exit code を返す。`bin/sotp ref-verify run` は旧 entry を有効 cache として再利用せず、新 schema の cache を再生成する。旧 cache の silently forward-compat な読み込みは行わない [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D3] [tasks: T002]
- [ ] [AC-13] `/track:ref-verify` skill の Step 2 が `sotp ref-verify results` の出力を使う手順に更新されており、`spec-adr-verify-cache.json` / `<layer>-catalogue-spec-verify-cache.json` を直接 Read する旧手順が削除されている [adr: knowledge/adr/2026-06-26-0842-ref-verify-results-command.md#D1] [tasks: T008]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/coding-principles.md#No Panics in Library Code
- knowledge/conventions/prefer-type-safe-abstractions.md#Make Illegal States Unrepresentable

## Signal Summary

### Stage 1: Spec Signals
🔵 36  🟡 0  🔴 0

