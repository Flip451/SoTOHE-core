<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 38, yellow: 0, red: 0 }
---

# `sotp review results` で review.json 直読みを置き換える

## Goal

- [GO-01] 読み取り専用の正規 API として `sotp review results` を新設し、scope 別の `ReviewState` 表示 (旧 `review status` の機能を包含)、過去 N round の verdict / findings の CLI 取得、および commit hint 出力を 1 コマンドに統合することで、review.json の直読み慣行を規約的に排除する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1]
- [GO-02] 既存 `sotp review status` を削除 (後方互換 alias なし) し、`sotp review results` (flag なし既定実行) を旧 `status` の完全な等価コマンドとして位置付けることで、read API の一本化を達成する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1]
- [GO-03] commit hint の判定ロジックおよび approval/bypass 判定を CLI 層から domain/usecase 層に lift することで、`sotp review check-approved` と `sotp review results` の hint が同じ usecase 戻り値を消費し、drift を構造的に排除する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5]

## Scope

### In Scope
- [IN-01] `sotp review results` CLI サブコマンドの実装: `--track-id`, `--items-dir`, `--scope`/`--all`, `--limit N|all`, `--round-type fast|final|any`, `--no-hint` の各 flag を持つコマンドとして追加する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D2] [tasks: T005]
- [IN-02] text 単一フォーマットによる出力実装: 既定実行 (`--limit 0`) では state summary (header 行 + diff base 行 + scope 別 indicator + state line + summary 行) のみを出力する。state line はスコープの `ReviewState` から導出し、review.json にラウンド履歴がある scope については最新ラウンドの type / verdict / timestamp を state line に付加する (例: `[+] domain  final@... zero_findings`)。`--limit N > 0` では state summary に加え最新 round の findings 詳細と過去 round 履歴をインデント付きで展開する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D3] [tasks: T005]
- [IN-03] `scope_config.all_scope_names()` が返す scope universe を完全列挙: named scopes + implicit `Other` scope をすべて state line として出力し、省略しない。diff にマッチしない scope は `[.] <scope>: not required (empty)` として表示する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D4] [tasks: T005]
- [IN-04] commit hint の実装: 全 scope が `NotRequired(*)` 状態かつ `review.json` が存在するとき (`--no-hint` 未指定の場合のみ) 、Summary 行の後に hint メッセージを append する。`ApprovedWithBypass` (all not started + review.json 不在) では hint を出さない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T005]
- [IN-05] approval/bypass 判定ロジックの domain/usecase への lift: `ReviewApprovalVerdict` 相当の enum を domain 層に導入し、`ReviewCycle` に `evaluate_approval` 相当のメソッドを usecase 層に追加する。CLI `run_check_approved` は usecase 戻り値を exit code + eprintln にマップするだけに痩せる [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T001, T003, T004]
- [IN-06] `sotp review status` CLI サブコマンドの削除 (後方互換 alias なし): `ReviewCommand::Status` variant + `StatusArgs` + `execute_status` + `run_status` を CLI から完全に除去し、使用箇所 (`.claude/`, `knowledge/`, `track/workflow.md`, `Makefile.toml` 等) をすべて `sotp review results` に更新する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1] [tasks: T006, T007]
- [IN-07] `review.json` の存在確認を infrastructure 側 port に委譲: approval 判定に必要な「ローカルレビューが記録されているか」の I/O 判定を `ReviewStore` または別 port method として infrastructure 層に配置し、usecase 層が直接 `std::fs` に触れないようにする [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T001, T002]

