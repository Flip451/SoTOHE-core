---
adr_id: 2026-06-10-1630-git-hooks-process-level-enforcement
decisions:
  - id: D1
    user_decision_ref: "adr:add hearing 2026-06-11"
    candidate_selection: "from:[cc-hook-string-scan, deny-list-expansion-plus-hook, git-hooks-process-level] chose:git-hooks-process-level"
    status: proposed
  - id: D2
    user_decision_ref: "adr:add hearing 2026-06-11"
    candidate_selection: "from:[token-only-pass, claudecode-env-discrimination, nonce-file-token] chose:token-only-pass"
    status: proposed
  - id: D3
    user_decision_ref: "adr:add hearing 2026-06-11"
    candidate_selection: "from:[no-token-protection, bash-keyword-scan, bash-plus-write-edit-scan] chose:bash-keyword-scan"
    status: proposed
  - id: D4
    user_decision_ref: "adr:add hearing 2026-06-11"
    candidate_selection: "from:[keep-all-string-scans, keep-precise-checks-and-0080-blocks, drop-all-scan-era-blocks-keep-live-checks] chose:drop-all-scan-era-blocks-keep-live-checks"
    status: proposed
  - id: D5
    user_decision_ref: "adr:add hearing 2026-06-11"
    candidate_selection: "from:[expand-deny-list, keep-deny-as-is] chose:keep-deny-as-is"
    status: proposed
  - id: D6
    user_decision_ref: "adr:add hearing 2026-06-11"
    candidate_selection: "from:[stdout-status-quo, write-reason-to-stderr] chose:write-reason-to-stderr"
    status: proposed
  - id: D7
    user_decision_ref: "adr:add hearing 2026-06-11"
    candidate_selection: "from:[manual-setup-only, setup-plus-ci-verify, setup-plus-ci-verify-plus-runtime-fail-closed] chose:setup-plus-ci-verify-plus-runtime-fail-closed"
    status: proposed
---
# git 書き込みガードの enforcement を git hooks 層へ移行する

## Context

git 操作ガードは `libs/domain/src/guard/policy.rs` に単一の Claude Code PreToolUse フック (`block-direct-git-ops`) として実装されており、Bash コマンド文字列の解析によって git 書き込みを検出している。この「コマンド文字列から git 起動を推測する」方式は以下の構造的課題を抱えていた。

**誤発火 (false positive)**: `command_contains_git` は非 git コマンドの argv トークンと redirect テキストに対して `"git"` の大小無視部分文字列検索を行う。`ls .git/hooks/`、`GIT_DIR` を含む grep パターン、`git_cli/mod.rs` というファイルパスなど、正当な非 git 操作が silently block される (本 ADR の検討セッション中にも 3 連続で発生)。これは `knowledge/adr/2026-03-11-0080-guard-policy-ban-patterns.md` の Consequences で SEC-11 として記録された known issue であり、同 ADR の Reassess When 条件「SEC-11 の false positive がユーザー体験に影響する場合」に該当する。回避のため briefing-file 迂回などのワークアラウンドが `CLAUDE.md` guardrails に蓄積していた。

**検出漏れ (false negative) と構造的ギャップ**: Claude Code フックは Claude Code 自身の tool call しか intercept できない。外部サブプロセス (Codex CLI `--sandbox workspace-write` 等) 内の git 操作には届かないことが `CLAUDE.md` に既知のギャップとして明記されている。コマンド文字列スキャンをどれだけ精緻化してもこのギャップは閉じない。

**silent block**: フックが block verdict を返すとき、reason 文字列が stdout に流れている。Claude Code は終了コード 2 のとき stderr のみを AI に提示するため、block が「No stderr output」となり remediation 指示が届かない。

**残滓化した file-write ガード (CON-07)**: 出力リダイレクト一括 block・`tee`・`sed -i` の block は `knowledge/conventions/bash-write-guard.md` (CON-07) に由来し、その動機は「Bash 経由の file write が file-lock hooks (`file-lock-acquire`/`file-lock-release`) を迂回するのを防ぐ」ことだった。しかし file-lock hooks は既に撤去されており、リポジトリ内で `file-lock` に言及するのは CON-07 自身の 1 行のみ。保護対象を失ったガードが `2>/dev/null` 等の標準シェル idiom へのフリクションだけを生んでいる。

先行検討として「`permissions.deny` の大量拡張 + フック文字列スキャンの縮小」(3 層防御案) をドラフトしたが、検証の結果、(1) heredoc 経由 (`bash <<'SH' … git add … SH`) や `find -exec git add` など現在 block されている経路が解禁される後退、(2) entry-point 列挙 (`bash -c` / `bash -lc` / …) は `bash -xc` 等の変形に対して構造的に不完全、(3) `python3 -c "print('hello')"` のような無害な利用まで一律 deny される新フリクション、が判明した。

