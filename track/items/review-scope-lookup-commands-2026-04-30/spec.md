<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 41, yellow: 0, red: 0 }
---

# scope 分類ロジックの CLI 公開 (classify / files)

## Goal

- [GO-01] `sotp review classify --track-id <id> <path>...` を新設し、workspace 内の任意の repo-relative path を受けて scope 名 (named / Other / `<excluded>`) を text フォーマットで返すことで、`track/review-scope.json` のパターン編集時の試運転コストを下げ、agent および人間が scope 境界を CLI 一発で確認できるようにする [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1, knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D2]
- [GO-02] `sotp review files --scope <name> --track-id <id>` を新設し、現 diff base からの diff 範囲のうち当該 scope に属するファイル一覧を text フォーマットで返すことで、agent briefing の組成および個別 scope の作業範囲確認を CLI 一発で行えるようにする [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1, knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D2]
- [GO-03] `classify` / `files` の処理を `ReviewCycle` (review cycle orchestrator) とは独立した application service として usecase 層に配置することで、scope routing query が不要な dependency (`Reviewer` / `ReviewHasher`) を持たない最小 dependency 構成を実現し、hexagonal architecture の責務分離を先取りする [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6]

## Scope

### In Scope
- [IN-01] `sotp review classify --track-id <id> <path>...` CLI サブコマンドの実装: `--track-id` (必須) と 1 個以上の repo-relative path 引数を受け、`<path>TAB<scope-csv>` 形式で 1 path = 1 行を stdout に出力する read-only コマンドとして追加する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1, knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D2] [tasks: T002]
- [IN-02] `sotp review files --scope <name> --track-id <id>` CLI サブコマンドの実装: `--scope` (必須) と `--track-id` (必須) を受け、現 diff base からの diff 範囲のうち当該 scope に属するファイルを 1 行ずつ stdout に出力する read-only コマンドとして追加する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1, knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D2] [tasks: T003]
- [IN-03] `classify` の入力バリデーション: `FilePath::new` の string-level チェック (Empty / Absolute / Traversal を拒否) のみを行い、ファイルの実在確認は行わない。不正 path は human-readable なエラーメッセージを stderr に出力し exit code != 0 で返す [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D3] [tasks: T001, T002]
- [IN-04] `files` の scope 名バリデーション: `ReviewScopeConfig::contains_scope` を使い、未定義 scope 名は `Unknown scope: <name>. Known scopes: <list>` を stderr に出力し exit code != 0 で返す [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D4] [tasks: T003]
- [IN-05] `classify` の出力フォーマット: `<path>TAB<scope-csv>` の 1 行形式。複数 named scope に該当する path はカンマ区切りで 1 行に収める。operational / other_track pattern にマッチして routing map から除外された path は `<excluded>` で表示する。入力 path は必ず 1 個以上 (IN-01 / clap num_args(1..)) であり、各 path は常に 1 行出力に対応するため、入力 n 件に対して出力も常に n 行になる [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5, knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D7] [tasks: T002]
- [IN-06] `files` の出力フォーマット: `<path>` を 1 行ずつ。差分に当該 scope のファイルが 0 件の場合は empty stdout + exit 0 (エラーではない) [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5] [tasks: T003]
- [IN-07] scope routing query の usecase 層 application service 新設: `ReviewCycle` と独立した最小 dependency の application service (interactor) を usecase 層に追加し、CLI 層はその service を呼ぶ薄いラッパーとして実装する。新 service が依存する port は `ReviewScopeConfig` 提供の domain API と `DiffGetter` port に限定し、`Reviewer` / `ReviewHasher` には依存しない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6] [tasks: T001]
- [IN-08] scope universe の出力一致: `classify` が出力する scope 名を `scope_config.all_scope_names()` が返す universe 内 (named scopes + implicit `other`) に収める。`<excluded>` は universe 外の CLI sentinel として別扱いする。`files` は scope 名ではなくファイルパスを出力するため本制約は `classify` のみに適用される [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D7] [tasks: T002]
- [IN-09] `files` の diff base 解決: 既存 `review results` と同じ解決規則 (`.commit_hash` ファイル → 不在時は `git rev-parse main` fallback) を使用する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D4] [tasks: T001, T003]

### Out of Scope
- [OS-01] JSON 出力フォーマット (`--format json`): 両コマンドとも text 1 系統のみを提供し、JSON output は本 track では採用しない (YAGNI — ADR Rejected Alternatives §C)。具体的な script consumer の必要性が顕在化したタイミングで additive に追加する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5]
- [OS-02] `classify` を file path 引数なし (= 現 diff 全体を対象とする) で呼べるようにする拡張: `files --scope <name>` を各 scope に対して呼び出すことで shell loop で組み立てられるため、本 track では採用しない (ADR Rejected Alternatives §E) [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1]
- [OS-03] multi-match の rich display (1 file = N 行の展開表示): CSV 1 行形式を採用し、改行展開は行わない (ADR Rejected Alternatives §D)。tab-delimited で `cut -f1`, `cut -f2` の対称な抽出を維持する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D7]
- [OS-04] 既存 `ReviewCycle` の read-only query 切り出し refactor (get_scope_target / get_review_states 等の別 service への移設): 本 track で新設する application service は将来 refactor 後の姿を先取りするが、既存 `ReviewCycle` 自体の改修は本 track のスコープ外 [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6]
- [OS-05] domain 層 `ReviewScopeConfig` の API surface 変更: `classify` / `all_scope_names` / `contains_scope` は無変更で流用する。本 track では domain API に変更を加えない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6]
- [OS-06] 既存 `sotp review results` / `check-approved` / `codex-local` の CLI surface 変更: 本 track が追加する 2 コマンドは独立した read-only query として配置し、既存コマンドの引数・出力・振る舞いは変えない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1]
- [OS-07] `track/review-scope.json` のスキーマ変更: scope 定義ファイル自体は変えない。本 track は既存スキーマを読み取るのみ [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1]

