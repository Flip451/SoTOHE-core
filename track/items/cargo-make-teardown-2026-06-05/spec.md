<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 43, yellow: 0, red: 0 }
---

# cargo make ラッパー層の解体 — bin/sotp 直叩きへの一本化と git 操作・再現性ゲートの選択的維持

## Goal

- [GO-01] cargo make の約 90 タスクのうち、bin/sotp native subcommand への単純パススルーになっているラッパーを削除し、呼び出し経路を 1 層（cargo make → bin/sotp native 直叩き）に減らす。ただし git 書き込み操作（フック制約）、Docker 再現性ゲート、セットアップ、複数ステップのオーケストレーションは cargo make を入口として維持する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#Decision]
- [GO-02] bin/sotp make サブコマンド（clap 層の MakeTask enum + composition 層の make_* メソッド群）を完全廃止し、3 層パススルー（cargo make → bin/sotp make → bin/sotp native）と死蔵コード（呼び出し元のない make_* メソッド群）を同時に除去する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D2, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6]
- [GO-03] tools-daemon（compose.yml の `tools-daemon` サービスと、それに依存する Makefile.toml の 10 タスク）を削除し、日々のフィードバックループを `docker compose run --rm tools` 経由に一本化する。並列ワーカー隔離（WORKER_ID / CARGO_TARGET_DIR_RELATIVE）は run --rm でも使えるため維持する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D1]
- [GO-04] commit ゲートの複合オーケストレーション（commit / track-commit-message）において、bin/sotp make が担っていた cargo make ci サブプロセス起動という循環依存（cargo make → bin/sotp → cargo make）を解消し、依存方向を一方向にする。cargo make script に 6 ステップの繋ぎこみのみを残し、ロジックは bin/sotp native subcommand に内包する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6]
- [GO-05] DRY しきい値の 0.85 ハードコードが 3 箇所に散在している状態を解消し、.harness/config/dry-check.json を SSoT とする専用設定ファイル管理に移行する。設定が読めない場合は fail-closed でエラーにし、暗黙の既定値（0.85）でゲートを通過させない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9]

## Scope