本質的な保護目標は「**guarded path (sotp ラッパー) 以外からの生の git 書き込みを防ぐ**」ことだけである。これはコマンド文字列の推測ではなく、git の hooks 機構 (process-level) で git 操作そのものを捕捉すれば、誤発火と検出漏れの両方が構造的に消える。

## Decision

### D1: git hooks を git 書き込みの enforcement 点とする

リポジトリ内 `.githooks/` に git hooks を配置し、`core.hooksPath` で有効化する。

| git hook | 捕捉対象 | 備考 |
| --- | --- | --- |
| `reference-transaction` (prepared state) | ローカル ref 更新すべて — commit、branch 作成/削除、merge、rebase、cherry-pick、`reset <rev>`、stash、notes、fetch (remote-tracking) | non-zero exit でトランザクション abort。**`--no-verify` で迂回不可** |
| `pre-push` | push | `--no-verify` で迂回可 (許容残余) |

hook 本体は 2 行程度のシェルシムとし、`bin/sotp hook dispatch` の新しい hook 名 (例: `git-ref-update` / `git-pre-push`) へ exec する。判定ロジックは既存の Rust hook アーキテクチャ (domain 層 policy + usecase 層 dispatch) に乗せ、`policy.rs` のルール別 remediation メッセージ定数を再利用する。

<!-- illustrative, non-canonical -->
```sh
#!/bin/sh
# .githooks/reference-transaction
exec "$(git rev-parse --show-toplevel)/bin/sotp" hook dispatch git-ref-update "$@"
```

git hook の stderr は Bash tool の実行結果としてそのまま AI に表示されるため、block 理由と remediation 指示が自然に届く。

`git switch` / `git checkout <existing-branch>` の HEAD symbolic-ref 切替が `reference-transaction` で捕捉されるかは実装時に検証する。reference-transaction が発火しない場合でも、D4 で維持する直接 git サブコマンドチェック（policy.rs の `git switch` チェックを含む）が Claude Code Bash tool からの直接呼び出しを覆う。外部サブプロセス経由の `git switch` は許容残余となる（process-level enforcement の射程は基本的にすべての git プロセスを対象とするが、reference-transaction が発火しないケースは外部サブプロセスを含め未検証の残余リスクとして残る）。

### D2: `SOTP_GUARDED_GIT` 通行証による単一判定モデル

git hook の判定は環境変数 1 つのみ:

- 環境変数 `SOTP_GUARDED_GIT` が提示されている → 許可 (guarded path)
- それ以外 → block (remediation メッセージ付き)

人間 / AI の判別 (例: `CLAUDECODE` 環境変数の有無) は行わない。判定変数を 1 つに絞ることで、判別変数の unset 系操作 (`env -u` / 空代入) という対称な攻撃面が存在しなくなり、fail mode が fail-closed (トークンがなければ block) に統一される。

- **正規経路**: sotp の git 呼び出しは `libs/infrastructure/src/git_cli/` の `Command::new("git")` に集約されている。この単一通過点で子プロセス環境に `SOTP_GUARDED_GIT` を設定する。`cargo make track-commit-message` / `track-add-paths` / `track-branch-create` / `track-pr-push` 等のラッパーはすべて sotp 経由なので自動的に通行証を持つ。
- **人間の ad-hoc 操作**: 自分のターミナルで `SOTP_GUARDED_GIT=1 git rebase …` のように通行証を自己提示する。D3 のキーワードスキャンは Claude Code の Bash tool にしか作用しないため、人間には制約がない。
- AI の `git fetch` / `git stash` / `git checkout <branch>` 等、従来フックを素通りしていた ref 更新操作も block 対象になる。必要な操作はラッパー経由 (`cargo make track-switch-main` 等) に誘導する。

### D3: Claude Code フックによるトークン文字列のインライン入力禁止

`block-direct-git-ops` フックに「Bash コマンド文字列に `SOTP_GUARDED_GIT` (exact match) が含まれていたら block」を追加する。

これによりトークンは AI 自身の Bash コマンドから入力不可能になり、`SOTP_GUARDED_GIT=1 git commit` や `bash -c 'SOTP_GUARDED_GIT=1 …'` といったインライン偽装が成立しない。トークンの注入点は sotp バイナリ内部 (Rust コード) に限定される。

