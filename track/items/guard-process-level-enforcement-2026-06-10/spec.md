<!-- Generated from spec.json — DO NOT EDIT DIRECTLY -->
---
version: "1.0"
signals: { blue: 43, yellow: 0, red: 0 }
---

# git 書き込みガードの enforcement を git hooks 層へ移行する

## Goal

- [GO-01] git 書き込みガードの enforcement 点を Claude Code フックのコマンド文字列スキャンからプロセスレベルの git hooks（reference-transaction / pre-push）へ移行し、SEC-11 誤発火（非 git コマンドへの過剰ブロック）を解消する。また、外部サブプロセス経由のギャップについては、reference-transaction で捕捉される ref 更新操作（commit / merge / rebase / cherry-pick / reset / fetch 等）と push を構造的に閉じる。ただし git add 等の非 ref 更新ワークツリー操作は reference-transaction フックで捕捉されず、許容残余として残る [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D1]
- [GO-02] git hook の通行証を環境変数 SOTP_GUARDED_GIT の単一変数モデルに統一し、sotp バイナリの git_cli 単一通過点でのみトークンを注入することで fail-closed な許可判定を実現する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2]
- [GO-03] Claude Code フックのコマンド文字列スキャン時代のブランケットブロック（0080 由来 + CON-07 の一括リダイレクト・tee・sed-i ブロック）を撤廃し、精密チェックと D3 トークンキーワードスキャンのみを維持することでフリクションを大幅に削減する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4]
- [GO-04] フックの block 理由文字列を stdout から stderr へ移行することで、AI に remediation 指示が確実に届くようにする（D6 stderr fix） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D6]
- [GO-05] core.hooksPath の配備を bootstrap / CI verify / runtime fail-closed の三重で担保し、git hooks 層の不在による保護空白を排除する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7]

## Scope

