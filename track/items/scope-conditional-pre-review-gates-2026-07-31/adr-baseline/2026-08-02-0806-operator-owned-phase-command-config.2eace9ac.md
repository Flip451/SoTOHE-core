---
adr_id: "2026-08-02-0806-operator-owned-phase-command-config"
decisions:
  - id: D1
    user_decision_ref: "chat:2026-08-13:本文確認・修正指示の上で ADR 全体を承認"
    status: proposed
  - id: D2
    user_decision_ref: "chat:2026-08-13:本文確認・修正指示の上で ADR 全体を承認"
    status: proposed
  - id: D3
    user_decision_ref: "chat:2026-08-13:本文確認・修正指示の上で ADR 全体を承認"
    status: proposed
  - id: D4
    user_decision_ref: "chat:2026-08-13:本文確認・修正指示の上で ADR 全体を承認"
    status: proposed
  - id: D5
    user_decision_ref: "chat:2026-08-13:本文確認・修正指示の上で ADR 全体を承認"
    status: proposed
---
# phase command と pre-review gate をテンプレート利用者所有の argv 配列で宣言する

## Context

config で宣言したいものは二つある。一つは、phase の writer を実行する前に通す事前 command である。
もう一つは、scope に応じて実行する pre-review gate である。

config の値を、実行内容を表す Rust 内部の enum 値や、検査を識別する内部 ID にする形には問題がある。
その値と実際の処理との対応表を内部に置くと、config を書くテンプレート利用者から実行内容が見えない。公開された `bin/sotp`
interface を見ても、どの値で何が起きるのかを理解できず、選ぶこともできない。

phase を進める順序を prompt の文章だけに持たせる形にも問題がある。どの writer の前にどの事前 command を
通すかを機構が再現できない。そのため、正しい順序が守られる保証がない。

template config はテンプレート利用者所有である。有効な公開 command とその順序を選ぶ責任はテンプレート利用者にある。
一方、各 command が何を行い、いつ non-zero を返すかという意味は `bin/sotp` が所有する。
phase engine は subcommand の文法や意味を複製してはならない。

## Decision

### D1: config は任意の実行体の literal argv 配列を宣言する

phase command config は、実行内容を表す Rust 内部の enum 値、検査を識別する内部 ID、または型付きの内部対応表を引く
key ではなく、任意の実行体の literal argv 配列を保持する。典型的にはテンプレート利用者が公開された `bin/sotp` command を宣言するが、
実行体の allowlist は置かず、今後も導入しない。command 全体を一つの文字列で書く形式は受理しない。
引数ごとに分けた配列だけを受理する。実行体に固有の制限は、sotp basename と
phase enter / review local / review fix-local の組み合わせを拒否する再帰呼び出し denylist、および D5 の
fail-closed validation だけである。
<!-- illustrative, non-canonical -->
たとえば `bin/sotp signal check-spec-adr` は `["bin/sotp", "signal", "check-spec-adr"]` と表す。
config を書くテンプレート利用者は有効な command と順序を選び、engine は argv を書き換えたり意味解釈したりしない。

### D2: 各 phase は writer と順序付き事前 command を宣言する

各 phase declaration は canonical writer command を一つと、writer の前に通過すべき事前 command の
順序付き list を持つ。phase engine は事前 command を宣言順に一つずつ実行し、最初の non-zero exit で停止して
残りと writer を実行しない。すべてが zero の場合に限り writer を一度実行する。

この機構は `2026-07-22-0400-sot-reentry-sequencing.md` D6 を refine し、prompt に書かれた phase を進める順序を
テンプレート利用者所有の command 宣言によって機械実行可能にする。各事前 command の意味と
zero/non-zero の契約は引き続き `bin/sotp` が所有する。

### D3: usecase が sequencing を、infrastructure が bounded process execution を所有する

