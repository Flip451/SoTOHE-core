---
adr_id: 2026-06-12-1518-hooks-path-setup-fail-closed
decisions:
  - id: D1
    user_decision_ref: "chat:2026-06-13 hooksPath 未設定時は fail-closed にするという判断"
    candidate_selection: "from:[direct-git-only-runtime-check, agent-runtime-setup-preflight, wrapper-language-ban] chose:agent-runtime-setup-preflight"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-06-13 SRP 違反を避けるという判断"
    candidate_selection: "from:[fold-into-block-direct-git-ops, dedicated-hooks-path-setup-handler] chose:dedicated-hooks-path-setup-handler"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-06-13 運用配布物は sotp と docs であり cargo run fallback は前提外という判断"
    candidate_selection: "from:[cargo-run-fallback, installed-sotp-or-fail-closed] chose:installed-sotp-or-fail-closed"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-06-13 GitHub Actions CI を壊さないという制約"
    candidate_selection: "from:[remove-all-cargo-run-invocations, keep-source-ci-cargo-run-outside-distribution-hooks] chose:keep-source-ci-cargo-run-outside-distribution-hooks"
    status: proposed
---
# hooksPath 未設定時の runtime fail-closed を agent 実行面の setup preflight に分離する

## Context

`knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md` は、git 書き込みガードの enforcement 点を Claude Code のコマンド文字列推測から git hooks の process-level enforcement へ移すことを決めた。特に D7 は、`core.hooksPath` 未設定を setup、CI verify、runtime fail-closed の三重で担保するとしている。

D7 の runtime fail-closed を「`block-direct-git-ops` が直接 git コマンドを検出したときだけ `core.hooksPath` を確認する」と実装すると、setup 前の最も危険な期間に穴が残る。`core.hooksPath` 未設定時は git hooks 層そのものが不在であり、直接 git 文字列だけを見て止めても、外部サブプロセスや wrapper 内部から git が起動される経路には届かない。問題の本質は Python や特定 wrapper で git を使うことではなく、git hooks が未配備の状態で任意の Bash 実行を進めてしまうことである。

また、運用配布物は `sotp` バイナリと運用ドキュメントであり、Claude Code hooks や Codex の実行前指示が source checkout を前提に `cargo run --quiet -p cli -- ...` へ fallback する設計は配布前提と合わない。一方で、SoTOHE 自身の CI や開発用 Makefile は source checkout 上で動作する内部経路であり、そこから `cargo run` を機械的に除去すると GitHub Actions CI を壊すリスクがある。

したがって、D7 の runtime fail-closed は「直接 git 検出時の追加チェック」ではなく、「agent が任意 Bash を実行する前に hooksPath 設定を確認し、未設定なら setup だけを許す preflight」として明確化する必要がある。

## Decision

### D1: hooksPath 未設定時は agent 実行面で runtime fail-closed にする

Claude Code の Bash PreToolUse hook と Codex の実行前ガードは、任意 Bash を進める前に repository-local `core.hooksPath` が `.githooks` を指していることを確認する。

未設定または不一致の場合、通常の Bash 実行は fail-closed とし、設定を促す remediation を stderr / 実行面の visible message に出す。許可する例外は setup のための最小コマンドだけに限定する。

- `cargo make bootstrap`
- `git config --local core.hooksPath .githooks`

この判断は「git を起動しそうな wrapper の中身を推測する」ことを目的にしない。hooksPath が未設定なら git hooks enforcement が未配備である、という状態そのものを block 条件にする。

### D2: setup preflight は `block-direct-git-ops` から分離する

`block-direct-git-ops` は、D7 以前から持っている直接 git サブコマンド検出、`SOTP_GUARDED_GIT` 文字列入力禁止、`bin/sotp` 上書き検出などの git operation policy に集中させる。

hooksPath setup preflight は独立した hook handler として実装し、責務を「hooksPath 設定済みなら allow、未設定なら setup コマンドだけ allow、それ以外は block」に限定する。これにより、setup 誘導の policy と直接 git 操作の policy が同じ handler に混在しない。

### D3: 配布対象 hooks は installed `sotp` を解決できなければ fail closed にする

Claude Code hooks などの運用配布対象は、次の順に `sotp` を解決する。

1. `SOTP_CLI_BINARY`
2. repository-local `bin/sotp`
3. `PATH` 上の `sotp`

いずれも利用できない場合は、source checkout 上の `cargo run --quiet -p cli -- ...` へ fallback せず fail closed にする。これは、hooks の配布前提が「source tree から Rust crate を build/run できること」ではなく「配布済み `sotp` と運用ドキュメントで agent を誘導できること」だからである。