- 適用範囲は **Bash コマンド文字列のみ**。Write/Edit の content はスキャンしない — ガード実装自体 (`git_cli/`、hook 実装、テスト) がこの文字列を正当に含み、保守のたびに誤発火するため。
- スクリプトファイル間接実行 (Write でトークンを含むスクリプトを書いて `bash script.sh` で実行) は複数ステップの意図的回避であり、許容残余とする (Consequences 参照)。
- `SOTP_GUARDED_GIT` は exact match の固有文字列であり、`"git"` 部分文字列スキャンと異なり通常ワークフローでの誤発火は実質ゼロ (発火し得るのはガード機構自体を Bash の grep で触る場合のみで、専用 Grep/Read ツールで代替可能)。
- 実装上、スキャンは raw コマンド文字列と quote-strip 済み argv トークンの**両方**に適用する (`SOTP_GUARDED_GI"T"=1` のようなクォート分割による回避を防ぐ)。eval / 変数展開による実行時の文字列合成はスキャンの射程外であり、スクリプト間接実行と同じ意図的回避クラスとして許容残余に置く (Consequences 参照)。

### D4: Claude Code フックの縮小 — 文字列スキャン時代の一括 block (0080 由来 + CON-07) の撤廃

**撤廃する**:

- `command_contains_git` — 非 git コマンドへの `"git"` 部分文字列スキャン (`2026-03-11-0080` Decision 4)。同スキャンが捕捉していた heredoc / `find -exec` / 多重ネスト経路は、D1 の git hooks が実プロセスレベルで代替する。
- `env` 無条件 block (`2026-03-11-0080` Decision 1) — 根拠は「`env -S` 等の引数形式を文字列スキャンが解析しきれない」という文字列スキャン方式の盲点保護だった。token-only fail-closed モデル (D2) では環境操作はトークンを**消す**方向にしか働けず (消えれば block 側)、トークンの**付与**は D3 のキーワードスキャンが塞ぐため、根拠が消滅している。
- `$` / バッククォート展開 block (`command_contains_expansion`、`2026-03-11-0080` Decision 2) — 根拠は「`$CMD` の展開先を文字列スキャンが知り得ない」という同種の盲点保護。実プロセス enforcement では展開先が git でも token なしなら git hook が block する。撤廃により `$HOME` / `$(...)` 等、git と無関係な変数参照・コマンド置換への常時フリクションが消える。残る「eval による実行時のトークン文字列合成」は意図的回避クラスとして許容残余に置く (D3 / Consequences 参照)。
- `has_output_redirect` 一括 block — `>`, `>>`, `>|`, `<>`, `2> file` の解禁 (`2>&1` / `1>&2` の FD 複製は従来から block されていない)。
- `FILE_WRITE_COMMANDS` (`tee`) と `sed -i` チェック — CON-07 の file-lock 保護という存在理由が消滅しているため。`knowledge/conventions/bash-write-guard.md` は本決定に合わせて改訂する。

0080 の Decision 1・2・4 は本決定が supersede する。

**維持する** (誤発火ゼロかつ根拠が生きている精密チェック):

- 直接 git サブコマンドチェック (`git add` / `commit` / `push` / `switch` / `merge` / `rebase` / `cherry-pick` / `reset` / `branch -d` / `checkout -b`) — 特に `git add` は index 更新に発火する git hook が存在しないため、Claude Code フックが引き続き一次防御となる。
- launcher stripping (`sudo` / `nohup` / `timeout` 等 — `COMMAND_LAUNCHERS`)・`VAR=val` skip・`.exe` suffix strip (`2026-03-11-0080` Decision 3) — これらは block ルールではなく、上記サブコマンドチェックが `sudo git add` 等の launcher 越し呼び出しを見抜くための解析基盤。
- `bin/sotp` 上書き検出 (`is_bin_sotp_overwrite`) — 出自は CON-07 ではなく glibc 不一致防止 (`cargo make build-sotp` への誘導) であり根拠が生きている。

**追加する**:

- D3 のトークンキーワードスキャン。
- `block-test-file-deletion` にリダイレクト先チェックを追加 — リダイレクト解禁で可能になる `> tests/foo.rs` 形式のテストファイル truncation を、redirect ターゲットのテストファイルパターン照合で block する (`SimpleCommand.redirect_texts` は既に取得済み)。

### D5: `permissions.deny` は拡張しない

先行ドラフトが計画していた deny 項目の大量追加 (`bash -c` / `python3 -c` 系 wrapper 約 20 項目 + 危険 git サブコマンド約 12 項目) は行わない。entry-point 列挙はフラグ変形 (`bash -xc`、`bash -l -c` 等) に対して構造的に不完全であり、git hooks がプロセスレベルで同じ目的をより確実に達成する。既存の `permissions.deny` と `FORBIDDEN_ALLOW` は現状維持。