### In Scope
- [IN-01] compose.yml の tools-daemon サービス定義と、Makefile.toml の依存 10 タスク（tools-up / tools-down / fmt-exec / clippy-exec / test-exec / test-one-exec / check-exec / machete-exec / deny-exec / llvm-cov-exec）の削除。bin/sotp make exec（tools-daemon への docker compose exec バリアント）も同時削除する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D1] [tasks: T003]
- [IN-02] bin/sotp make サブコマンドの完全削除：apps/cli/src/commands/make.rs（MakeTask enum と dispatch_*）、apps/cli-composition/src/make.rs（make_* メソッド群）、apps/cli/src/main.rs の CliCommand::Make 配線を削除する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D2] [tasks: T004]
- [IN-03] git 書き込みを伴わない非 git パススルータスク群を bin/sotp native 直叩きへ移行し、対応する Makefile.toml の wrapper と bin/sotp make バリアントを削除する。対象例：track-resolve / track-transition / track-add-task / track-next-task / track-task-counts / track-set-override / track-sync-views / track-review-results / track-check-approved / track-local-plan / track-local-review / track-local-review-fix-codex / track-local-dry-fix / track-signals / track-baseline-capture、git push を伴わない pr 系（track-pr-status / track-pr-merge / track-pr-ensure）など [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D3] [tasks: T005, T009]
- [IN-04] git 書き込み操作タスク群（add / add-all / unstage / note / track-note / track-add-paths / track-branch-create / track-branch-switch / track-switch-main / track-pr-push / track-pr / track-pr-review）について、冗長な bin/sotp make 層を削除し、cargo make task が command/args 配列形式で直接 bin/sotp git <sub>（または bin/sotp track branch <sub>）を呼ぶ形式に変更する。bin/sotp git の各サブコマンドが持つロジック（add-all の transient scratch ファイル除外、commit-from-file のトラックブランチガード等）は保持する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D4] [tasks: T006]
- [IN-05] Docker 再現性ゲート（ci / ci-rust / fmt / clippy / test / deny / machete / check / check-layers / verify-* 外部公開 wrapper / scripts-selftest 等）、セットアップ（bootstrap / build-sotp / build-tools）、および複数コマンドを束ねるオーケストレーションタスクを cargo make に維持する。ホスト cargo make → コンテナ cargo make --allow-private <task>-local の二重構造は本 ADR のスコープで現状維持 [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D5] [tasks: T010]
- [IN-06] commit / track-commit-message を cargo make script として再構成する。script は「(1) bin/sotp git add-all → (2) cargo make ci → (3) bin/sotp review check-approved → (4) bin/sotp dry check-approved → (5) bin/sotp git commit-from-file tmp/track-commit/commit-message.txt --cleanup → (6) bin/sotp track set-commit-hash」の 6 ステップを順に呼び失敗で停止する繋ぎこみのみとし、計算・条件分岐・データ処理は cargo make 側に置かない。bin/sotp track set-commit-hash は native subcommand として新設する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6] [tasks: T008]
- [IN-07] make_track_commit_message の付随ロジックを以下の通り移植する：(a) 類似度しきい値解決 → bin/sotp dry check-approved が .harness/config/dry-check.json から自己解決（D9）、(b) .commit_hash 永続化と失敗時復旧ヒント → bin/sotp track set-commit-hash（新設 native subcommand）に内包、(c) CI ログのキャプチャと失敗時末尾表示 → 廃止（cargo make ci の標準出力をそのまま表示）。resolve_commit_dry_threshold（track/items/<id>/dry-check.json からしきい値を引き継ぐロジック）は廃止する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [tasks: T002, T007, T008]
- [IN-08] 参照のない orphan タスクと ghost 参照の整理：test-nightly（参照ゼロ）/ track-baseline-capture cargo make wrapper（参照ゼロ）を削除。verify-doc-links の public compose wrapper を削除し、ci-local の内部依存だけ残す。hooks-selftest の ghost 参照（.claude/rules/07-dev-environment.md 内）を削除する（専用タスクは新設しない） [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D7] [tasks: T009]
- [IN-09] タスク削除・改名に伴う全消費者の同期：Rust 検証ロジック（libs/infrastructure/src/verify/orchestra.rs の cargo make 許可リスト、doc_patterns.rs、view_freshness.rs / convention_docs.rs のエラーメッセージ、libs/domain/src/guard/policy.rs のブロックメッセージ、libs/domain/src/skill_compliance/mod.rs のリマインダー）、権限設定（.claude/settings.json の permissions.allow の Bash(cargo make ...) エントリ）、スラッシュコマンド / スキル / エージェント定義（.claude/commands/** / .claude/skills/** / .claude/agents/**）、運用ドキュメント（DEVELOPER_AI_WORKFLOW.md / track/workflow.md / .claude/rules/07-dev-environment.md / CLAUDE.md / LOCAL_DEVELOPMENT.md 等）、テスト（scripts/test_make_wrappers.py）を一括更新する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D8] [tasks: T003, T004, T005, T008, T009, T010]
- [IN-10] .harness/config/dry-check.json（新設）に DRY しきい値（threshold）を集約し、bin/sotp dry write と bin/sotp dry check-approved が設定ファイルを読んで使う。--threshold（CLI 明示）は Option（既定なし）とし、未指定のときだけ設定ファイルを読む。散在する 0.85 ハードコード 3 箇所と resolve_commit_dry_threshold を廃止する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [tasks: T001, T007]
- [IN-11] 定型引数（--items-dir track/items 等）を bin/sotp make が補っていた箇所については、native subcommand 側の既定値・引数処理に寄せ、呼び出し側が中間層を介さずに native を直接呼べるようにする [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D3] [tasks: T005]

### Out of Scope
- [OS-01] git 系を含む全タスクを native 直叩きに統一し、block-direct-git-ops フック側を緩める方式（全タスク native 統一案）。フックの fail-closed な単純さを維持するため採用しない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#Rejected Alternatives]
- [OS-02] cargo make を全廃し Docker オーケストレーションも Rust 側に移す方式。Docker 再現性ゲートは compose による隔離が本質であり、Rust 側に Docker オーケストレーションを持つのは責務違反のため採用しない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#Rejected Alternatives]
- [OS-03] ホスト cargo make → コンテナ cargo make --allow-private <task>-local の二重構造の簡素化。本 ADR のスコープ外（現状維持） [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D5]
- [OS-04] bin/sotp make サブコマンドを git オーケストレーション専用に縮小して残す方式。git ラッパーは cargo make task の command/args 配列から直接 bin/sotp git を呼べば足り、make 名前空間を残す必要がないため採用しない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#Rejected Alternatives]
- [OS-05] bin/sotp 側で CI を直接実行し cargo make ci を呼ばないサイクル解消方式。CI の構成（Docker 隔離・タスク列）を Rust 側にも複製することになり二重管理になるため採用しない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#Rejected Alternatives]
- [OS-06] DRY しきい値の SSoT を agent-profiles.json のフィールドや Rust コードのハードコード定数に置く方式。専用設定ファイル（.harness/config/dry-check.json）を選択したため対象外 [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9]
- [OS-07] track/items/<id>/dry-check.json の threshold フィールドを次回のしきい値決定に引き継ぐ resolve_commit_dry_threshold の維持。.harness/config/dry-check.json を SSoT とするため廃止する（track の per-track ファイルにある threshold は履歴記録として残るが、次回決定には使わない） [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [tasks: T007]

## Constraints
- [CN-01] ロジックは bin/sotp（Rust native subcommand）に置き、cargo make へ流出させない。cargo make に残すのは「複数 native コマンドの順次実行（繋ぎこみ）」と「docker / setup の入口」だけとし、計算・条件分岐・データ処理・出力整形を cargo make の @shell script に書いてはならない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#Decision] [tasks: T004, T005, T006, T008]
- [CN-02] git 書き込み操作（git add / git commit / git push / git switch / git branch -d / git notes add / git restore --staged 等）の入口は cargo make 経由を維持する。block-direct-git-ops フック（command_contains_git）が Claude Code の Bash tool call で git 文字列を含む引数をブロックするため、bin/sotp git <sub> の Bash 直叩きは正規ルートとして機能しない。cargo make task（タスク名に git を含まない）の command/args 配列から bin/sotp git <sub> を呼ぶ経路が唯一の正規ルートである [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D4] [tasks: T006]
- [CN-03] git 単純ラッパータスク（D4 対象：add / add-all / unstage / note / track-note / track-add-paths / track-branch-create / track-branch-switch / track-switch-main / track-pr-push / track-pr / track-pr-review）から bin/sotp git <sub> を呼ぶ際は command/args 配列形式（@shell スクリプトではない）を使い、引数の shell 文字列展開を排除する。この制約は D6 対象の commit / track-commit-message には適用されない（D4 は commit / track-commit-message を複数ステップのオーケストレーションとして明示的に D6 へ委ねている）。D6 の commit 6 ステップ script では可変入力（コミットメッセージ・ノート本文）をファイル経由（tmp/track-commit/*.txt）に限定することで引数安全性を確保しており、@shell script としての実装を許容する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D4, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6] [tasks: T006]
- [CN-04] .harness/config/dry-check.json が読めない（不在 / パースエラー / I/O エラー）場合、bin/sotp dry write / bin/sotp dry check-approved は暗黙の既定値（0.85）にフォールバックせずエラーで停止する（fail-closed）。設定欠落を黙って通過させてはならない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [tasks: T001, T007]
- [CN-05] タスク削除と検証ロジックの更新は同一変更内で行う。orchestra.rs の cargo make 許可リスト等と実体が乖離すると CI（verify-orchestra / verify-arch-docs 等）が落ちるため、削除とドキュメント・検証ロジック同期を分割することを禁止する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D8] [tasks: T003, T004, T005, T008, T009]
- [CN-06] bin/sotp git の各サブコマンドが持つ保護ロジック（add-all の transient scratch ファイル除外、commit-from-file のトラックブランチガード、add-from-file のパス検証と重複排除、switch-and-pull の checkout + pull 連結）は削除しない。cargo make から bin/sotp git を呼ぶ経路に変更しても、これらの保護ロジックは bin/sotp git 側に残る [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D4] [tasks: T006]
- [CN-07] .harness/config/dry-check.json の読み込みロジックは infrastructure 層に置く（AgentProfiles::load と同様のパターン）。cargo make 側に読み込みロジックを持ち込まない。--threshold は Option（既定なし）とし、未指定のときだけ設定ファイルを読む [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [conv: knowledge/conventions/hexagonal-architecture.md#Adapter Rules] [tasks: T001]

## Acceptance Criteria
- [ ] [AC-01] compose.yml から tools-daemon サービス定義が削除されており、Makefile.toml から tools-up / tools-down / fmt-exec / clippy-exec / test-exec / test-one-exec / check-exec / machete-exec / deny-exec / llvm-cov-exec の 10 タスクが存在しない。bin/sotp make exec（または相当するコマンド）が apps/cli/src/commands/make.rs から削除されている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D1] [tasks: T003]
- [ ] [AC-02] apps/cli/src/commands/make.rs（MakeTask enum と dispatch_* 関数群）、apps/cli-composition/src/make.rs（make_* メソッド群）、apps/cli/src/main.rs の CliCommand::Make 配線がいずれも存在しない。`bin/sotp make <任意>` が実行不可能（コマンド not found または subcommand not recognized）になっている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D2] [tasks: T004]
- [ ] [AC-03] D3 で列挙した非 git パススルータスク群（track-resolve / track-transition / track-add-task 等）が Makefile.toml から削除されており、スラッシュコマンド / スキル / エージェント定義 / 運用ドキュメントが bin/sotp native 直叩き形式（bin/sotp track <sub> 等）に更新されている。native subcommand の既定値・引数処理が定型引数（--items-dir track/items 等）を補うため、移行後も機能が維持されている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D3] [tasks: T005]
- [ ] [AC-04] D4 で列挙した git 書き込みタスク群（add / add-all / unstage / track-branch-create 等）が、@shell スクリプトではなく command/args 配列形式で bin/sotp git <sub>（または bin/sotp track branch <sub>）を呼んでいる。bin/sotp git add-all の transient scratch ファイル除外ロジック、commit-from-file のトラックブランチガードなど保護ロジックが維持されている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D4] [tasks: T006]
- [ ] [AC-05] Docker 再現性ゲートタスク（ci / ci-rust / fmt / clippy / test / deny 等）と bootstrap / build-sotp / build-tools が cargo make に残っており、`docker compose run --rm tools` 経由の隔離実行が維持されている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D5] [tasks: T010]
- [ ] [AC-06] cargo make track-commit-message（または commit）が「(1) bin/sotp git add-all → (2) cargo make ci → (3) bin/sotp review check-approved → (4) bin/sotp dry check-approved → (5) bin/sotp git commit-from-file ... → (6) bin/sotp track set-commit-hash」の 6 ステップを順に呼び失敗で停止する script として実装されており、@shell 内に計算・条件分岐・データ処理が含まれていない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6] [tasks: T008]
- [ ] [AC-07] bin/sotp track set-commit-hash native subcommand が新設されており、.commit_hash の永続化と失敗時の復旧ヒント出力が本サブコマンドに内包されている。現行 composition 層の persist_commit_hash_for_track に相当するロジックが track サブコマンド配下へ移されている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6] [tasks: T002]
- [ ] [AC-08] resolve_commit_dry_threshold（track/items/<id>/dry-check.json の最新レコードからしきい値を引き継ぐロジック）が廃止されており、しきい値解決は bin/sotp dry check-approved と bin/sotp dry write が .harness/config/dry-check.json から自己解決する。cargo make script が --threshold を渡さない [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D6, knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [tasks: T007, T008]
- [ ] [AC-09] test-nightly タスク、track-baseline-capture の cargo make wrapper が Makefile.toml から削除されている。verify-doc-links の public compose wrapper が Makefile.toml から削除されており（ci-local の内部 dependency は残存）、.claude/rules/07-dev-environment.md の hooks-selftest に関する cargo make タスク参照が削除されている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D7] [tasks: T009]
- [ ] [AC-10] libs/infrastructure/src/verify/orchestra.rs の cargo make 許可リスト（静的配列）が削除済みタスクを参照していない。doc_patterns.rs / view_freshness.rs / convention_docs.rs のエラーメッセージ / policy.rs のブロックメッセージ / skill_compliance/mod.rs のリマインダーが移行後の入口文字列と整合している。.claude/settings.json の permissions.allow の Bash(cargo make ...) エントリが移行先（bin/sotp 直叩き）に更新されている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D8] [tasks: T003, T004, T005, T008, T010]
- [ ] [AC-11] .harness/config/dry-check.json が新設されており、threshold フィールドを持つ JSON（schema_version + threshold 構造）として存在する。bin/sotp dry write と bin/sotp dry check-approved がこのファイルを読み、--threshold 未指定のときに設定ファイルの値を使う。0.85 ハードコードが apps/cli/src/commands/dry.rs および apps/cli-composition/src/make.rs から消えている [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [tasks: T001, T007]
- [ ] [AC-12] --threshold 未指定かつ .harness/config/dry-check.json が不在またはパースエラーの場合、bin/sotp dry write と bin/sotp dry check-approved が非ゼロで終了する（0.85 にフォールバックして通過しない） [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D9] [tasks: T001, T007]
- [ ] [AC-13] scripts/test_make_wrappers.py の削除済みタスクに対応するテストケースが削除または skip 更新されており、cargo make ci（全体 CI: fmt-check + clippy + nextest + deny + check-layers + verify-*）が pass する [adr: knowledge/adr/2026-06-05-1535-cargo-make-teardown.md#D8] [tasks: T003, T004, T005, T009, T010]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/hexagonal-architecture.md#Layer Dependencies
- knowledge/conventions/hexagonal-architecture.md#Adapter Rules
- knowledge/conventions/shell-parsing.md#Single Parser Rule
- knowledge/conventions/pre-track-adr-authoring.md#Rules
- knowledge/conventions/workflow-ceremony-minimization.md#Rules
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Trait-Based Abstraction (Hexagonal Architecture)
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator

## Signal Summary

### Stage 1: Spec Signals
🔵 43  🟡 0  🔴 0