### Out of Scope
- [OS-01] JSON 出力フォーマット (`--format json`): text 単一フォーマットのみを提供し、JSON output は本 track では採用しない (YAGNI — Rejected Alternatives §A)。具体的な script consumer の必要性が顕在化したタイミングで additive に追加する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D3]
- [OS-02] `sotp review check-approved` の CLI surface (exit code / 引数) 変更: `check-approved` は stateless gate として責務が独立しており、CLI interface は無変更。内部実装 (usecase への lift) は IN-05 に含むが、外部から観測可能な振る舞いは変えない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D6]
- [OS-03] `sotp review codex-local` の stdout JSON shape 変更、commit hint 追加、verdict サマリ追加: `codex-local` は 1 scope の review を回す責務のツールであり、track 全体の状態判定ロジックは混入しない (Rejected Alternatives §C) [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D6]
- [OS-04] `review.json` スキーマ変更 (schema_version 2 → 3): 本 track では review.json のスキーマ自体を変えない。コマンドは既存 schema_version 2 の round 配列を読み取るだけ [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1]
- [OS-05] `review results --exit-code-on-blocked` のような `check-approved` 統合: 将来の拡張候補として ADR Reassess When に記録されているが、本 track のスコープ外 (Rejected Alternatives §D) [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D6]
- [OS-06] 全 scope が `NotRequired::Empty` かつ `review.json` が存在する edge case への明示的救済: hint が出るが diff が空なので `/track:commit` 側で弾かれる。ユーザーへの実害はなく、救済ロジックは入れない (ADR D5.2 既知の minor edge case) [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5]