## Constraints
- [CN-01] 両コマンドとも text 1 系統のみの出力フォーマットを提供する。JSON 出力は本 track では実装しない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5] [tasks: T002, T003]
- [CN-02] 両コマンドとも read-only / side-effect なし。review.json や任意のファイルへの書き込みを行わない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1] [tasks: T002, T003]
- [CN-03] `classify` の入力バリデーションは `FilePath::new` の string-level チェック (Empty / Absolute / Traversal) のみとし、ファイルの実在を確認しない (I/O を回さない方針)。新規作成予定の path に対しても classify を回せる [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D3] [tasks: T001, T002]
- [CN-04] `classify` の出力は入力引数の順を保つ。`files` の出力は diff getter 由来の順を保つ [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5] [tasks: T002, T003]
- [CN-05] scope routing query の処理ロジックは `ReviewCycle` (review cycle orchestrator) とは独立した application service として usecase 層に配置する。CLI 層はその service を呼ぶ薄いラッパーに留める。hexagonal architecture の layer dependency 規則に準拠する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6] [conv: knowledge/conventions/hexagonal-architecture.md#Port Placement Rules] [tasks: T001]
- [CN-06] 新規 application service の dependency は `ReviewScopeConfig` (domain API) と `DiffGetter` port に限定する。`Reviewer` / `ReviewHasher` には依存しない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6] [tasks: T001]
- [CN-07] 出力する scope 名は `scope_config.all_scope_names()` の universe 内に収める。`<excluded>` は universe 外の CLI sentinel として別扱いし、scope 名として扱わない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D7] [tasks: T002]
- [CN-08] `files` の diff base 解決は既存 `review results` と同じ規則 (`.commit_hash` ファイル → 不在時は `git rev-parse main` fallback) に従う [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D4] [tasks: T001, T003]

## Acceptance Criteria
- [ ] [AC-01] `sotp review classify --track-id <id> libs/domain/src/lib.rs apps/cli/src/main.rs` が実行可能であり、各 path に対して `<path>TAB<scope>` 形式の 1 行が stdout に出力される [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D2, knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5] [tasks: T002]
- [ ] [AC-02] `classify` に複数 named scope にマッチする path を渡すと、scope 列がカンマ区切りでアルファベット昇順 (例: `domain,usecase`) で 1 行に出力される。sort はデターミニスティックな出力のために必須 [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D7] [tasks: T002]
- [ ] [AC-03] `classify` に operational / other_track pattern にマッチする path を渡すと、scope 列が `<excluded>` で出力される [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D7] [tasks: T002]
- [ ] [AC-04] `classify` に存在しないファイルの path (ただし string-level バリデーションを通過する正常な repo-relative path) を渡しても、正常に scope 分類結果が出力される (ファイル実在確認なし) [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D3] [tasks: T002]
- [ ] [AC-05] `classify` に絶対パス / `..` を含む path / 空文字列を渡すと、human-readable なエラーメッセージが stderr に出力され exit code != 0 で終了する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D3] [tasks: T002]
- [ ] [AC-06] `classify` はすべての path がバリデーションを通過しており named scope へのマッチが 0 件であっても (すべて `other` または `<excluded>` に分類される場合) exit 0 で終了する。各 path には常に 1 行が出力されるため empty stdout にはならない [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5] [tasks: T002]
- [ ] [AC-07] `sotp review files --scope domain --track-id <id>` が実行可能であり、現 diff base からの diff 範囲のうち `domain` scope に属するファイルが 1 行ずつ stdout に出力される [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D2, knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5] [tasks: T003]
- [ ] [AC-08] `files` に未定義の scope 名を渡すと、`Unknown scope: <name>. Known scopes: <list>` が stderr に出力され exit code != 0 で終了する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D4] [tasks: T003]
- [ ] [AC-09] `files` で diff 範囲に当該 scope のファイルが 0 件の場合、empty stdout かつ exit 0 で終了する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D5] [tasks: T003]
- [ ] [AC-10] `classify` の出力の scope 名は `scope_config.all_scope_names()` の universe 内に収まっている (named scopes + `other`)。`<excluded>` がそれらとは別の sentinel として区別されている [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D7] [tasks: T002]
- [ ] [AC-11] 新 application service が usecase 層に配置されており、`ReviewCycle` と独立している。`cargo make check-layers` が pass する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6] [tasks: T001, T002, T003, T004]
- [ ] [AC-12] 新 application service の dependency graph に `Reviewer` / `ReviewHasher` が含まれない。`DiffGetter` port および `ReviewScopeConfig` (domain) のみに依存している [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D6] [tasks: T001]
- [ ] [AC-13] 既存の `sotp review results` / `check-approved` / `codex-local` の CLI surface および振る舞いが変化しない。新コマンド追加後も既存コマンドが従来通り動作する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1] [tasks: T003]
- [ ] [AC-14] `cargo make ci` の全項目 (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する [adr: knowledge/adr/2026-04-29-1547-review-scope-lookup-commands.md#D1] [tasks: T004]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/review-protocol.md#概要
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 41  🟡 0  🔴 0