- usecase layer は、「どの順で、何個の command を実行し、失敗したらどこで止めるか」という進行を受け持つ。
  phase declaration を解決し、事前 command を一つずつ順に実行して、最初の non-zero exit で残りを止める。
  すべて成功した場合だけ writer を一度起動する。個々の command の意味は知らず、argv を順に runner へ渡す。
- infrastructure layer は、「一つのプロセスを制限付きで安全に最後まで動かすこと」を受け持つ。
  argv、repository-root cwd、timeout、output bound を受け取る汎用の `ProgramInvocation` runner を実装してよい。
  CLI subcommand を解釈せず、Clap/CLI grammar に依存しない。また、何個中の何個目かや、失敗後に次を
  実行するかどうかも知らない。
- CLI layer は phase の enter / explain / validate の表玄関だけを公開し、command の意味を重複して実装しない。

infrastructure が進行まで知ると、runner が CLI の文法や意味を解釈し始め、D1 の「argv を意味解釈しない」
という境界が壊れる。usecase がプロセス実行まで受け持つと、timeout、output bound など、OS に依存する処理が
business layer に混ざる。

### D4: pre-review gate も scope ごとの ordered argv 配列として宣言する

reviewer を起動する前に何を検査するかは、review scope ごとに、任意の実行体の argv 配列として
`pre-review-gates.json` に実行順で宣言する。典型的には公開された `bin/sotp` command を宣言するが、実行体の
allowlist は置かない。

`sotp review local` は scope を解決し、その scope に宣言された argv を上から順に実行する。最初の
non-zero exit で review 入口を閉じ、reviewer を起動しない。すべてが zero の場合は reviewer を起動する。
argv 配列が空の scope には pre-review gate がない。

<!-- illustrative, non-canonical -->
たとえば usecase scope では、`bin/sotp signal calc-impl-catalog` →
`bin/sotp signal check-impl-catalog --gate commit` → `bin/sotp task-contract coverage` →
`bin/sotp task-contract check` の順に宣言する。

scope ごとに宣言するのは、必要な検査が scope によって異なるためである。cargo-make の `dependencies` は
CLI が scope を解決する前に動くため、planning / SoT scope で downstream liveness の検査だけを省けない。
そこで coverage-before-check の順序は保ったまま、その所有を scope-blind な cargo-make `dependencies` から、
`sotp review local` が scope を解決した後のテンプレート利用者所有の argv 実行へ移す。

argv 配列にする理由は D1 と同じである。gate-kind から内部処理への見えない対応表を置かず、config を
読めば実行する command と順序を監査できるようにする。Makefile wrapper は薄い委譲に留め、scope 条件分岐や
gate dependency を重複して所有しない。

この decision は `2026-07-30-1036-scope-conditional-pre-review-gates.md` D1 / D2 を refine し、
scope × gate の宣言を、テンプレート利用者が公開 CLI だけで監査できる scope × ordered command declaration に具体化する。

また `2026-06-27-0852-pre-review-task-contract-conformance-gate.md` D6 の配線所有を modify する。

### D5: invocation config と実行を fail-closed に検証する

テンプレート利用者の config は疑ってから実行する。検証できなければ実行前に non-zero で止まり、黙って続行しない。

phase command config と pre-review gate config は、少なくとも次を検証する。

- schema version — 解釈できない形式を別の形式として読み違えないため。
- 各 argv の非空 — 空の宣言が「何もせず成功」として通過しないため。
- phase または scope declaration の一意性 — 同じ対象の宣言が重複し、どれを使うか曖昧にならないため。
- repository-root 固定 cwd — command を呼び出した場所によって結果が変わらないため。
- bounded output — command の出力が上限なく増え続けないため。
- 明示 timeout または約 60 分の既定 timeout — 終わらない command が phase や review 全体を止め続けないため。
- phase engine の再帰呼び出し — config が sotp を宣言し、sotp が sotp を繰り返し起動しないため。

argv は shell を経由せず、そのまま実行する。機構は shell interpolation、pipe、暗黙の `sh -c` を提供しないため、
config の文字列を暗黙に展開・分解・注入する経路を作らない。テンプレート利用者が shell 実行体を明示的に宣言する自由は、
D1 / D4 の範囲に含まれる。

