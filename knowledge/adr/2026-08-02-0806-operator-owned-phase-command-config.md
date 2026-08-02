---
adr_id: "2026-08-02-0806-operator-owned-phase-command-config"
decisions:
  - id: D1
    review_finding_ref: "user-directed-design-refinement:scope-conditional-pre-review-gates-2026-07-31:operator-owned-cli-command-arrays"
    status: proposed
  - id: D2
    review_finding_ref: "user-directed-design-refinement:scope-conditional-pre-review-gates-2026-07-31:operator-owned-cli-command-arrays"
    status: proposed
  - id: D3
    review_finding_ref: "user-directed-design-refinement:scope-conditional-pre-review-gates-2026-07-31:operator-owned-cli-command-arrays"
    status: proposed
  - id: D4
    review_finding_ref: "user-directed-design-refinement:scope-conditional-pre-review-gates-2026-07-31:operator-owned-cli-command-arrays"
    status: proposed
  - id: D5
    review_finding_ref: "user-directed-design-refinement:scope-conditional-pre-review-gates-2026-07-31:operator-owned-cli-command-arrays"
    status: proposed
  - id: D6
    review_finding_ref: "user-directed-design-refinement:scope-conditional-pre-review-gates-2026-07-31:operator-owned-cli-command-arrays:carrier-removal"
    status: proposed
---
# phase command と pre-review gate を operator-owned argv 配列で宣言する

## Context

phase の writer 実行前 prerequisite と scope 条件付き pre-review gate を config で宣言しても、
config の値が Rust の predicate enum や gate ID に留まり、その実行対応表を内部 registry が所有すれば、
template config の author/operator は公開された `bin/sotp` interface だけでは挙動を理解・選択できない。
また prompt だけが phase の降下順序を所有する構成では、正しい writer と prerequisite の順序を機構で再現できない。

template config は operator-owned であり、有効な公開 command とその順序を選ぶ責任も operator にある。
一方、command の意味論は `bin/sotp` が所有しているため、phase engine が subcommand grammar を複製してはならない。

## Decision

### D1: config は公開 CLI の literal argv 配列を宣言する

phase command config は Rust predicate enum、semantic gate ID、または hidden typed predicate registry の key ではなく、
公開された `bin/sotp` command の literal argv 配列を保持する。shell string は受理しない。
<!-- illustrative, non-canonical -->
たとえば `bin/sotp signal check-spec-adr` は `["bin/sotp", "signal", "check-spec-adr"]` と表す。
config author/operator は有効な command と順序を選び、engine は argv を書き換えたり意味解釈したりしない。

### D2: 各 phase は writer と ordered pre-entry commands を宣言する

各 phase declaration は canonical writer command を一つと、writer の前に通過すべき pre-entry command の
ordered list を持つ。phase engine は pre-entry を宣言順に一つずつ実行し、最初の non-zero exit で停止して
残りと writer を実行しない。すべてが zero の場合に限り writer を一度実行する。

この機構は `2026-07-22-0400-sot-reentry-sequencing.md` D6 を refine し、prompt-level の降下規律を
operator-owned command declaration によって機械実行可能にする。各 prerequisite の command semantics と
zero/non-zero の契約は引き続き `bin/sotp` が所有する。

### D3: usecase が sequencing を、infrastructure が bounded process execution を所有する

orchestration/usecase layer は phase declaration の解決、pre-entry の順次処理、first-failure stop、writer dispatch
を所有する。infrastructure は argv、repository-root cwd、timeout、output bound を受け取る汎用の
`ProgramInvocation` runner を実装してよいが、CLI subcommand を解釈せず Clap/CLI grammar に依存しない。
CLI layer は phase の enter / explain / validate surface を公開し、command semantics の重複実装を持たない。

### D4: pre-review gate も scope ごとの ordered argv 配列として宣言する

`pre-review-gates.json` は各 review scope に適用する `bin/sotp` command argv 配列を実行順に宣言する。
gate-kind から内部 operation への不可視な mapping は置かない。`sotp review local` が scope を解決して
該当 command を順次 dispatch し、最初の non-zero exit で review 入口を閉じる。Makefile wrapper は
薄い委譲に留め、scope 条件分岐や gate dependency を所有しない。

この decision は `2026-07-30-1036-scope-conditional-pre-review-gates.md` D1 / D2 を refine し、
scope × gate の宣言を、operator が公開 CLI だけで監査できる scope × ordered command declaration に具体化する。

また `2026-06-27-0852-pre-review-task-contract-conformance-gate.md` D6 の配線所有を modify する。
coverage-before-check の順序は維持するが、その所有を scope-blind な cargo-make `dependencies` から、
`sotp review local` が scope 解決後に dispatch する operator-owned ordered argv sequence へ移す。
Makefile dependencies は CLI の scope 解決より先に実行されるため、既存 D6 の所有を保存したままでは、
planning / SoT scope に対して downstream liveness を条件付きで省略できず、scope-aware rule を満たせない。