### In Scope
- [IN-01] .githooks/ ディレクトリに reference-transaction フック（--no-verify 迂回不可）と pre-push フックを配置する。各フックは bin/sotp hook dispatch の新しいフック名（git-ref-update / git-pre-push）へ exec する 2 行程度のシェルシムとして実装する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D1] [tasks: T002, T007]
- [IN-02] libs/infrastructure/src/git_cli/ の Command::new("git") 単一通過点で子プロセス環境に SOTP_GUARDED_GIT を設定し、sotp 経由のすべての git 呼び出し（cargo make track-commit-message / track-add-paths / track-branch-create / track-pr-push 等）が自動的に通行証を持つようにする [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2] [tasks: T003]
- [IN-03] block-direct-git-ops フックに「Bash コマンド文字列に SOTP_GUARDED_GIT（exact match）が含まれていたら block」するキーワードスキャンを追加する。スキャンは 2 段階で実施する: (a) GuardHookHandler（usecase 層）が check_commands 呼び出し前に raw コマンド文字列をスキャンし、(b) policy.rs の check_commands（domain 層）が quote-strip 済み argv トークン（SimpleCommand.argv）をスキャンする。両段階を合わせてクォート分割による回避を防ぐ（SimpleCommand は raw 文字列を保持しないため段階 a を usecase 層で実施）。Bash コマンド文字列のみが対象であり Write/Edit の content はスキャンしない [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D3] [tasks: T001]
- [IN-04] policy.rs から撤廃するブランケットブロック: command_contains_git（非 git コマンドへの "git" 部分文字列スキャン）、env 無条件ブロック、command_contains_expansion（$ / バッククォート展開ブロック）、has_output_redirect 一括ブロック（>/>>/> |/<>）、FILE_WRITE_COMMANDS（tee）、sed -i チェック。これらは 0080 D1/D2/D4 および CON-07 に由来し、本 ADR の D4 が supersede する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T001]
- [IN-05] policy.rs で維持する精密チェック: 直接 git サブコマンドチェック（git add / commit / push / switch / merge / rebase / cherry-pick / reset / branch -d / checkout -b）、launcher stripping（sudo / nohup / timeout 等）・VAR=val skip・.exe suffix strip（解析基盤）、bin/sotp 上書き検出（is_bin_sotp_overwrite）。また block-test-file-deletion にリダイレクト先ターゲットのテストファイルパターン照合を追加し、リダイレクト解禁後の「> tests/foo.rs」形式の truncation を block する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T001]
- [IN-06] permissions.deny および FORBIDDEN_ALLOW は現状維持とする（D5: entry-point 列挙を大量追加しない） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D5] [tasks: T009]
- [IN-07] apps/cli-composition/src/hook.rs の block verdict 生成部分を CommandOutcome { stdout: Some(reason), stderr: None, exit_code: 2 } から CommandOutcome { stdout: None, stderr: Some(reason), exit_code: 2 } へ変更し、apps/cli/src/commands/hook.rs 経由で reason が stderr に出力されるようにする [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D6] [tasks: T005]
- [IN-08] cargo make bootstrap および /track:setup が git config core.hooksPath .githooks を設定するステップを含む（D7 の setup 層） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7] [tasks: T006]
- [IN-09] bin/sotp verify サブコマンド（または新規サブコマンド）が core.hooksPath の設定値を検査し、未設定または .githooks 以外に設定されている場合に cargo make ci を fail させる（D7 の CI verify 層） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7] [tasks: T004, T006]
- [IN-10] block-direct-git-ops フックが実行時に core.hooksPath の未設定を検知した場合に git コマンドを block する runtime fail-closed 動作（D7 の第3層） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7] [tasks: T006]
- [IN-11] knowledge/conventions/bash-write-guard.md（CON-07）を D4 に合わせて改訂する。Layer-2 ブロック（出力リダイレクト一括・tee・sed-i）の撤廃と、その動機（file-lock hooks の撤去）を反映した文書更新を行う [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [conv: knowledge/conventions/bash-write-guard.md#Layer 2] [tasks: T008]

### Out of Scope
- [OS-01] permissions.deny の大量拡張（bash -c / python3 -c 系 wrapper 約 20 項目 + 危険 git サブコマンド約 12 項目）。entry-point 列挙はフラグ変形に対して構造的に不完全であり、git hooks が同じ目的をより確実に達成するため採用しない（D5） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D5] [tasks: T009]
- [OS-02] Write/Edit の content へのキーワードスキャン拡大。ガード実装自体（git_cli/、hook 実装、テスト）が SOTP_GUARDED_GIT を正当に含むため、保守のたびに誤発火する。スクリプトファイル間接実行は許容残余として扱う（D3 / Rejected F） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D3] [tasks: T001]
- [OS-03] CLAUDECODE 環境変数による人間 / AI 判別モデルの導入。判定変数が 2 つに増え攻撃面が生まれ、fail mode が fail-open になるため採用しない（Rejected B） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2] [tasks: T002]
- [OS-04] nonce ファイル方式によるトークン強化。D3 のキーワードスキャンでインライン偽装は既に不可能であり、残るスクリプト間接実行は nonce でも防げない。複雑さに見合う追加防御がないため採用しない（Rejected D） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2] [tasks: T002]
- [OS-05] git-cli.sh 等の新規ラッパースクリプト導入。既存の cargo make / bin/sotp ラッパー群が同じ役割を提供しており、抽象階層の重複になるため採用しない（Rejected E） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2] [tasks: T003]
- [OS-06] 0080 由来の env 無条件ブロックおよび $ / バッククォート展開ブロックの維持。両者の根拠はコマンド文字列スキャン方式の盲点保護であり、プロセスレベル enforcement 移行後に根拠が消滅している（Rejected G） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T001]