## Constraints
- [CN-01] `sotp review results` は text 単一フォーマットのみを提供する。JSON 出力は本 track では実装しない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D3] [tasks: T005]
- [CN-02] 既定モード (`--all`) における出力の scope 一覧は `scope_config.all_scope_names()` が返す universe を完全列挙する。named scopes + `Other` を含み、差分なし / レビュー未実行の scope も省略しない。`--scope <name>` フィルタ指定時は対象 scope のみを表示する (AC-14 のフィルタ動作は CN-02 の完全列挙から除外される) [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D4] [tasks: T005]
- [CN-03] commit hint は `check-approved` の OK 条件 (全 scope `NotRequired(*)`) + `review.json` 存在 の AND で発火する。`--no-hint` flag で抑制可能。`ApprovedWithBypass` ケース (all not started + review.json 不在) では hint を出さない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T005]
- [CN-04] approval/bypass 判定ロジックは usecase 層に配置し、CLI 層が直接ドメイン判定を実装しない。`review.json` 存在確認の I/O は infrastructure 側の port に委譲する。hexagonal architecture の layer dependency 規則に準拠する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [conv: knowledge/conventions/hexagonal-architecture.md#Port Placement Rules] [tasks: T001, T002, T003, T004]
- [CN-05] `sotp review status` の削除は破壊的変更であり、後方互換 alias を置かない。使用箇所はすべて `sotp review results` に移行する。no-backward-compat convention に従う [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1] [conv: knowledge/conventions/no-backward-compat.md#Rules] [tasks: T006, T007]
- [CN-06] `--limit` flag は履歴詳細の深さのみを制御し、state summary の表示有無には影響しない。`--limit 0` は「履歴 0 件」= state summary のみの意味であり、旧 `review status` の等価実行となる [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D2] [tasks: T005]
- [CN-07] `sotp review check-approved` の CLI surface (exit code / 引数) は無変更とする。内部実装のみ usecase への lift を行い、外部から観測可能な振る舞い (exit 0 / exit 1 / eprintln メッセージ形式) を変えない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D6] [tasks: T004]

## Acceptance Criteria
- [ ] [AC-01] `sotp review results --track-id <id>` が実行可能であり、全 scope の state summary (indicator + scope + state line) と Summary 行を text フォーマットで stdout に出力する。出力は旧 `sotp review status` の出力と等価な情報を含む [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1, knowledge/adr/2026-04-28-1905-review-results-command.md#D2] [tasks: T005]
- [ ] [AC-02] `sotp review results --track-id <id> --limit 2` を実行すると、各 scope の最新 round の findings 詳細 (message / severity / file:line / category) が state line 直下に展開され、`history (newer first, up to --limit):` 見出しの下に過去 round の履歴が表示される [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D3] [tasks: T005]
- [ ] [AC-03] `sotp review results --track-id <id> --limit all` を実行すると、全 round の履歴が展開される。`--limit 0` (既定) は state summary のみを出力し、履歴行は出力しない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D2] [tasks: T005]
- [ ] [AC-04] 出力の scope 一覧は `scope_config.all_scope_names()` が返す scope universe を完全列挙する。差分にマッチしない scope は `[.] <scope>: not required (empty)` として出力される。`Other` scope が常に含まれる [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D4] [tasks: T005]
- [ ] [AC-05] 全 scope が `NotRequired(*)` かつ `review.json` が存在するとき、Summary 行の後に commit hint が出力される。`--no-hint` を指定するとこの行は出力されない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T005]
- [ ] [AC-06] `review.json` が存在しないとき、commit hint は出力されない。hint の発火条件は AC-05 の 2 条件 (全 NotRequired(*) かつ review.json 存在) の AND であり、review.json 不在はその条件を満たさない。典型例: PR ベースの bypass パスとして全 Required scope が `Required(NotStarted)` かつ review.json が存在しない場合。全 scope が `NotRequired(Empty)` の場合は review.json の有無を問わず hint の第 1 条件 (全 NotRequired(*)) を満たすが、review.json が存在しなければ第 2 条件を満たさず hint は出ない (review.json が存在する empty edge case は OS-06 参照) [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T005]
- [ ] [AC-07] `sotp review status` サブコマンドが CLI から削除されており、`sotp review status` を実行するとコマンド未知エラーとなる。後方互換 alias は存在しない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1] [tasks: T006]
- [ ] [AC-08] `sotp review status` への参照が `.claude/` / `knowledge/` / `track/workflow.md` / `Makefile.toml` のすべての箇所で `sotp review results` に更新されており、dead reference が残らない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1] [tasks: T007]
- [ ] [AC-09] approval/bypass 判定ロジックが usecase 層に実装されており (`ReviewApprovalVerdict` 相当の enum が domain 層に定義され、`ReviewCycle` または別 usecase 関数として `evaluate_approval(reader, review_json_exists) -> ReviewApprovalVerdict` 相当が usecase 層に存在する)、CLI `run_check_approved` が usecase 戻り値を exit code + eprintln にマップするだけの実装になっている [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T001, T003, T004]
- [ ] [AC-10] `sotp review check-approved` の外部から観測可能な振る舞い (exit 0 on approved / exit 1 on blocked, eprintln の [OK] / [BLOCKED] / [WARN] メッセージ形式) が変化しない。CLI surface は無変更 [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D6] [tasks: T004]
- [ ] [AC-11] `sotp review results` と `sotp review check-approved` の hint / approval 判定が同じ usecase 戻り値 (`ReviewApprovalVerdict`) を消費することで、両コマンドの振る舞いが常に同一の判定根拠から導出される。具体的に排除される drift は「hint が出るが check-approved は blocked」のパターン。hint は `review results` 独自の追加条件 (Approved かつ review.json 存在の AND) を満たす場合のみ発火するため、`Approved + review.json 不在` で hint が出ないのは drift ではなく仕様どおり。`ApprovedWithBypass` (PR ベース bypass パス) では hint を出さず check-approved は SUCCESS を返すが、これも両コマンドが同一の verdict を見た結果であり drift ではない [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T003, T004, T005]
- [ ] [AC-12] `review.json` 存在確認の I/O が usecase 層に直接書かれておらず、infrastructure 側の port method (`ReviewStore` または専用の port trait) を経由して判定されている。`cargo make check-layers` が pass する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D5] [tasks: T001, T002, T004]
- [ ] [AC-13] `--round-type fast|final|any` flag が機能しており、`--limit > 0` 時に指定された round type のみの round が履歴に表示される。既定 `any` はすべての round type を対象とする [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D2] [tasks: T005]
- [ ] [AC-14] `--scope <name>` flag が機能しており、指定した単一 scope の結果のみを表示する。`--all` (既定) は全 scope を表示する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D2] [tasks: T005]
- [ ] [AC-15] `cargo make ci` の全項目 (fmt-check + clippy + nextest + deny + check-layers + verify-*) が pass する [adr: knowledge/adr/2026-04-28-1905-review-results-command.md#D1] [tasks: T008]

## Related Conventions (Required Reading)
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/no-backward-compat.md#Rules
- knowledge/conventions/review-protocol.md#概要
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/05-testing.md#Test Structure

## Signal Summary

### Stage 1: Spec Signals
🔵 38  🟡 0  🔴 0