### D5: invocation config と実行を fail-closed に検証する

phase command config と pre-review gate config は、少なくとも schema version、各 argv の非空、phase または
scope declaration の一意性、repository root 固定 cwd、bounded output、明示 timeout または約 60 分の既定
timeout、および phase engine の再帰呼び出しを検証する。検証不能・重複・再帰・上限超過は実行前または
検出時に non-zero で停止する。shell interpolation、pipe、暗黙の `sh -c` は提供しない。

### D6: source-only baseline restoration は direct signal check で review 前に検証する

`task-contract` は型カタログの実在 entry を task に帰属させる artifact に留め、恒久的な
source-only baseline-restoration target または carrier variant を追加しない。baseline restoration を予定する
task は、最終的なカタログ entry を捏造せず、implementation plan で復元対象の source と復元意図を特定してよい。

operator が宣言する implementation scope の review command sequence は、impl-catalog を再計算し、続けて
`bin/sotp signal check-impl-catalog --gate commit` を実行してから、`bin/sotp task-contract coverage` と
`bin/sotp task-contract check` を実行する。この phase-correct な direct signal check により、source-only の 🔴 が
残っていれば review 入口を閉じる一方、宣言先行の 🟡 は既存の commit-gate semantics に従って許容する。

この順序で bootstrap の必要性は解消されるため、source-only baseline-restoration carrier の仕組みは採用せず、
spec、type catalogue、source のいずれにも再導入しない。実在するカタログ entry に対する通常の
`task-contract` coverage / liveness invariant は維持し、implementation-first の例外を作らない。

## Rejected Alternatives

- **hard-coded prompt-only sequencing**: operator-owned config から実行順を監査できず、writer 前 prerequisite の
  機械強制にもならないため却下。
- **hidden Rust predicate registry**: config author に非公開の Rust mapping の知識を要求し、公開 CLI と config の
  間に第二の command vocabulary を作るため却下。
- **semantic gate ID と不可視な executable mapping**: scope config が実際に呼ぶ command と順序を表さず、
  operator が `bin/sotp` interface だけで検証できないため却下。
- **shell command string**: quoting、interpolation、pipe、`sh -c` の意味論と injection surface を持ち込み、
  opaque argv execution より検証境界が広がるため却下。
- **恒久的な source-only baseline-restoration carrier domain / schema**: implementation plan に source target と
  復元意図を記録し、review 前に impl-catalog を直接検査すれば足りるため、最終カタログに存在しない対象の
  恒久的な domain variant と schema surface を増やす必要がない。
- **fake catalogue entry または fake reference attribution**: 実在しない最終契約を作って
  `task-contract` の attribution 対象にすると、catalogue-entry attribution という責務と通常の coverage invariant を
  歪めるため却下。
- **source-only の 🔴 を review へ通す**: 復元対象の欠落を reviewer 入場後まで持ち越して fail-open にするため却下。
  declaration-ahead の 🟡 だけを既存 commit-gate semantics に従って許容する。

## Consequences

### Positive

- operator は config と公開 `bin/sotp` interface だけで phase prerequisite と pre-review gate の実行順を監査できる。
- phase engine は exit status の orchestration に限定され、CLI semantics の二重実装を避けられる。
- argv、cwd、timeout、output、recursion の境界を共通の fail-closed invocation contract として検証できる。
- Makefile wrapper を薄く保ったまま、`sotp review local` と phase engine が dispatch を所有できる。

### Negative

- command rename や argv 変更時は operator-owned config の同期が必要になる。
- config author/operator は有効な command と順序を選ぶ責任を負い、誤設定は validation または command の
  non-zero exit として顕在化する。
- generic process runner には timeout、output capture、recursion detection の実装と診断が必要になる。

## Reassess When

- 公開 `bin/sotp` interface だけでは安全な phase orchestration を表現できない command 種別が実証されたとき。
- argv declaration の互換性維持費が、operator 可視性と config portability の便益を上回ったとき。
- repository-root process execution より狭い権限境界が必要になったとき。

## Related

- `knowledge/adr/2026-07-30-1036-scope-conditional-pre-review-gates.md` D1 / D2 — scope 条件付き
  pre-review gate 宣言と CLI dispatch の refinement 対象。
- `knowledge/adr/2026-07-22-0400-sot-reentry-sequencing.md` D6 — prompt-level sequencing を
  operator-owned phase command config で機械実行可能にする refinement 対象。
- `knowledge/adr/2026-06-27-0852-pre-review-task-contract-conformance-gate.md` D1 / D5 / D6 —
  `task-contract` の catalogue-entry attribution と coverage / liveness の責務境界。D6 は
  coverage-before-check の順序を維持したまま、cargo-make dependency 所有を operator-owned ordered argv
  配線へ移す明示的な modification target。
- `knowledge/adr/2026-06-16-1030-signal-gate-strictness-config.md` D3 — commit gate における
  impl-catalog の interim semantics。