### D4: source checkout / CI 用の `cargo run` は配布対象 hooks と分けて扱う

Makefile や CI 内部の `cargo run --quiet -p cli -- ...` は、SoTOHE 自身の source checkout を前提にした開発・検証経路であり、配布対象 hooks の fallback とは別物として扱う。

そのため、配布対象 hook command から `cargo run` fallback を除去しても、Makefile の内部タスクから機械的に `cargo run` を除去する必要はない。特に GitHub Actions CI を壊さないことを優先し、source checkout 前提の検証タスクは既存の CI 契約が成立する形を維持する。

## Rejected Alternatives

### A. `block-direct-git-ops` に hooksPath setup policy を同居させる

却下する。直接 git 操作の検出と、agent 実行前の setup preflight は責務が異なる。同じ handler に同居させると、SRP に反し、将来の変更時に「git operation policy を直したつもりで setup policy が変わる」またはその逆の drift を生みやすい。

### B. Python や wrapper からの git 利用を禁止する

却下する。問題は Python で git を使うことでも、特定 wrapper が git を spawn することでもない。問題は `core.hooksPath` が未設定で git hooks enforcement が存在しない状態を fail-open にすることである。wrapper 言語や実装方式を deny list 化しても、別の entry point が増えるたびに同じ穴が再発する。

### C. 配布対象 hook で `cargo run --quiet -p cli -- ...` に fallback する

却下する。運用配布対象は `sotp` とドキュメントであり、配布先が Rust workspace と cargo build 環境を持つとは限らない。`cargo run` fallback は `sotp` 配布不備を隠し、missing CLI を fail closed にできない。

### D. CI verify だけで hooksPath 未設定を検出する

却下する。CI verify は merge gate として有効だが、agent が local workspace で任意 Bash を実行する時点の事故防止にはならない。runtime preflight が必要である。

### E. Makefile 内の `cargo run` も一律に除去する

却下する。Makefile は SoTOHE 自身の source checkout と GitHub Actions CI の内部経路であり、配布対象 hook command とは前提が異なる。CI を壊してまで機械的に同一視しない。

## Consequences

### Positive

- hooksPath 未設定状態が agent 実行面で早期に visible failure になり、git hooks enforcement の未配備を見逃しにくくなる。
- setup 誘導が専用 handler に分離され、直接 git 操作ガードの責務が肥大化しない。
- 配布対象 hooks が source checkout 前提の `cargo run` fallback に依存しなくなる。
- Makefile / CI の内部経路は source checkout 前提として維持でき、GitHub Actions CI を不用意に壊さない。

### Negative

- 新規 clone 直後や `sotp` 未配備の環境では、agent の Bash 利用が setup 完了まで強く制限される。
- setup preflight 用の hook handler と CLI dispatch 名が増えるため、type catalogue と orchestra verification の追従が必要になる。
- Codex 側は Claude Code のような hook 実行面を持たない環境があるため、同じ決定を instruction / preflight policy として表現する必要がある。

### Neutral

- 人間の手元ターミナル操作を新たに制限するものではない。
- GitHub Actions CI や source checkout 前提の内部検証タスクは、配布対象 hook command とは別契約として扱う。
- `block-direct-git-ops` の直接 git サブコマンド検出は、setup preflight 分離後も残る。

## Reassess When

- Codex 側に Claude Code と同等の hook 実行面が導入され、instruction ではなく実 hook として setup preflight を配布できるようになったとき。
- `sotp` 配布形態が変わり、repository-local `bin/sotp` や `PATH` 解決以外の標準設置先が必要になったとき。
- setup 前の Bash 制限が通常開発フローに過剰な摩擦を生む実例が観測されたとき。
- GitHub Actions CI が `cargo run` を経由しない配布済み `sotp` 実行へ移行できる状態になったとき。

## Related

- `knowledge/adr/2026-06-10-1630-git-hooks-process-level-enforcement.md` — 親 decision。D7 の runtime fail-closed を本 ADR が具体化する。
- `knowledge/adr/2026-03-11-0050-fail-closed-hooks.md` — fail-closed hook policy の既存判断。
- `knowledge/adr/2026-04-09-2323-python-hooks-removal.md` — 配布 hooks を Python script から Rust hook dispatch へ寄せた背景。
- `.claude/settings.json` — Claude Code hook command の配布面。
- `.codex/instructions.md` — Codex 実行前 policy の配布面。
- `Makefile.toml` — source checkout / CI 内部経路。