判定できない場合は拒否に倒す。検証不能、重複、再帰、または上限超過は、実行前または検出時に non-zero で停止し、
その結果を D2 / D4 の first-failure stop へ渡す。

## Rejected Alternatives

- **prompt だけに固定した順序制御**: テンプレート利用者所有の config から実行順を監査できず、writer 前の事前 command の
  機械強制にもならないため却下。
- **Rust 内部の enum 値と非公開の対応表**: config を書くテンプレート利用者に非公開の Rust 対応表の知識を要求し、公開 CLI と config の
  間に第二の command 用語体系を作るため却下。
- **検査を識別する内部 ID と見えない実行対応表**: scope config が実際に呼ぶ command と順序を表さず、
  テンプレート利用者が `bin/sotp` interface だけで検証できないため却下。
- **shell command string**: 引用符の解釈、文字列展開、pipe、`sh -c` の動作と注入経路を持ち込み、
  argv を意味解釈せず実行する場合より検証境界が広がるため却下。
- **source-only baseline-restoration carrier**: 撤回済みの draft ADR `2026-08-01-1749` では restoration task に
  attribution 先を与える仕組みとして検討した。しかし shipped default config が impl-catalog の検査を
  task-contract の検査より先に実行するため不要になり、実在しない entry を台帳に混ぜることから却下した。
  出自: `user-directed-design-refinement:scope-conditional-pre-review-gates-2026-07-31:operator-owned-cli-command-arrays:carrier-removal`。
- **架空の catalogue entry または架空の参照帰属**: 実在しない最終契約を作って
  `task-contract` の帰属対象にすると、catalogue entry の帰属記録という責務と通常の coverage の決まりを
  歪めるため却下。
- **source-only の 🔴 を review へ通す**: 復元対象の欠落を reviewer 入場後まで持ち越し、失敗しても通すため却下。
  宣言先行の 🟡 だけを既存 commit-gate の意味に従って許容する。

## Consequences

### Positive

- テンプレート利用者は config と公開 `bin/sotp` interface だけで phase の事前 command と pre-review gate の実行順を監査できる。
- phase engine は exit status による進行制御に限定され、CLI が持つ意味の二重実装を避けられる。
- argv、cwd、timeout、output、recursion の境界を、判定不能なら拒否する共通の実行契約として検証できる。
- Makefile wrapper を薄く保ったまま、`sotp review local` と phase engine が実行を所有できる。

### Negative

- command 名や argv の変更時はテンプレート利用者所有の config の同期が必要になる。
- config を書くテンプレート利用者は有効な command と順序を選ぶ責任を負い、誤設定は validation または command の
  non-zero exit として顕在化する。
- generic process runner には timeout、output capture、recursion detection の実装と診断が必要になる。

## Reassess When

- 公開 `bin/sotp` interface だけでは安全な phase orchestration を表現できない command 種別が実証されたとき。
- argv declaration の互換性維持費が、テンプレート利用者から実行内容が見える利点と config portability の便益を上回ったとき。
- repository-root process execution より狭い権限境界が必要になったとき。

## Related

- `knowledge/adr/2026-07-30-1036-scope-conditional-pre-review-gates.md` D1 / D2 — scope 条件付き
  pre-review gate 宣言と CLI dispatch の refinement 対象。
- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` D6 — prompt に書かれた phase を進める順序を
  テンプレート利用者所有の phase command config で機械実行可能にする refinement 対象。
- `knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md` D1 / D5 / D6 —
  `task-contract` の catalogue-entry attribution と coverage / liveness の責務境界。D6 は
  coverage-before-check の順序を維持したまま、cargo-make dependency 所有をテンプレート利用者所有の ordered argv
  配線へ移す明示的な modification target。
- `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md` D3 — commit gate における
  impl-catalog の interim semantics。