### D6: フックの block 理由を stderr に出力する

フックが block verdict を返す (終了コード 2) とき、reason 文字列を stdout ではなく stderr へ書き出す。現状 `apps/cli-composition/src/hook.rs` は block verdict を `CommandOutcome { stdout: Some(reason), stderr: None, exit_code: 2 }` として返し、`apps/cli/src/commands/hook.rs` が stdout を `println!` で出すため、Claude Code (exit 2 で stderr のみ提示) には届かず block が silent になっている。CLI レイヤーの修正 (stdout → stderr) で、各 block が self-describing になり remediation 指示 (どのラッパー / どの `/track:*` コマンドを使うか) が AI に届く。

### D7: 配備と fail-closed の三重担保

`core.hooksPath` が未設定だと git hooks 層は不在になる。以下の三重で担保する:

1. **setup**: `cargo make bootstrap` / `/track:setup` が `git config core.hooksPath .githooks` を設定する。
2. **CI verify**: `bin/sotp verify` 系サブコマンドが設定値を検査し、未設定 / 不一致なら `cargo make ci` を fail させる。
3. **runtime fail-closed**: `block-direct-git-ops` フックが実行時に `core.hooksPath` 未設定を検知したら git コマンドを block する (fail-closed hooks の系譜 CN-04 と整合)。

## Rejected Alternatives

### A. 3 層防御案 — `permissions.deny` 大量拡張 + フック文字列スキャン縮小 (先行ドラフト)

`bash -c` / `python3 -c` 等の entry-point を deny 列挙し、フックの文字列スキャンを縮小する案。却下: (1) heredoc / `find -exec` など現在 block されている経路が解禁される後退が発生、(2) フラグ変形に対する列挙の構造的不完全性、(3) 無害な `python3 -c` まで一律 deny する新フリクション。enforcement 点をコマンド文字列に置く限り false positive / false negative のトレードオフから抜けられない。

### B. `CLAUDECODE` 環境変数による人間 / AI 判別

git hook で「`SOTP_GUARDED_GIT` あり → 許可、`CLAUDECODE` あり → block、どちらもなし (人間) → 許可」とする案。却下: 判定変数が 2 つに増え、`CLAUDECODE` の unset 系操作という対称な攻撃面が生まれる。fail mode も fail-open (判別変数が消えると人間扱いで許可)。人間の利便性はトークン自己提示 (D2) で足りるため、判別変数は不要。

### C. 現状維持 — フック単独 + 文字列スキャン

却下: SEC-11 誤発火と外部サブプロセスギャップが解消しない。本セッション中にも誤発火が 3 連続で発生しており、ユーザー体験への影響は Reassess 条件を満たしている。

### D. nonce ファイル方式のトークン強化

sotp が乱数 nonce を生成しファイルと環境変数で照合する案。却下: D3 のキーワードスキャンによりインライン偽装は既に不可能であり、残るスクリプト間接実行は nonce でも防げない (スクリプトは nonce 機構ごと再実装できないが、人間向け escape hatch を塞いでしまう)。複雑さに見合う追加防御がない。

### E. `git-cli.sh` 等の新規ラッパースクリプト導入

却下: 既存の `cargo make` / `bin/sotp` ラッパー群が同じ役割を既に提供している。新規ラッパーは抽象階層の重複になる。

### F. キーワードスキャンの Write/Edit content への拡大

スクリプトファイル間接実行も塞ぐ案。却下: ガード実装自体の Rust ソース・テスト・ドキュメントが `SOTP_GUARDED_GIT` を正当に含むため、保守のたびに誤発火する。パス allowlist の保守コストが発生し、「誤発火ゼロ」という本再設計の中心価値を損なう。間接実行は意図的回避として許容残余に置く。

### G. 0080 由来の `env` 無条件 block / `$`・バッククォート展開 block の維持

本 ADR の初期ドラフトはこの 2 つを「精密チェック」として維持していた。却下: 両者の根拠は「文字列スキャンが env の引数形式 / 展開後の文字列を解析できない」という**文字列スキャン方式の盲点保護**であり、enforcement が実プロセス (git hooks) に移った時点で消滅している。残余価値 (eval による実行時トークン合成の防止) はスクリプト間接実行と同じ意図的回避クラスで、片方だけ blanket ban するのは一貫しない。特に展開 block は `$HOME` / `$(...)` への常時フリクションという実害を伴っていた。

## Consequences

### Positive