## Constraints
- [CN-01] git hooks 本体は 2 行程度のシェルシムとし、判定ロジックは既存の Rust hook アーキテクチャ（domain 層 policy + usecase 層 dispatch）に乗せる。policy.rs の remediation メッセージ定数は再利用する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D1] [tasks: T002]
- [CN-02] git hook の通行証判定は環境変数 SOTP_GUARDED_GIT の有無のみとする。SOTP_GUARDED_GIT が提示されていれば許可、それ以外はブロック（remediation メッセージ付き）。人間 / AI の判別変数は持たない [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2] [tasks: T002]
- [CN-03] SOTP_GUARDED_GIT の自動注入点は libs/infrastructure/src/git_cli/ の Command::new("git") 単一通過点のみとする（対象: sotp 経由の git 書き込み操作 = track-commit-message / track-add-paths / track-branch-create / track-pr-push 等）。これにより sotp 経由の書き込み git 呼び出しすべてが自動的に通行証を持つ。例外（read-only git config 読み取り）: T004 の verify hooks-path 関数および T006 の runtime fail-closed 判定（CLI 合成ルートが git config --local core.hooksPath を読む呼び出し）は SOTP_GUARDED_GIT を注入しない read-only 呼び出しであり、この単一通過点制約の対象外として明示的に許容する（読み取り専用であり git reference-transaction フックを発火させないため）。AI（Claude Code Bash tool）によるインライン注入は D3 キーワードスキャンが阻止する。なお人間がターミナルから SOTP_GUARDED_GIT=1 git ... と自己提示することは D2 で明示的に許容された escape hatch であり、このトークン注入規則の制約対象ではない [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2] [conv: knowledge/conventions/enforce-by-mechanism.md#Rules] [tasks: T003]
- [CN-04] D3 キーワードスキャンは Bash コマンド文字列のみを対象とし、Write/Edit の content はスキャンしない。スクリプトファイル間接実行は許容残余として扱い、Write/Edit content スキャンによる保守時誤発火を回避する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D3] [tasks: T001]
- [CN-05] shell コマンド解析は既存の ShellParser port（domain::guard::ShellParser）と ConchShellParser adapter（infrastructure::shell::ConchShellParser）を引き続き使用する。D3 の quote-strip 済み argv トークンスキャンも同パーサー経由で実施し、独自トークナイザーの実装を禁止する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D3] [conv: knowledge/conventions/shell-parsing.md#Single Parser Rule] [tasks: T001]
- [CN-06] block-direct-git-ops フックは core.hooksPath が未設定の場合に runtime fail-closed（git コマンドを block）で動作する。git hooks 層が不在の状態でも CC フックが最低限の防御を維持する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7] [tasks: T006]
- [CN-07] block-test-file-deletion フックにリダイレクト先のテストファイルパターン照合を追加する。リダイレクト解禁（D4）により可能になる「> tests/foo.rs」形式の truncation を SimpleCommand.redirect_texts に対するパターンマッチで block する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T001]
- [CN-08] hook dispatch の Rust usecase 層は既存のアーキテクチャ（HookDispatchInteractor / HookVerdictOutput）に git-ref-update / git-pre-push の新ディスパッチエントリを追加する形で拡張する。既存のフック dispatch パスを壊さない [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D1] [conv: knowledge/conventions/hexagonal-architecture.md#Port Placement Rules] [tasks: T002]

## Acceptance Criteria
- [ ] [AC-01] .githooks/reference-transaction と .githooks/pre-push が存在し、それぞれ bin/sotp hook dispatch git-ref-update / git-pre-push へ exec するシェルシムとして実装されている。reference-transaction フックは --no-verify で迂回できないことが git のドキュメントで確認できる [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D1] [tasks: T007]
- [ ] [AC-02] libs/infrastructure/src/git_cli/ の git 呼び出し通過点が子プロセス環境に SOTP_GUARDED_GIT を設定している。sotp 経由の git コマンド（cargo make track-commit-message 等）が reference-transaction フックを通過し、sotp 非経由の直接 git commit / push が block される [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D2] [tasks: T003, T009]
- [ ] [AC-03] SOTP_GUARDED_GIT キーワードスキャンが 2 段階で機能する: (a) usecase 層 GuardHookHandler が check_commands 呼び出し前に raw HookInput.command 文字列に SOTP_GUARDED_GIT が含まれる場合に block する（例: SOTP_GUARDED_GIT=1 git commit という raw 文字列を block する）。(b) domain 層 policy.rs の check_commands が quote-strip 済み argv トークン（SimpleCommand.argv）に SOTP_GUARDED_GIT が含まれる場合に block する（クォート分割回避を防ぐ）。SimpleCommand は raw コマンド文字列を保持しないため段階 (a) は usecase 層で実施する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D3] [tasks: T001]
- [ ] [AC-04] 従来 block されていた正当な Bash 操作が解禁されている: echo hello > /tmp/file.txt（出力リダイレクト）、ls | tee output.txt（tee）、sed 's/a/b/' file.txt（-i なし sed）、env cargo test（env）、$(pwd)（コマンド置換）。各操作が allow verdict を返すユニットテストが通過する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T001]
- [ ] [AC-05] D4 で維持される精密チェックが引き続き機能する: git add / commit / push / switch / merge / rebase / cherry-pick / reset の直接呼び出しが block され、bin/sotp 上書き（cp target/release/sotp bin/sotp 等）が block される。既存テストが通過する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T001]
- [ ] [AC-06] block-test-file-deletion フックが「> tests/foo.rs」形式のリダイレクトターゲットに tests/ パスが含まれる Bash コマンドを block する。既存の rm -based テストファイル削除 block は引き続き機能する [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T001]
- [ ] [AC-07] permissions.deny および FORBIDDEN_ALLOW に新規エントリが追加されていない（D5 の現状維持確認） [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D5] [tasks: T009]
- [ ] [AC-08] block verdict の reason 文字列が stderr に出力される。block 経路の CommandOutcome は stdout: None かつ stderr: Some(reason) かつ exit_code: 2 となり、apps/cli/src/commands/hook.rs の execute 関数が CommandOutcome.stderr を eprintln! で出力する。allow 経路その他の非 block パスの stdout 処理は変更しない。block 発生時に AI が remediation 指示を受け取れる [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D6] [tasks: T005]
- [ ] [AC-09] cargo make bootstrap の実行ログに「git config core.hooksPath .githooks」相当のステップが出力され、実行後に core.hooksPath が .githooks に設定されている [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7] [tasks: T006]
- [ ] [AC-10] cargo make ci（または bin/sotp verify 系サブコマンド）が core.hooksPath の設定値を検査し、未設定または .githooks 以外の場合に non-zero exit で CI を fail させる [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7] [tasks: T004, T006]
- [ ] [AC-11] block-direct-git-ops フックが core.hooksPath 未設定を検知した場合に git コマンドを block する（runtime fail-closed）。core.hooksPath が設定済みの場合はこの追加 block は発火しない [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D7] [tasks: T006]
- [ ] [AC-12] knowledge/conventions/bash-write-guard.md が D4 の内容を反映した形に改訂されており、Layer-2 ブロック（出力リダイレクト一括・tee・sed-i）の撤廃と file-lock hooks 撤去という動機の消滅が記述されている [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [conv: knowledge/conventions/bash-write-guard.md#Layer 2] [tasks: T008]
- [ ] [AC-13] cargo make ci が通過する（fmt-check / clippy / test / deny / check-layers / verify-arch-docs / verify-orchestra を含む）。D4 の削除・追加後のコードが CI を break しない [adr: knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md#D4] [tasks: T009]

## Related Conventions (Required Reading)
- knowledge/conventions/enforce-by-mechanism.md#Rules
- knowledge/conventions/shell-parsing.md#Single Parser Rule
- knowledge/conventions/hexagonal-architecture.md#Port Placement Rules
- knowledge/conventions/bash-write-guard.md#Overview
- .claude/rules/04-coding-principles.md#No Panics in Library Code
- .claude/rules/04-coding-principles.md#Error Handling: Result and ? Operator

## Signal Summary

### Stage 1: Spec Signals
🔵 43  🟡 0  🔴 0