- **誤発火の構造的解消**: enforcement 点が「コマンド文字列の推測」から「実 git プロセスの捕捉」に移ることで、grep / echo / ファイルパス / briefing 文中の "git" への誤 block が消える。`CLAUDE.md` の briefing-file 迂回・`2>/dev/null` 禁止などのワークアラウンド群が不要になる。
- **ref 更新操作のカバレッジ拡大**: heredoc・`find -exec`・多重ネスト・外部サブプロセス (Codex CLI workspace-write) — どの経路でも `reference-transaction` が発火する ref 更新 (commit / branch / merge / rebase / push 等) は実 git 起動時点で捕捉される。`CLAUDE.md` に記録されていた「フックは外部サブプロセスに届かない」という既知ギャップのうち ref 更新クラスが閉じる。`git add` (index 更新) と ref を動かさない worktree 破壊は git hooks の捕捉範囲外であり、外部サブプロセス経由は Negative に記載の残余リスクとして残る。
- **`reference-transaction` は `--no-verify` で迂回できない**。
- **block が self-describing になる**: git hook の stderr + D6 の修正により、remediation 指示が確実に AI に届く。
- **設定の簡素化**: deny リスト拡張が不要になり、フックのポリシーコードも縮小する。

### Negative

- **セットアップ依存**: `core.hooksPath` 未設定の間は git hooks 層が不在 (D7 で三重担保するが、新規 clone 直後は setup 実行までフックの fail-closed のみが防御)。
- **従来素通りだった操作の block**: AI の `git fetch` / `git pull` / `git stash` / `git checkout <branch>` / `git notes add` 等も block 対象になる。remediation はラッパー経由または人間のトークン提示。
- **意図的回避の許容残余**: スクリプトファイル間接実行 + 自己トークン付与、eval / 変数展開による実行時のトークン文字列合成、`git push --no-verify`。これらは複数ステップの意図的回避であり、「フックはセキュリティ境界ではなく事故防止と誘導」という framing の許容範囲。
- **git hooks で捕捉できない操作が残る**: `git add` (index 更新に hook が存在しない) は Claude Code フックの直接チェック (D4 維持分) が覆うが外部サブプロセス経由は残余リスク。ref を動かさない worktree 破壊 (`git reset --hard HEAD` / `git checkout -- <path>` / `git clean -f`) のうち `git reset` は D4 維持チェックが直接呼び出しを覆う。`git checkout -- <path>` と `git clean -f` は D4 の維持リストに含まれておらず、直接呼び出し・外部サブプロセス経由ともに残余リスク (必要な場合は実装フェーズで D4 維持チェックへの追加を検討)。

### Neutral

- 人間のターミナル操作は通行証の自己提示で従来通り可能。
- `policy.rs` の精密チェック群 (git サブコマンド / launcher 解析 / bin/sotp) とメッセージ定数は維持・再利用される。
- 読み取り専用 git コマンド (`status` / `diff` / `log` / `show` 等) は ref を更新しないため git hooks の影響を受けない。

## Reassess When

- Claude Code が外部サブプロセスへの hook 適用や、より表現力のある permission 制御を提供したとき (Layer 構成の再配分を検討)。
- 正当なラッパー経路がトークン非継承で block される事例が出たとき (トークン注入点の見直し)。
- スクリプトファイル間接実行による回避が実際に観測されたとき (Rejected F = Write/Edit content スキャンの再評価)。
- `reference-transaction` で捕捉できない git 書き込みクラスが新たに見つかったとき。

## Related

- `libs/domain/src/guard/policy.rs` — Claude Code フック policy 実装 (D3 追加 / D4 縮小の対象)
- `libs/infrastructure/src/git_cli/` — git 呼び出しの単一通過点 (D2 トークン注入点)
- `apps/cli-composition/src/hook.rs`, `apps/cli/src/commands/hook.rs` — D6 が対象とする CLI 経路
- `libs/usecase/src/hook_dispatch.rs` — `HookVerdictOutput.reason` の usecase 境界
- `.claude/settings.json` — `permissions.allow` / `permissions.deny` (D5: 現状維持)
- `knowledge/adr/2026-03-11-0080-guard-policy-ban-patterns.md` — Decision 1 (`env` block)・Decision 2 (展開 block)・Decision 4 (`command_contains_git`) は本 ADR の D4 が supersede する。存続は Decision 3 (`.exe` strip — 解析基盤) のみ
- `knowledge/conventions/bash-write-guard.md` — CON-07。D4 に合わせて改訂対象
- `knowledge/conventions/pre-track-adr-authoring.md` — ADR ライフサイクル convention
